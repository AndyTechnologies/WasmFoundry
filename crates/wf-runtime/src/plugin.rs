//! Plugin loader: loads and instantiates Wasm Component plugins.
//!
//! A plugin is a Wasm Component that exports the `wf:plugin/guest` interface
//! and imports the `wf:plugin/host` interface. This module handles loading
//! the component, linking the host interface, and invoking the guest functions.
//!
//! NOTE: This is a minimal implementation for PHASE 18. The component model
//! host function integration requires wit-component generated bindings.
//! For now, we load the plugin and provide a basic interface.

use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};

use wf_core::{Plugin, PluginCapability, PluginConfig};

use crate::RuntimeError;

#[allow(unused_imports)]
use wasmtime::Config;

/// Host state specific to a plugin.
///
/// This is embedded in the main HostState and provides the
/// `wf:plugin/host` interface implementation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PluginHostState {
    /// The plugin's configuration (for capability checks).
    config: PluginConfig,
    /// The project root directory.
    project_root: String,
    /// The target directory.
    target_dir: String,
    /// The manifest as JSON string.
    manifest: String,
}

impl PluginHostState {
    #[allow(dead_code)]
    /// Creates a new plugin host state.
    pub fn new(
        config: PluginConfig,
        project_root: String,
        target_dir: String,
        manifest: String,
    ) -> Self {
        PluginHostState {
            config,
            project_root,
            target_dir,
            manifest,
        }
    }

    /// Checks if the plugin has a capability.
    #[allow(dead_code)]
    fn has_capability(&self, cap: PluginCapability) -> bool {
        self.config.capabilities.contains(&cap)
    }

    /// Implements host.log
    #[allow(dead_code)]
    fn log(&self, level: &str, message: &str) -> Result<(), String> {
        if !self.has_capability(PluginCapability::Log) {
            return Err("capability 'log' not granted".into());
        }
        eprintln!(
            "[plugin:{}] {}: {}",
            self.config.name,
            level.to_uppercase(),
            message
        );
        Ok(())
    }

    /// Implements host.read-file
    #[allow(dead_code)]
    fn read_file(&self, path: &str) -> Result<String, String> {
        if !self.has_capability(PluginCapability::ReadFile) {
            return Err("capability 'read-file' not granted".into());
        }
        let full_path = std::path::Path::new(&self.project_root).join(path);
        std::fs::read_to_string(&full_path).map_err(|e| format!("failed to read {}: {}", path, e))
    }

