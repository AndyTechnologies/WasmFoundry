//! The `wf plugin` command: initialize and build plugins.

use std::fs;
use std::path::PathBuf;

use wf_toolchains::process::{CommandSpec, ProcessRunner, StdRunner};

use crate::{EXIT_DIAGNOSTIC, EXIT_OK, EXIT_USAGE};

/// Runs `wf plugin init` and returns the process exit code.
pub fn init(args: &crate::cli::PluginInitArgs) -> i32 {
    let plugin_dir = PathBuf::from(&args.name);

    if plugin_dir.exists() {
        eprintln!(
            "wf: plugin directory {} already exists",
            plugin_dir.display()
        );
        return EXIT_USAGE;
    }

    // Create directory structure
    if let Err(e) = fs::create_dir_all(plugin_dir.join("src")) {
        eprintln!("wf: failed to create plugin directory: {e}");
        return EXIT_DIAGNOSTIC;
    }

    // Create Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = {{ version = "0.261", features = ["wasmtime"] }}
wf-plugin-host = {{ path = "../../../../crates/wf-plugin-host" }}

[build-dependencies]
wit-bindgen = {{ version = "0.261", features = ["wasmtime"] }}
"#,
        args.name
    );
    if let Err(e) = fs::write(plugin_dir.join("Cargo.toml"), cargo_toml) {
        eprintln!("wf: failed to write Cargo.toml: {e}");
        return EXIT_DIAGNOSTIC;
    }

    // Create plugin.wit
    let wit_content = r#"package wf:plugin;

world plugin-world {
    export init: func(config: string) -> result<(), string>
    export pre-build: func(context: build-context) -> result<(), string>
    export post-build: func(context: build-context, result: build-result) -> result<(), string>
    export transform-module: func(module: module-info, bytes: list<u8>) -> result<list<u8>, string>
    import log: func(level: log-level, message: string)
    import read-file: func(path: string) -> result<string, string>
    import write-file: func(path: string, content: string) -> result<(), string>
    import run-command: func(cmd: string, args: list<string>) -> result<string, string>
    import project-root: func() -> string
    import target-dir: func() -> string
    import manifest: func() -> string
}

record build-context {
    project-root: string,
    target-dir: string,
    manifest: string,
}

record build-result {
    success: bool,
    modules: list<module-info>,
    duration-ms: u64,
}

record module-info {
    name: string,
    kind: module-kind,
    path: string,
}

enum module-kind {
    core-module,
    component,
}

enum log-level {
    debug,
    info,
    warn,
    error,
}
"#;
    if let Err(e) = fs::write(plugin_dir.join("plugin.wit"), wit_content) {
        eprintln!("wf: failed to write plugin.wit: {e}");
        return EXIT_DIAGNOSTIC;
    }

    // Create src/lib.rs template
    let lib_rs = r#"// Plugin template for WasmFoundry
// This plugin demonstrates the basic structure.

use wf_plugin_host::{BuildContext, BuildResult, ModuleInfo, ModuleKind, PluginHost};

wit_bindgen::generate!({
    world: "plugin-world",
    path: "../plugin.wit",
    generate_all: true,
});

struct MyPlugin;

impl PluginHost for MyPlugin {
    fn init(&mut self, config: String) -> Result<(), String> {
        // Parse config (JSON) and initialize plugin state
        eprintln!("[my-plugin] init with config: {}", config);
        Ok(())
    }

    fn pre_build(&mut self, context: BuildContext) -> Result<(), String> {
        eprintln!("[my-plugin] pre-build: project_root={}", context.project_root);
        Ok(())
    }

    fn post_build(&mut self, context: BuildContext, result: BuildResult) -> Result<(), String> {
        eprintln!("[my-plugin] post-build: success={}, modules={}", result.success, result.modules.len());
        Ok(())
    }

