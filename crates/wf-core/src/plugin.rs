//! Plugin system domain types.
//!
//! Plugins are Wasm Components that implement the `wf:plugin/guest` interface
//! and are provided the `wf:plugin/host` interface by the runtime.

// use crate::NameError; // reserved for future validation

/// The capabilities a plugin may request.
///
/// These are granted by the host at load time based on the manifest and
/// user configuration. A plugin without a capability will have its
/// corresponding host function return an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginCapability {
    /// Allow logging via host.log
    Log,
    /// Allow reading files via host.read-file
    ReadFile,
    /// Allow writing files via host.write-file
    WriteFile,
    /// Allow running commands via host.run-command (dangerous!)
    RunCommand,
    /// Allow accessing project-root via host.project-root
    ProjectRoot,
    /// Allow accessing target-dir via host.target-dir
    TargetDir,
    /// Allow accessing manifest via host.manifest
    Manifest,
}

impl PluginCapability {
    /// All available capabilities.
    pub const ALL: &[PluginCapability] = &[
        PluginCapability::Log,
        PluginCapability::ReadFile,
        PluginCapability::WriteFile,
        PluginCapability::RunCommand,
        PluginCapability::ProjectRoot,
        PluginCapability::TargetDir,
        PluginCapability::Manifest,
    ];

    /// Default capabilities granted to every plugin.
    pub const DEFAULT: &[PluginCapability] = &[
        PluginCapability::Log,
        PluginCapability::ProjectRoot,
        PluginCapability::TargetDir,
        PluginCapability::Manifest,
    ];

    /// Returns the WIT function name this capability enables.
    pub fn wit_function(&self) -> &'static str {
        match self {
            PluginCapability::Log => "log",
            PluginCapability::ReadFile => "read-file",
            PluginCapability::WriteFile => "write-file",
            PluginCapability::RunCommand => "run-command",
            PluginCapability::ProjectRoot => "project-root",
            PluginCapability::TargetDir => "target-dir",
            PluginCapability::Manifest => "manifest",
        }
    }
}

/// Configuration for a plugin, as declared in the manifest.
///
/// Plugins are discovered in the `plugins/` directory and configured
/// via `[[plugin]]` entries in `wasmfoundry.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginConfig {
    /// Unique identifier for this plugin.
    pub name: String,
    /// Path to the plugin's .wasm component file, relative to project root.
    pub path: String,
    /// Capabilities granted to this plugin.
    pub capabilities: Vec<PluginCapability>,
    /// Arbitrary JSON configuration passed to plugin.init().
    pub config: Option<String>,
}

impl PluginConfig {
    /// Creates a new plugin config with default capabilities.
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        PluginConfig {
            name: name.into(),
            path: path.into(),
            capabilities: PluginCapability::DEFAULT.to_vec(),
            config: None,
        }
    }

    /// Sets the capabilities for this plugin.
    pub fn with_capabilities(mut self, caps: Vec<PluginCapability>) -> Self {
        self.capabilities = caps;
        self
    }

    /// Sets the JSON configuration for this plugin.
    pub fn with_config(mut self, config: impl Into<String>) -> Self {
        self.config = Some(config.into());
        self
    }
}

/// A loaded plugin instance.
///
/// Contains the compiled component and its metadata. The actual
/// instantiation and execution happens in `wf-runtime`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    /// The plugin's configuration.
    pub config: PluginConfig,
    /// The plugin's component bytes.
    pub bytes: Vec<u8>,
    /// The plugin's WIT world name (should be `wf:plugin/guest`).
    pub world: Option<String>,
}

impl Plugin {
    /// Creates a new plugin from config and bytes.
    pub fn new(config: PluginConfig, bytes: Vec<u8>) -> Self {
        Plugin {
            config,
            bytes,
            world: None,
        }
    }

    /// Sets the world name.
    pub fn with_world(mut self, world: impl Into<String>) -> Self {
        self.world = Some(world.into());
        self
    }
}

/// A plugin manifest entry, as written in `wasmfoundry.toml`.
///
/// This is the TOML representation that gets parsed into `PluginConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    pub name: String,
    pub path: String,
    pub capabilities: Option<Vec<String>>,
    pub config: Option<String>,
}

impl PluginManifest {
    /// Converts to `PluginConfig`, validating capability names.
    pub fn into_config(self) -> Result<PluginConfig, String> {
        let mut config = PluginConfig::new(self.name, self.path);
        if let Some(caps) = self.capabilities {
            let mut parsed = Vec::new();
            for cap in caps {
                parsed.push(parse_capability(&cap)?);
            }
            config.capabilities = parsed;
        }
        config.config = self.config;
        Ok(config)
    }
}

/// Parses a capability string to `PluginCapability`.
fn parse_capability(s: &str) -> Result<PluginCapability, String> {
    match s.to_lowercase().as_str() {
        "log" => Ok(PluginCapability::Log),
        "read-file" | "read_file" => Ok(PluginCapability::ReadFile),
        "write-file" | "write_file" => Ok(PluginCapability::WriteFile),
        "run-command" | "run_command" => Ok(PluginCapability::RunCommand),
        "project-root" | "project_root" => Ok(PluginCapability::ProjectRoot),
        "target-dir" | "target_dir" => Ok(PluginCapability::TargetDir),
        "manifest" => Ok(PluginCapability::Manifest),
        other => Err(format!("unknown plugin capability: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_config_defaults() {
        let config = PluginConfig::new("test", "plugins/test.wasm");
        assert_eq!(config.capabilities, PluginCapability::DEFAULT);
    }

    #[test]
    fn plugin_manifest_parses_capabilities() {
        let manifest = PluginManifest {
            name: "test".into(),
            path: "plugins/test.wasm".into(),
            capabilities: Some(vec!["log".into(), "read-file".into(), "run-command".into()]),
            config: None,
        };
        let config = manifest.into_config().unwrap();
        assert!(config.capabilities.contains(&PluginCapability::Log));
        assert!(config.capabilities.contains(&PluginCapability::ReadFile));
        assert!(config.capabilities.contains(&PluginCapability::RunCommand));
    }

    #[test]
    fn plugin_manifest_rejects_unknown_capability() {
        let manifest = PluginManifest {
            name: "test".into(),
            path: "plugins/test.wasm".into(),
            capabilities: Some(vec!["unknown".into()]),
            config: None,
        };
        let err = manifest.into_config().unwrap_err();
        assert!(err.contains("unknown"));
    }

    #[test]
    fn capability_case_insensitive() {
        assert_eq!(parse_capability("LOG").unwrap(), PluginCapability::Log);
        assert_eq!(
            parse_capability("Read-File").unwrap(),
            PluginCapability::ReadFile
        );
        assert_eq!(
            parse_capability("write_file").unwrap(),
            PluginCapability::WriteFile
        );
    }
}
