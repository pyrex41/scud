//! Migration shim for legacy TOML agent definitions
//!
//! Provides compatibility layer for transitioning from legacy agent TOML format
//! to the new extension manifest system. Emits deprecation warnings and maps
//! legacy agent_type fields to extension manifests.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use colored::Colorize;

use super::loader::{
    DiscoveryOptions, ExtensionError, ExtensionManifest, ExtensionRegistry, LegacyAgentToml,
};

/// Global flag to track whether deprecation warnings have been shown this session.
/// Used to avoid spamming the user with repeated warnings.
static DEPRECATION_WARNING_SHOWN: AtomicBool = AtomicBool::new(false);

/// Deprecation warning configuration
#[derive(Debug, Clone)]
pub struct DeprecationConfig {
    /// Whether to show deprecation warnings at all
    pub enabled: bool,

    /// Whether to show only once per session (vs. every load)
    pub once_per_session: bool,

    /// Whether to suggest migration commands
    pub show_migration_hints: bool,
}

impl Default for DeprecationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            once_per_session: true,
            show_migration_hints: true,
        }
    }
}

impl DeprecationConfig {
    /// Create config that suppresses all warnings (useful for tests)
    pub fn silent() -> Self {
        Self {
            enabled: false,
            once_per_session: false,
            show_migration_hints: false,
        }
    }
}

/// Migration shim for converting legacy agent formats to extensions
///
/// This struct provides utilities for:
/// - Loading legacy agent TOML files with deprecation warnings
/// - Mapping task agent_type fields to extension manifests
/// - Converting between legacy and new formats
#[derive(Debug)]
pub struct MigrationShim {
    /// Configuration for deprecation warnings
    deprecation_config: DeprecationConfig,

    /// Cache of agent_type -> extension manifest mappings
    agent_type_cache: HashMap<String, ExtensionManifest>,

    /// Paths that have been loaded as legacy (for tracking)
    legacy_paths_loaded: Vec<PathBuf>,
}

impl MigrationShim {
    /// Create a new migration shim with default configuration
    pub fn new() -> Self {
        Self {
            deprecation_config: DeprecationConfig::default(),
            agent_type_cache: HashMap::new(),
            legacy_paths_loaded: Vec::new(),
        }
    }

    /// Create a migration shim with custom deprecation config
    pub fn with_config(config: DeprecationConfig) -> Self {
        Self {
            deprecation_config: config,
            agent_type_cache: HashMap::new(),
            legacy_paths_loaded: Vec::new(),
        }
    }