    fn transform_module(&mut self, module: ModuleInfo, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
        eprintln!("[my-plugin] transform-module: name={}, kind={:?}, size={} bytes",
                  module.name, module.kind, bytes.len());
        // Return bytes unchanged (identity transform)
        Ok(bytes)
    }
}

export!(MyPlugin);
"#;
    if let Err(e) = fs::write(plugin_dir.join("src/lib.rs"), lib_rs) {
        eprintln!("wf: failed to write src/lib.rs: {e}");
        return EXIT_DIAGNOSTIC;
    }

    // Create build.rs for wit-bindgen
    let build_rs = r#"fn main() {
    // Generate bindings from the WIT file
    wit_bindgen::generate!({
        world: "plugin-world",
        path: "../plugin.wit",
        generate_all: true,
    });
}
"#;
    if let Err(e) = fs::write(plugin_dir.join("build.rs"), build_rs) {
        eprintln!("wf: failed to write build.rs: {e}");
        return EXIT_DIAGNOSTIC;
    }

    println!("wf: created plugin {}", args.name);
    println!(
        "  {}: Cargo.toml, plugin.wit, src/lib.rs, build.rs",
        plugin_dir.display()
    );
    println!();
    println!("Next steps:");
    println!("  cd {}", args.name);
    println!("  wf plugin build");
    println!("  # copy target/<name>.wasm to your project's plugins/ directory");

    EXIT_OK
}

/// Runs `wf plugin build` and returns the process exit code.
pub fn build(args: &crate::cli::PluginBuildArgs) -> i32 {
    let plugin_dir = if let Some(dir) = &args.dir {
        PathBuf::from(dir)
    } else {
        PathBuf::from(".")
    };

    if !plugin_dir.exists() {
        eprintln!(
            "wf: plugin directory {} does not exist",
            plugin_dir.display()
        );
        return EXIT_USAGE;
    }
    if !plugin_dir.is_dir() {
        eprintln!("wf: {} is not a directory", plugin_dir.display());
        return EXIT_USAGE;
    }

    let cargo_toml = plugin_dir.join("Cargo.toml");
    if !cargo_toml.is_file() {
        eprintln!("wf: no Cargo.toml in {}", plugin_dir.display());
        return EXIT_USAGE;
    }

    // Check for at least one .wit file
    let wit_file = fs::read_dir(&plugin_dir)
        .map_err(|_| {
            eprintln!("wf: cannot read directory {}", plugin_dir.display());
            EXIT_DIAGNOSTIC
        })
        .and_then(|entries| {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("wit") {
                    return Ok(path);
                }
            }
            Err(EXIT_DIAGNOSTIC)
        });

    if wit_file.is_err() {
        eprintln!("wf: no .wit file found in {}", plugin_dir.display());
        return EXIT_USAGE;
    }

    // Run cargo component build
    let spec = CommandSpec::new("cargo")
        .args([
            "component".to_string(),
            "build".to_string(),
            "--release".to_string(),
            "--target".to_string(),
            "wasm32-wasip1".to_string(),
        ])
        .current_dir(&plugin_dir);

    let output = match StdRunner.run(&spec) {
        Ok(output) => output,
        Err(e) => {
            eprintln!("wf: failed to run cargo component: {e}");
            eprintln!("  help: install cargo component (cargo install cargo-component)");
            return EXIT_DIAGNOSTIC;
        }
    };

    if output.success() {
        // Find the output artifact
        let target_dir = plugin_dir
            .join("target")
            .join("wasm32-wasip1")
            .join("release");
        if let Ok(entries) = fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                    println!("wf: built plugin -> {}", path.display());
                    break;
                }
            }
        }
        EXIT_OK
    } else {
        eprintln!("wf: cargo component build failed:");
        if !output.stdout().is_empty() {
            eprintln!("{}", output.stdout());
        }
        if !output.stderr().is_empty() {
            eprintln!("{}", output.stderr());
        }
        EXIT_DIAGNOSTIC
    }
}