    /// Implements host.write-file
    #[allow(dead_code)]
    fn write_file(&self, path: &str, content: &str) -> Result<(), String> {
        if !self.has_capability(PluginCapability::WriteFile) {
            return Err("capability 'write-file' not granted".into());
        }
        let full_path = std::path::Path::new(&self.project_root).join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("failed to create dir: {}", e))?;
        }
        std::fs::write(&full_path, content).map_err(|e| format!("failed to write {}: {}", path, e))
    }

    /// Implements host.run-command
    #[allow(dead_code)]
    fn run_command(&self, cmd: &str, args: &[String]) -> Result<String, String> {
        if !self.has_capability(PluginCapability::RunCommand) {
            return Err("capability 'run-command' not granted".into());
        }
        let output = std::process::Command::new(cmd)
            .args(args)
            .current_dir(&self.project_root)
            .output()
            .map_err(|e| format!("failed to run command: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    /// Implements host.project-root
    #[allow(dead_code)]
    fn project_root(&self) -> String {
        if !self.has_capability(PluginCapability::ProjectRoot) {
            return "capability 'project-root' not granted".into();
        }
        self.project_root.clone()
    }

    /// Implements host.target-dir
    #[allow(dead_code)]
    fn target_dir(&self) -> String {
        if !self.has_capability(PluginCapability::TargetDir) {
            return "capability 'target-dir' not granted".into();
        }
        self.target_dir.clone()
    }

    /// Implements host.manifest
    #[allow(dead_code)]
    fn manifest(&self) -> String {
        if !self.has_capability(PluginCapability::Manifest) {
            return "capability 'manifest' not granted".into();
        }
        self.manifest.clone()
    }
}

/// A loaded and instantiated plugin.
///
/// Holds the component instance and provides methods to invoke
/// the plugin's exported functions.
#[derive(Debug)]
pub struct LoadedPlugin {
    /// The plugin's configuration.
    config: PluginConfig,
    /// The Wasmtime component instance.
    _instance: wasmtime::component::Instance,
    /// The Wasmtime store (holds the instance state).
    _store: Store<PluginHostState>,
    /// The component bytes (for re-instantiation if needed).
    _bytes: Vec<u8>,
}

/// Loads a plugin component and instantiates it with the host interface.
pub fn load_plugin(
    engine: &Engine,
    plugin: &Plugin,
    project_root: &str,
    target_dir: &str,
    manifest_json: &str,
) -> Result<LoadedPlugin, RuntimeError> {
    // Create the component
    let component =
        Component::new(engine, &plugin.bytes).map_err(|e| RuntimeError::InvalidWasm {
            message: e.to_string(),
        })?;

    // Create host state for this plugin
    let host_state = PluginHostState::new(
        plugin.config.clone(),
        project_root.to_string(),
        target_dir.to_string(),
        manifest_json.to_string(),
    );

    // Create store with plugin host state
    let mut store = Store::new(engine, host_state);

    // Create linker - for now we don't add host functions (requires wit-component bindings)
    let linker: Linker<PluginHostState> = Linker::new(engine);

    // TODO: Add host interface functions using wit-component generated bindings
    // For now, we just instantiate the component without host imports
    // This means plugins must not use host functions until this is implemented

    // Instantiate the component
    let instance = linker
        .instantiate(&mut store, &component)
        .map_err(|e| RuntimeError::Link {
            message: e.to_string(),
        })?;

    // TODO: Call plugin.init with config (requires component model function calling)

    Ok(LoadedPlugin {
        config: plugin.config.clone(),
        _instance: instance,
        _store: store,
        _bytes: plugin.bytes.clone(),
    })
}

impl LoadedPlugin {
    /// Returns the plugin's name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Calls the plugin's `pre-build` hook.
    pub fn pre_build(&mut self, _context: BuildContext) -> Result<(), RuntimeError> {
        // TODO: Implement when component function calling is available
        Ok(())
    }

    /// Calls the plugin's `post-build` hook.
    pub fn post_build(
        &mut self,
        _context: BuildContext,
        _result: BuildResult,
    ) -> Result<(), RuntimeError> {
        // TODO: Implement when component function calling is available
        Ok(())
    }

    /// Calls the plugin's `transform-module` hook.
    pub fn transform_module(
        &mut self,
        _module: ModuleInfo,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>, RuntimeError> {
        // TODO: Implement when component function calling is available
        Ok(bytes)
    }
}

/// Build context passed to plugin hooks.
#[derive(Debug, Clone)]
pub struct BuildContext {
    pub project_root: String,
    pub target_dir: String,
    pub manifest: String,
}

/// Build result passed to post-build hook.
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub success: bool,
    pub modules: Vec<ModuleInfo>,
    pub duration_ms: u64,
}

/// Module info passed to plugin hooks.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub kind: ModuleKind,
    pub path: String,
}

/// Module kind for plugin hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    CoreModule,
    Component,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use wasmtime::Config;

    #[test]
    fn plugin_host_state_capability_check() {
        let config = PluginConfig::new("test", "test.wasm")
            .with_capabilities(vec![PluginCapability::Log, PluginCapability::ReadFile]);
        let state = PluginHostState::new(config, "/project".into(), "/target".into(), "{}".into());

        assert!(state.has_capability(PluginCapability::Log));
        assert!(state.has_capability(PluginCapability::ReadFile));
        assert!(!state.has_capability(PluginCapability::WriteFile));
    }
}