    /// Load a legacy agent TOML file, emitting deprecation warning if configured
    pub fn load_legacy_agent(&mut self, path: &Path) -> Result<ExtensionManifest, ExtensionError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ExtensionError::Io(format!("Failed to read {}: {}", path.display(), e)))?;

        self.load_legacy_agent_str(&content, path)
    }

    /// Load a legacy agent from string content
    pub fn load_legacy_agent_str(
        &mut self,
        content: &str,
        path: &Path,
    ) -> Result<ExtensionManifest, ExtensionError> {
        let legacy: LegacyAgentToml = toml::from_str(content)
            .map_err(|e| ExtensionError::Parse(format!("Failed to parse legacy format: {}", e)))?;

        // Extract name before consuming legacy struct
        let agent_name = legacy.agent.name.clone();

        // Emit deprecation warning
        self.emit_deprecation_warning(path, &agent_name);

        // Track loaded path
        self.legacy_paths_loaded.push(path.to_path_buf());

        // Convert to extension manifest (consumes legacy)
        let manifest = legacy.into_extension_manifest(path);

        // Cache by agent name for future lookups
        self.agent_type_cache.insert(agent_name, manifest.clone());

        Ok(manifest)
    }

    /// Resolve an agent_type to an extension manifest
    ///
    /// Searches in order:
    /// 1. Cached manifests from previous loads
    /// 2. Project-local .scud/agents/<agent_type>.toml
    /// 3. Built-in agent definitions (embedded)
    ///
    /// Returns the manifest if found, with deprecation warning for legacy formats.
    pub fn resolve_agent_type(
        &mut self,
        agent_type: &str,
        project_root: &Path,
    ) -> Option<ExtensionManifest> {
        // Check cache first
        if let Some(manifest) = self.agent_type_cache.get(agent_type) {
            return Some(manifest.clone());
        }

        // Try project-local agent definition
        let local_path = project_root
            .join(".scud")
            .join("agents")
            .join(format!("{}.toml", agent_type));

        if local_path.exists() {
            if let Ok(manifest) = self.load_legacy_agent(&local_path) {
                return Some(manifest);
            }
        }

        // Try built-in agents (from assets)
        if let Some(manifest) = self.load_builtin_agent(agent_type) {
            return Some(manifest);
        }

        None
    }

    /// Load a built-in agent definition by name
    fn load_builtin_agent(&mut self, name: &str) -> Option<ExtensionManifest> {
        // Built-in agent definitions embedded at compile time
        let content = match name {
            "builder" => Some(include_str!("../assets/spawn-agents/builder.toml")),
            "analyzer" => Some(include_str!("../assets/spawn-agents/analyzer.toml")),
            "planner" => Some(include_str!("../assets/spawn-agents/planner.toml")),
            "researcher" => Some(include_str!("../assets/spawn-agents/researcher.toml")),
            "reviewer" => Some(include_str!("../assets/spawn-agents/reviewer.toml")),
            "repairer" => Some(include_str!("../assets/spawn-agents/repairer.toml")),
            "fast-builder" => Some(include_str!("../assets/spawn-agents/fast-builder.toml")),
            "outside-generalist" => Some(include_str!(
                "../assets/spawn-agents/outside-generalist.toml"
            )),
            _ => None,
        }?;

        let path = PathBuf::from(format!("built-in:{}.toml", name));

        // Don't emit deprecation warnings for built-in agents - they're managed by us
        let legacy: LegacyAgentToml = toml::from_str(content).ok()?;
        let manifest = legacy.into_extension_manifest(&path);

        // Cache for future lookups
        self.agent_type_cache
            .insert(name.to_string(), manifest.clone());

        Some(manifest)
    }

    /// Emit a deprecation warning for legacy format usage
    fn emit_deprecation_warning(&self, path: &Path, agent_name: &str) {
        if !self.deprecation_config.enabled {
            return;
        }

        if self.deprecation_config.once_per_session {
            // Only show warning once per session
            if DEPRECATION_WARNING_SHOWN.swap(true, Ordering::SeqCst) {
                return;
            }
        }

        eprintln!(
            "{} {} {}",
            "⚠".yellow(),
            "Deprecation warning:".yellow().bold(),
            "Legacy agent TOML format detected".yellow()
        );
        eprintln!(
            "  {} {} ({})",
            "→".dimmed(),
            agent_name.cyan(),
            path.display().to_string().dimmed()
        );

        if self.deprecation_config.show_migration_hints {
            eprintln!();
            eprintln!(
                "  {} The legacy [agent]/[model]/[prompt] format is deprecated.",
                "ℹ".blue()
            );
            eprintln!(
                "  {} Use the new extension format with [extension] section instead.",
                "ℹ".blue()
            );
            eprintln!();
            eprintln!("  {} Convert legacy format:", "→".dimmed());
            eprintln!("    {}", "scud migrate-agents".cyan());
            eprintln!();
            eprintln!(
                "  {} To suppress this warning, set SCUD_NO_DEPRECATION_WARNINGS=1",
                "→".dimmed()
            );
        }
    }

    /// Get list of paths that were loaded as legacy format
    pub fn legacy_paths(&self) -> &[PathBuf] {
        &self.legacy_paths_loaded
    }

    /// Check if any legacy formats were loaded
    pub fn has_legacy_loads(&self) -> bool {
        !self.legacy_paths_loaded.is_empty()
    }

    /// Get cached agent types
    pub fn cached_agent_types(&self) -> Vec<&str> {
        self.agent_type_cache.keys().map(|s| s.as_str()).collect()
    }

    /// Clear the cache
    pub fn clear_cache(&mut self) {
        self.agent_type_cache.clear();
        self.legacy_paths_loaded.clear();
    }
}

impl Default for MigrationShim {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility function to check if content is in legacy agent format
pub fn is_legacy_agent_format(content: &str) -> bool {
    // Try to parse as legacy format
    toml::from_str::<LegacyAgentToml>(content).is_ok() && !content.contains("[extension]")
}

/// Utility function to check if a file is in legacy agent format
pub fn is_legacy_agent_file(path: &Path) -> Result<bool, ExtensionError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ExtensionError::Io(format!("Failed to read {}: {}", path.display(), e)))?;
    Ok(is_legacy_agent_format(&content))
}

/// Convert a legacy agent TOML string to new extension format string
///
/// This is a convenience function for migration tooling.
pub fn convert_legacy_to_extension_toml(
    legacy_content: &str,
    source_path: &Path,
) -> Result<String, ExtensionError> {
    let legacy: LegacyAgentToml = toml::from_str(legacy_content)
        .map_err(|e| ExtensionError::Parse(format!("Failed to parse legacy format: {}", e)))?;

    let manifest = legacy.into_extension_manifest(source_path);

    // Generate the new format TOML
    let mut output = String::new();

    // Extension metadata section
    output.push_str("# Migrated from legacy agent format\n");
    output.push_str("[extension]\n");
    output.push_str(&format!("id = \"{}\"\n", manifest.extension.id));
    output.push_str(&format!("name = \"{}\"\n", manifest.extension.name));
    output.push_str(&format!("version = \"{}\"\n", manifest.extension.version));
    output.push_str(&format!(
        "description = \"\"\"\n{}\n\"\"\"\n",
        manifest.extension.description
    ));

    if let Some(ref main) = manifest.extension.main {
        output.push_str(&format!("main = \"{}\"\n", main));
    }

    // Config section (preserved from legacy)
    if !manifest.config.is_empty() {
        output.push_str("\n# Configuration (migrated from legacy [model] and [prompt] sections)\n");
        output.push_str("[config]\n");

        // Sort keys for consistent output
        let mut keys: Vec<_> = manifest.config.keys().collect();
        keys.sort();

        for key in keys {
            if let Some(value) = manifest.config.get(key) {
                match value {
                    serde_json::Value::String(s) => {
                        if s.contains('\n') {
                            output.push_str(&format!("{} = \"\"\"\n{}\n\"\"\"\n", key, s));
                        } else {
                            output.push_str(&format!("{} = \"{}\"\n", key, s));
                        }
                    }
                    serde_json::Value::Bool(b) => {
                        output.push_str(&format!("{} = {}\n", key, b));
                    }
                    serde_json::Value::Number(n) => {
                        output.push_str(&format!("{} = {}\n", key, n));
                    }
                    _ => {
                        // For complex types, use TOML inline
                        if let Ok(toml_value) = serde_json::from_value::<toml::Value>(value.clone())
                        {
                            output.push_str(&format!("{} = {}\n", key, toml_value));
                        }
                    }
                }
            }
        }
    }

    Ok(output)
}

/// Integration with ExtensionRegistry to load legacy agents with warnings
pub fn load_registry_with_migration(
    registry: &mut ExtensionRegistry,
    root: &Path,
    options: DiscoveryOptions,
    deprecation_config: DeprecationConfig,
) -> Result<MigrationStats, ExtensionError> {
    let result = super::loader::discover(root, options)?;

    let mut stats = MigrationStats::default();

    for ext in &result.extensions {
        if ext.is_legacy {
            stats.legacy_count += 1;
            stats.legacy_paths.push(ext.path.clone());
        } else {
            stats.modern_count += 1;
        }
    }

    // Emit summary deprecation warning if there are legacy extensions
    if stats.legacy_count > 0 && deprecation_config.enabled {
        emit_summary_deprecation_warning(&stats, &deprecation_config);
    }

    registry.load_from_discovery(result);

    Ok(stats)
}

/// Statistics from migration-aware loading
#[derive(Debug, Default)]
pub struct MigrationStats {
    /// Number of extensions loaded in legacy format
    pub legacy_count: usize,

    /// Number of extensions loaded in modern format
    pub modern_count: usize,

    /// Paths to legacy format files
    pub legacy_paths: Vec<PathBuf>,
}

impl MigrationStats {
    /// Check if there were any legacy formats loaded
    pub fn has_legacy(&self) -> bool {
        self.legacy_count > 0
    }

    /// Total extensions loaded
    pub fn total(&self) -> usize {
        self.legacy_count + self.modern_count
    }
}

/// Emit a summary deprecation warning for multiple legacy files
fn emit_summary_deprecation_warning(stats: &MigrationStats, config: &DeprecationConfig) {
    if !config.enabled {
        return;
    }

    if config.once_per_session && DEPRECATION_WARNING_SHOWN.swap(true, Ordering::SeqCst) {
        return;
    }

    eprintln!(
        "{} {} {} legacy agent definition(s) detected",
        "⚠".yellow(),
        "Deprecation warning:".yellow().bold(),
        stats.legacy_count
    );

    // Show first few paths
    let max_show = 3;
    for (i, path) in stats.legacy_paths.iter().take(max_show).enumerate() {
        eprintln!("  {} {}", "→".dimmed(), path.display().to_string().dimmed());
        if i == max_show - 1 && stats.legacy_count > max_show {
            eprintln!(
                "  {} ... and {} more",
                "→".dimmed(),
                stats.legacy_count - max_show
            );
        }
    }

    if config.show_migration_hints {
        eprintln!();
        eprintln!(
            "  {} Run {} to migrate to the new extension format.",
            "ℹ".blue(),
            "scud migrate-agents".cyan()
        );
    }
}

/// Reset the global deprecation warning flag (useful for tests)
pub fn reset_deprecation_warning_flag() {
    DEPRECATION_WARNING_SHOWN.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Reset warning flag before each test
    fn setup() {
        reset_deprecation_warning_flag();
    }

    #[test]
    fn test_migration_shim_new() {
        setup();
        let shim = MigrationShim::new();
        assert!(!shim.has_legacy_loads());
        assert!(shim.cached_agent_types().is_empty());
    }

    #[test]
    fn test_migration_shim_load_legacy() {
        setup();
        let temp = TempDir::new().unwrap();
        let agent_path = temp.path().join("test-agent.toml");

        let content = r#"
[agent]
name = "test-agent"
description = "A test agent"

[model]
harness = "claude"
model = "opus"
"#;
        std::fs::write(&agent_path, content).unwrap();

        let mut shim = MigrationShim::with_config(DeprecationConfig::silent());
        let manifest = shim.load_legacy_agent(&agent_path).unwrap();

        assert_eq!(manifest.extension.name, "test-agent");
        assert_eq!(manifest.extension.id, "legacy.agent.test-agent");
        assert!(shim.has_legacy_loads());
        assert_eq!(shim.legacy_paths().len(), 1);
    }

    #[test]
    fn test_resolve_agent_type_from_cache() {
        setup();
        let temp = TempDir::new().unwrap();
        let agents_dir = temp.path().join(".scud").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let agent_path = agents_dir.join("my-agent.toml");
        let content = r#"
[agent]
name = "my-agent"
description = "My custom agent"

[model]
harness = "opencode"
"#;
        std::fs::write(&agent_path, content).unwrap();

        let mut shim = MigrationShim::with_config(DeprecationConfig::silent());

        // First resolve loads from file
        let manifest = shim.resolve_agent_type("my-agent", temp.path());
        assert!(manifest.is_some());
        assert_eq!(manifest.as_ref().unwrap().extension.name, "my-agent");

        // Second resolve uses cache
        let cached = shim.resolve_agent_type("my-agent", temp.path());
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().extension.name, "my-agent");

        // Check it's cached
        assert!(shim.cached_agent_types().contains(&"my-agent"));
    }

    #[test]
    fn test_resolve_builtin_agent() {
        setup();
        let temp = TempDir::new().unwrap();
        let mut shim = MigrationShim::with_config(DeprecationConfig::silent());

        // Should find built-in builder agent
        let manifest = shim.resolve_agent_type("builder", temp.path());
        assert!(manifest.is_some());
        assert_eq!(manifest.as_ref().unwrap().extension.name, "builder");
    }

    #[test]
    fn test_resolve_nonexistent_agent() {
        setup();
        let temp = TempDir::new().unwrap();
        let mut shim = MigrationShim::with_config(DeprecationConfig::silent());

        let manifest = shim.resolve_agent_type("nonexistent-agent", temp.path());
        assert!(manifest.is_none());
    }

    #[test]
    fn test_is_legacy_agent_format() {
        let legacy = r#"
[agent]
name = "test"
description = "test"
"#;
        assert!(is_legacy_agent_format(legacy));

        let modern = r#"
[extension]
id = "test"
name = "Test"
version = "1.0.0"
description = "test"
"#;
        assert!(!is_legacy_agent_format(modern));

        let invalid = r#"
[random]
key = "value"
"#;
        assert!(!is_legacy_agent_format(invalid));
    }

    #[test]
    fn test_convert_legacy_to_extension_toml() {
        let legacy = r#"
[agent]
name = "my-builder"
description = "A custom builder agent"

[model]
harness = "claude"
model = "sonnet"

[prompt]
template = "You are a builder."
"#;

        let converted =
            convert_legacy_to_extension_toml(legacy, Path::new("my-builder.toml")).unwrap();

        assert!(converted.contains("[extension]"));
        assert!(converted.contains("id = \"legacy.agent.my-builder\""));
        assert!(converted.contains("name = \"my-builder\""));
        assert!(converted.contains("[config]"));
        assert!(converted.contains("harness = \"claude\""));
        assert!(converted.contains("model = \"sonnet\""));
    }

    #[test]
    fn test_migration_stats() {
        let mut stats = MigrationStats::default();
        assert!(!stats.has_legacy());
        assert_eq!(stats.total(), 0);

        stats.legacy_count = 2;
        stats.modern_count = 3;
        assert!(stats.has_legacy());
        assert_eq!(stats.total(), 5);
    }

    #[test]
    fn test_deprecation_config_silent() {
        let config = DeprecationConfig::silent();
        assert!(!config.enabled);
        assert!(!config.show_migration_hints);
    }

    #[test]
    fn test_clear_cache() {
        setup();
        let temp = TempDir::new().unwrap();
        let mut shim = MigrationShim::with_config(DeprecationConfig::silent());

        // Load a builtin agent to populate cache
        shim.resolve_agent_type("builder", temp.path());
        assert!(!shim.cached_agent_types().is_empty());

        shim.clear_cache();
        assert!(shim.cached_agent_types().is_empty());
        assert!(!shim.has_legacy_loads());
    }

    #[test]
    fn test_is_legacy_agent_file() {
        let temp = TempDir::new().unwrap();

        // Legacy file
        let legacy_path = temp.path().join("legacy.toml");
        std::fs::write(
            &legacy_path,
            r#"
[agent]
name = "test"
description = "test"
"#,
        )
        .unwrap();
        assert!(is_legacy_agent_file(&legacy_path).unwrap());

        // Modern file
        let modern_path = temp.path().join("modern.toml");
        std::fs::write(
            &modern_path,
            r#"
[extension]
id = "test"
name = "Test"
version = "1.0.0"
description = "test"
"#,
        )
        .unwrap();
        assert!(!is_legacy_agent_file(&modern_path).unwrap());

        // Nonexistent file
        let missing_path = temp.path().join("missing.toml");
        assert!(is_legacy_agent_file(&missing_path).is_err());
    }

    #[test]
    fn test_load_legacy_agent_str() {
        setup();
        let content = r#"
[agent]
name = "inline-agent"
description = "Loaded from string"

[model]
harness = "opencode"
model = "grok"
"#;

        let mut shim = MigrationShim::with_config(DeprecationConfig::silent());
        let path = PathBuf::from("inline.toml");
        let manifest = shim.load_legacy_agent_str(content, &path).unwrap();

        assert_eq!(manifest.extension.name, "inline-agent");
        assert_eq!(
            manifest.config.get("harness"),
            Some(&serde_json::Value::String("opencode".to_string()))
        );
    }

    #[test]
    fn test_project_local_overrides_builtin() {
        setup();
        let temp = TempDir::new().unwrap();
        let agents_dir = temp.path().join(".scud").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        // Create a project-local builder that overrides the builtin
        let agent_path = agents_dir.join("builder.toml");
        let content = r#"
[agent]
name = "builder"
description = "Custom project builder"

[model]
harness = "opencode"
model = "custom-model"
"#;
        std::fs::write(&agent_path, content).unwrap();

        let mut shim = MigrationShim::with_config(DeprecationConfig::silent());
        let manifest = shim.resolve_agent_type("builder", temp.path());

        assert!(manifest.is_some());
        assert_eq!(
            manifest.as_ref().unwrap().extension.description,
            "Custom project builder"
        );
        // The custom model should be present
        assert_eq!(
            manifest.as_ref().unwrap().config.get("model"),
            Some(&serde_json::Value::String("custom-model".to_string()))
        );
    }
}
