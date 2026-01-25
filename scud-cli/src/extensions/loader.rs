use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Extension manifest structure for parsing extension.toml files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    /// Basic extension metadata
    pub extension: ExtensionMetadata,

    /// Tool definitions provided by the extension
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,

    /// Event handlers (lifecycle hooks)
    #[serde(default)]
    pub events: Vec<EventHandler>,

    /// Configuration options for the extension
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,

    /// Extension dependencies
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
}

/// Metadata section of an extension manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionMetadata {
    /// Unique identifier for the extension
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Version string (semver)
    pub version: String,

    /// Description of what the extension does
    pub description: String,

    /// Author or organization name
    #[serde(default)]
    pub author: Option<String>,

    /// Main entry point file (relative to extension directory)
    #[serde(default)]
    pub main: Option<String>,

    /// License identifier
    #[serde(default)]
    pub license: Option<String>,

    /// Homepage URL
    #[serde(default)]
    pub homepage: Option<String>,

    /// Repository URL
    #[serde(default)]
    pub repository: Option<String>,
}

/// Tool definition within an extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (used as command identifier)
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// Input parameters schema
    #[serde(default)]
    pub parameters: Vec<ToolParameter>,

    /// Command to execute (for script-based tools)
    #[serde(default)]
    pub command: Option<String>,

    /// Script file to run (relative to extension directory)
    #[serde(default)]
    pub script: Option<String>,
}

/// Parameter definition for a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    /// Parameter name
    pub name: String,

    /// Parameter type (string, number, boolean, array, object)
    #[serde(rename = "type")]
    pub param_type: String,

    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,

    /// Whether this parameter is required
    #[serde(default)]
    pub required: bool,

    /// Default value if not provided
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

/// Event handler definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventHandler {
    /// Event name to listen for (e.g., "task.start", "task.complete", "session.init")
    pub event: String,

    /// Handler type: "script", "command", or "webhook"
    #[serde(rename = "type")]
    pub handler_type: String,

    /// Command to execute (for "command" type)
    #[serde(default)]
    pub command: Option<String>,

    /// Script file to run (for "script" type)
    #[serde(default)]
    pub script: Option<String>,

    /// Webhook URL (for "webhook" type)
    #[serde(default)]
    pub url: Option<String>,
}

impl ExtensionManifest {
    /// Load and parse an extension manifest from a TOML file
    ///
    /// Supports both the new extension.toml format and legacy agent TOML format
    /// via automatic detection and conversion.
    pub fn from_file(path: &PathBuf) -> Result<Self, ExtensionError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ExtensionError::Io(format!("Failed to read {}: {}", path.display(), e)))?;

        Self::from_str(&content, path)
    }

    /// Parse an extension manifest from a string
    ///
    /// Automatically detects and handles legacy agent TOML format.
    pub fn from_str(content: &str, path: &PathBuf) -> Result<Self, ExtensionError> {
        // Try parsing as new extension format first
        if let Ok(manifest) = toml::from_str::<ExtensionManifest>(content) {
            return Ok(manifest);
        }

        // Try parsing as legacy agent TOML format and convert via shim
        if let Ok(legacy) = toml::from_str::<LegacyAgentToml>(content) {
            return Ok(legacy.into_extension_manifest(path));
        }

        // If both fail, return the error from the primary format
        Err(ExtensionError::Parse(format!(
            "Failed to parse {} as extension or legacy agent format",
            path.display()
        )))
    }
}

// =============================================================================
// Legacy Agent TOML Shim
// =============================================================================
// Provides compatibility with the existing spawn-agents/*.toml format:
//
// [agent]
// name = "builder"
// description = "..."
//
// [model]
// harness = "claude"
// model = "opus"
//
// [prompt]
// template = "..."

/// Legacy agent TOML format for backward compatibility
#[derive(Debug, Clone, Deserialize)]
pub struct LegacyAgentToml {
    /// Agent metadata section
    pub agent: LegacyAgentSection,

    /// Model configuration section
    #[serde(default)]
    pub model: Option<LegacyModelSection>,

    /// Prompt template section
    #[serde(default)]
    pub prompt: Option<LegacyPromptSection>,
}

/// Agent section in legacy format
#[derive(Debug, Clone, Deserialize)]
pub struct LegacyAgentSection {
    /// Agent name
    pub name: String,

    /// Agent description
    pub description: String,
}

/// Model section in legacy format
#[derive(Debug, Clone, Deserialize)]
pub struct LegacyModelSection {
    /// Harness type (e.g., "claude", "openai")
    #[serde(default)]
    pub harness: Option<String>,

    /// Model name (e.g., "opus", "sonnet")
    #[serde(default)]
    pub model: Option<String>,
}

/// Prompt section in legacy format
#[derive(Debug, Clone, Deserialize)]
pub struct LegacyPromptSection {
    /// Prompt template with variable placeholders
    #[serde(default)]
    pub template: Option<String>,
}

impl LegacyAgentToml {
    /// Convert legacy agent TOML to new extension manifest format
    pub fn into_extension_manifest(self, path: &PathBuf) -> ExtensionManifest {
        let mut config = HashMap::new();

        // Preserve model configuration
        if let Some(model) = &self.model {
            if let Some(harness) = &model.harness {
                config.insert(
                    "harness".to_string(),
                    serde_json::Value::String(harness.clone()),
                );
            }
            if let Some(model_name) = &model.model {
                config.insert(
                    "model".to_string(),
                    serde_json::Value::String(model_name.clone()),
                );
            }
        }

        // Preserve prompt template
        if let Some(prompt) = &self.prompt {
            if let Some(template) = &prompt.template {
                config.insert(
                    "prompt_template".to_string(),
                    serde_json::Value::String(template.clone()),
                );
            }
        }

        // Store original format indicator
        config.insert("_legacy_format".to_string(), serde_json::Value::Bool(true));

        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        ExtensionManifest {
            extension: ExtensionMetadata {
                id: format!("legacy.agent.{}", self.agent.name),
                name: self.agent.name,
                version: "1.0.0".to_string(),
                description: self.agent.description,
                author: None,
                main: Some(format!("{}.toml", filename)),
                license: None,
                homepage: None,
                repository: None,
            },
            tools: Vec::new(),
            events: Vec::new(),
            config,
            dependencies: HashMap::new(),
        }
    }
}

/// Error type for extension operations
#[derive(Debug, thiserror::Error)]
pub enum ExtensionError {
    #[error("IO error: {0}")]
    Io(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Extension not found: {0}")]
    NotFound(String),

    #[error("Duplicate extension ID: {0}")]
    DuplicateId(String),

    #[error("Discovery error in {path}: {message}")]
    Discovery { path: String, message: String },
}

// =============================================================================
// Extension Validation
// =============================================================================

impl ExtensionManifest {
    /// Validate the extension manifest for completeness and correctness
    ///
    /// Returns Ok(()) if valid, or Err with descriptive validation errors.
    pub fn validate(&self) -> Result<(), ExtensionError> {
        let mut errors = Vec::new();

        // Validate extension metadata
        if self.extension.id.is_empty() {
            errors.push("extension.id cannot be empty".to_string());
        }
        if self.extension.name.is_empty() {
            errors.push("extension.name cannot be empty".to_string());
        }
        if self.extension.version.is_empty() {
            errors.push("extension.version cannot be empty".to_string());
        }
        if self.extension.description.is_empty() {
            errors.push("extension.description cannot be empty".to_string());
        }

        // Validate ID format (alphanumeric, dots, hyphens, underscores)
        if !self
            .extension
            .id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
        {
            errors.push(format!(
                "extension.id '{}' contains invalid characters (allowed: alphanumeric, '.', '-', '_')",
                self.extension.id
            ));
        }

        // Validate version format (basic semver check)
        if !self
            .extension
            .version
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
        {
            // Allow pre-release versions like 1.0.0-beta.1
            if !is_valid_semver(&self.extension.version) {
                errors.push(format!(
                    "extension.version '{}' is not a valid semantic version",
                    self.extension.version
                ));
            }
        }

        // Validate tools
        for (i, tool) in self.tools.iter().enumerate() {
            if tool.name.is_empty() {
                errors.push(format!("tools[{}].name cannot be empty", i));
            }
            if tool.description.is_empty() {
                errors.push(format!("tools[{}].description cannot be empty", i));
            }

            // Tool must have either command or script
            if tool.command.is_none() && tool.script.is_none() {
                // This is allowed for tools that might be implemented natively
            }

            // Validate parameters
            for (j, param) in tool.parameters.iter().enumerate() {
                if param.name.is_empty() {
                    errors.push(format!(
                        "tools[{}].parameters[{}].name cannot be empty",
                        i, j
                    ));
                }
                if !is_valid_param_type(&param.param_type) {
                    errors.push(format!(
                        "tools[{}].parameters[{}].type '{}' is not valid (expected: string, number, boolean, array, object)",
                        i, j, param.param_type
                    ));
                }
            }
        }

        // Validate events
        for (i, event) in self.events.iter().enumerate() {
            if event.event.is_empty() {
                errors.push(format!("events[{}].event cannot be empty", i));
            }

            // Validate handler type
            match event.handler_type.as_str() {
                "command" => {
                    if event.command.is_none() {
                        errors.push(format!(
                            "events[{}] has type 'command' but no command specified",
                            i
                        ));
                    }
                }
                "script" => {
                    if event.script.is_none() {
                        errors.push(format!(
                            "events[{}] has type 'script' but no script specified",
                            i
                        ));
                    }
                }
                "webhook" => {
                    if event.url.is_none() {
                        errors.push(format!(
                            "events[{}] has type 'webhook' but no url specified",
                            i
                        ));
                    }
                }
                other => {
                    errors.push(format!(
                        "events[{}].type '{}' is not valid (expected: command, script, webhook)",
                        i, other
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ExtensionError::Validation(errors.join("; ")))
        }
    }
}

/// Check if a string is a valid semantic version
fn is_valid_semver(version: &str) -> bool {
    // Basic semver: X.Y.Z or X.Y.Z-prerelease+build
    let parts: Vec<&str> = version.split('-').collect();
    let version_part = parts[0];

    let nums: Vec<&str> = version_part.split('.').collect();
    if nums.len() < 2 || nums.len() > 3 {
        return false;
    }

    nums.iter().all(|n| n.parse::<u32>().is_ok())
}

/// Check if a parameter type is valid
fn is_valid_param_type(param_type: &str) -> bool {
    matches!(
        param_type,
        "string" | "number" | "boolean" | "array" | "object" | "integer"
    )
}

// =============================================================================
// Extension Discovery
// =============================================================================

/// Options for extension discovery
#[derive(Debug, Clone, Default)]
pub struct DiscoveryOptions {
    /// Maximum depth to traverse (None = unlimited)
    pub max_depth: Option<usize>,

    /// Whether to include legacy agent TOML files (in spawn-agents/ directories)
    pub include_legacy: bool,

    /// Whether to follow symbolic links
    pub follow_symlinks: bool,

    /// Whether to validate manifests during discovery
    pub validate: bool,

    /// Whether to skip extensions that fail to load (vs returning errors)
    pub skip_errors: bool,
}

impl DiscoveryOptions {
    /// Create options for discovering all extensions with validation
    pub fn standard() -> Self {
        Self {
            max_depth: Some(10),
            include_legacy: true,
            follow_symlinks: false,
            validate: true,
            skip_errors: false,
        }
    }

    /// Create lenient options that skip errors and include legacy
    pub fn lenient() -> Self {
        Self {
            max_depth: Some(10),
            include_legacy: true,
            follow_symlinks: false,
            validate: false,
            skip_errors: true,
        }
    }
}

/// Result of discovering a single extension
#[derive(Debug, Clone)]
pub struct DiscoveredExtension {
    /// The parsed extension manifest
    pub manifest: ExtensionManifest,

    /// Path to the manifest file
    pub path: PathBuf,

    /// Directory containing the extension
    pub directory: PathBuf,

    /// Whether this was loaded from legacy format
    pub is_legacy: bool,
}

/// Result of extension discovery operation
#[derive(Debug, Default)]
pub struct DiscoveryResult {
    /// Successfully discovered extensions
    pub extensions: Vec<DiscoveredExtension>,

    /// Errors encountered during discovery (when skip_errors is true)
    pub errors: Vec<ExtensionError>,

    /// Paths that were skipped
    pub skipped: Vec<PathBuf>,
}

impl DiscoveryResult {
    /// Check if discovery found any extensions
    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }

    /// Get count of discovered extensions
    pub fn count(&self) -> usize {
        self.extensions.len()
    }

    /// Check if there were any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Discover extensions in a directory tree
///
/// Scans the given directory for `extension.toml` files and optionally
/// legacy agent TOML files in `spawn-agents/` directories.
///
/// # Arguments
/// * `root` - The root directory to scan
/// * `options` - Discovery options controlling behavior
///
/// # Returns
/// * `Ok(DiscoveryResult)` - The discovery results with extensions and any errors
/// * `Err(ExtensionError)` - If a fatal error occurs during discovery
///
/// # Example
/// ```no_run
/// use std::path::PathBuf;
/// use scud::extensions::loader::{discover, DiscoveryOptions};
///
/// let root = PathBuf::from("./extensions");
/// let result = discover(&root, DiscoveryOptions::standard()).unwrap();
/// for ext in result.extensions {
///     println!("Found: {} v{}", ext.manifest.extension.name, ext.manifest.extension.version);
/// }
/// ```
pub fn discover(root: &Path, options: DiscoveryOptions) -> Result<DiscoveryResult, ExtensionError> {
    let mut result = DiscoveryResult::default();

    // Verify root directory exists
    if !root.exists() {
        return Err(ExtensionError::Io(format!(
            "Discovery root does not exist: {}",
            root.display()
        )));
    }

    if !root.is_dir() {
        return Err(ExtensionError::Io(format!(
            "Discovery root is not a directory: {}",
            root.display()
        )));
    }

    // Build walker with options
    let mut walker = WalkDir::new(root).follow_links(options.follow_symlinks);

    if let Some(max_depth) = options.max_depth {
        walker = walker.max_depth(max_depth);
    }

    // Track seen IDs to detect duplicates
    let mut seen_ids: HashMap<String, PathBuf> = HashMap::new();

    for entry in walker.into_iter() {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                let err = ExtensionError::Discovery {
                    path: e
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    message: e.to_string(),
                };
                if options.skip_errors {
                    result.errors.push(err);
                    continue;
                } else {
                    return Err(err);
                }
            }
        };

        let path = entry.path();

        // Skip non-files
        if !path.is_file() {
            continue;
        }

        // Check if this is an extension manifest
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let is_extension_toml = file_name == "extension.toml";
        let is_legacy_agent = options.include_legacy
            && file_name.ends_with(".toml")
            && path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                == Some("spawn-agents");

        if !is_extension_toml && !is_legacy_agent {
            continue;
        }

        // Try to load the manifest
        let manifest = match ExtensionManifest::from_file(&path.to_path_buf()) {
            Ok(m) => m,
            Err(e) => {
                if options.skip_errors {
                    result.errors.push(e);
                    result.skipped.push(path.to_path_buf());
                    continue;
                } else {
                    return Err(e);
                }
            }
        };

        // Validate if requested
        if options.validate {
            if let Err(e) = manifest.validate() {
                if options.skip_errors {
                    result.errors.push(e);
                    result.skipped.push(path.to_path_buf());
                    continue;
                } else {
                    return Err(e);
                }
            }
        }

        // Check for duplicate IDs
        let ext_id = &manifest.extension.id;
        if let Some(existing_path) = seen_ids.get(ext_id) {
            let err = ExtensionError::DuplicateId(format!(
                "'{}' defined in both {} and {}",
                ext_id,
                existing_path.display(),
                path.display()
            ));
            if options.skip_errors {
                result.errors.push(err);
                result.skipped.push(path.to_path_buf());
                continue;
            } else {
                return Err(err);
            }
        }
        seen_ids.insert(ext_id.clone(), path.to_path_buf());

        // Add to results
        let directory = path.parent().unwrap_or(path).to_path_buf();
        result.extensions.push(DiscoveredExtension {
            manifest,
            path: path.to_path_buf(),
            directory,
            is_legacy: is_legacy_agent,
        });
    }

    Ok(result)
}

/// Discover extensions from multiple root directories
///
/// Convenience function that scans multiple directories and combines results.
pub fn discover_all(
    roots: &[&Path],
    options: DiscoveryOptions,
) -> Result<DiscoveryResult, ExtensionError> {
    let mut combined = DiscoveryResult::default();
    let mut seen_ids: HashMap<String, PathBuf> = HashMap::new();

    for root in roots {
        if !root.exists() {
            continue; // Skip non-existent directories
        }

        let result = discover(root, options.clone())?;

        for ext in result.extensions {
            let ext_id = &ext.manifest.extension.id;
            if let Some(existing_path) = seen_ids.get(ext_id) {
                let err = ExtensionError::DuplicateId(format!(
                    "'{}' defined in both {} and {}",
                    ext_id,
                    existing_path.display(),
                    ext.path.display()
                ));
                if options.skip_errors {
                    combined.errors.push(err);
                    combined.skipped.push(ext.path.clone());
                    continue;
                } else {
                    return Err(err);
                }
            }
            seen_ids.insert(ext_id.clone(), ext.path.clone());
            combined.extensions.push(ext);
        }

        combined.errors.extend(result.errors);
        combined.skipped.extend(result.skipped);
    }

    Ok(combined)
}

// =============================================================================
// Extension Registry
// =============================================================================

/// Registry for managing discovered extensions
#[derive(Debug, Default)]
pub struct ExtensionRegistry {
    /// Discovered extensions indexed by ID
    extensions: HashMap<String, DiscoveredExtension>,
}

impl ExtensionRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Load extensions from a discovery result
    pub fn load_from_discovery(&mut self, result: DiscoveryResult) {
        for ext in result.extensions {
            self.extensions
                .insert(ext.manifest.extension.id.clone(), ext);
        }
    }

    /// Discover and load extensions from a directory
    pub fn discover_and_load(
        &mut self,
        root: &Path,
        options: DiscoveryOptions,
    ) -> Result<&mut Self, ExtensionError> {
        let result = discover(root, options)?;
        self.load_from_discovery(result);
        Ok(self)
    }

    /// Get an extension by ID
    pub fn get(&self, id: &str) -> Option<&DiscoveredExtension> {
        self.extensions.get(id)
    }

    /// Get an extension by ID (returns error if not found)
    pub fn get_or_error(&self, id: &str) -> Result<&DiscoveredExtension, ExtensionError> {
        self.extensions
            .get(id)
            .ok_or_else(|| ExtensionError::NotFound(id.to_string()))
    }

    /// Check if an extension is registered
    pub fn has(&self, id: &str) -> bool {
        self.extensions.contains_key(id)
    }

    /// List all extension IDs
    pub fn list_ids(&self) -> Vec<&str> {
        self.extensions.keys().map(|s| s.as_str()).collect()
    }

    /// List all extensions
    pub fn list(&self) -> Vec<&DiscoveredExtension> {
        self.extensions.values().collect()
    }

    /// Get count of registered extensions
    pub fn count(&self) -> usize {
        self.extensions.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }

    /// Remove an extension by ID
    pub fn remove(&mut self, id: &str) -> Option<DiscoveredExtension> {
        self.extensions.remove(id)
    }

    /// Clear all extensions
    pub fn clear(&mut self) {
        self.extensions.clear();
    }

    /// Filter extensions by predicate
    pub fn filter<F>(&self, predicate: F) -> Vec<&DiscoveredExtension>
    where
        F: Fn(&DiscoveredExtension) -> bool,
    {
        self.extensions.values().filter(|e| predicate(e)).collect()
    }

    /// Get all legacy extensions
    pub fn legacy_extensions(&self) -> Vec<&DiscoveredExtension> {
        self.filter(|e| e.is_legacy)
    }

    /// Get all non-legacy extensions
    pub fn modern_extensions(&self) -> Vec<&DiscoveredExtension> {
        self.filter(|e| !e.is_legacy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_extension_manifest() {
        let content = r#"
[extension]
id = "test-extension"
name = "Test Extension"
version = "1.0.0"
description = "A test extension"

[[tools]]
name = "test_tool"
description = "A test tool"

[[tools.parameters]]
name = "input"
type = "string"
required = true

[[events]]
event = "task.start"
type = "command"
command = "echo 'Task started'"
"#;

        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        assert_eq!(manifest.extension.id, "test-extension");
        assert_eq!(manifest.extension.name, "Test Extension");
        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.tools[0].name, "test_tool");
        assert_eq!(manifest.tools[0].parameters.len(), 1);
        assert_eq!(manifest.events.len(), 1);
        assert_eq!(manifest.events[0].event, "task.start");
    }

    #[test]
    fn test_parse_legacy_agent_toml() {
        let content = r#"
[agent]
name = "builder"
description = "Fast code implementation agent"

[model]
harness = "claude"
model = "opus"

[prompt]
template = "You are a code builder."
"#;

        let manifest =
            ExtensionManifest::from_str(content, &PathBuf::from("builder.toml")).unwrap();
        assert_eq!(manifest.extension.id, "legacy.agent.builder");
        assert_eq!(manifest.extension.name, "builder");
        assert_eq!(
            manifest.extension.description,
            "Fast code implementation agent"
        );

        // Check config preservation
        assert_eq!(
            manifest.config.get("harness"),
            Some(&serde_json::Value::String("claude".to_string()))
        );
        assert_eq!(
            manifest.config.get("model"),
            Some(&serde_json::Value::String("opus".to_string()))
        );
        assert!(manifest.config.get("prompt_template").is_some());
        assert_eq!(
            manifest.config.get("_legacy_format"),
            Some(&serde_json::Value::Bool(true))
        );
    }

    #[test]
    fn test_legacy_minimal() {
        let content = r#"
[agent]
name = "minimal"
description = "Minimal agent"
"#;

        let manifest =
            ExtensionManifest::from_str(content, &PathBuf::from("minimal.toml")).unwrap();
        assert_eq!(manifest.extension.name, "minimal");
        assert!(manifest.tools.is_empty());
        assert!(manifest.events.is_empty());
    }

    // =========================================================================
    // Error handling tests
    // =========================================================================

    #[test]
    fn test_parse_invalid_toml_syntax() {
        let content = r#"
[extension
id = "broken"
"#;
        let result = ExtensionManifest::from_str(content, &PathBuf::from("broken.toml"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExtensionError::Parse(_)));
        assert!(err.to_string().contains("Failed to parse"));
    }

    #[test]
    fn test_parse_missing_required_extension_fields() {
        // Missing 'description' field
        let content = r#"
[extension]
id = "test"
name = "Test"
version = "1.0.0"
"#;
        let result = ExtensionManifest::from_str(content, &PathBuf::from("test.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_neither_format_matches() {
        // Content that doesn't match either new or legacy format
        let content = r#"
[random]
key = "value"
"#;
        let result = ExtensionManifest::from_str(content, &PathBuf::from("random.toml"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExtensionError::Parse(_)));
    }

    #[test]
    fn test_parse_empty_content() {
        let content = "";
        let result = ExtensionManifest::from_str(content, &PathBuf::from("empty.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_whitespace_only_content() {
        let content = "   \n\t\n   ";
        let result = ExtensionManifest::from_str(content, &PathBuf::from("whitespace.toml"));
        assert!(result.is_err());
    }

    // =========================================================================
    // from_file tests
    // =========================================================================

    #[test]
    fn test_from_file_missing_file() {
        let path = PathBuf::from("/nonexistent/path/to/extension.toml");
        let result = ExtensionManifest::from_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExtensionError::Io(_)));
        assert!(err.to_string().contains("Failed to read"));
    }

    #[test]
    fn test_from_file_valid_extension() {
        let mut temp = NamedTempFile::new().unwrap();
        let content = r#"
[extension]
id = "file-test"
name = "File Test"
version = "1.0.0"
description = "Test loading from file"
"#;
        temp.write_all(content.as_bytes()).unwrap();

        let manifest = ExtensionManifest::from_file(&temp.path().to_path_buf()).unwrap();
        assert_eq!(manifest.extension.id, "file-test");
        assert_eq!(manifest.extension.name, "File Test");
    }

    #[test]
    fn test_from_file_valid_legacy() {
        let mut temp = NamedTempFile::new().unwrap();
        let content = r#"
[agent]
name = "file-legacy"
description = "Legacy from file"
"#;
        temp.write_all(content.as_bytes()).unwrap();

        let manifest = ExtensionManifest::from_file(&temp.path().to_path_buf()).unwrap();
        assert_eq!(manifest.extension.name, "file-legacy");
        assert!(manifest.config.get("_legacy_format").is_some());
    }

    #[test]
    fn test_from_file_invalid_content() {
        let mut temp = NamedTempFile::new().unwrap();
        let content = "not valid toml [[[";
        temp.write_all(content.as_bytes()).unwrap();

        let result = ExtensionManifest::from_file(&temp.path().to_path_buf());
        assert!(result.is_err());
    }

    // =========================================================================
    // Extension manifest edge cases
    // =========================================================================

    #[test]
    fn test_extension_with_all_optional_fields() {
        let content = r#"
[extension]
id = "full-extension"
name = "Full Extension"
version = "2.0.0"
description = "Extension with all fields"
author = "Test Author"
main = "main.py"
license = "MIT"
homepage = "https://example.com"
repository = "https://github.com/test/repo"

[config]
api_key = "secret"
timeout = 30
debug = true

[dependencies]
"other-ext" = ">=1.0.0"
"another-ext" = "^2.0"
"#;

        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("full.toml")).unwrap();
        assert_eq!(manifest.extension.author, Some("Test Author".to_string()));
        assert_eq!(manifest.extension.main, Some("main.py".to_string()));
        assert_eq!(manifest.extension.license, Some("MIT".to_string()));
        assert_eq!(
            manifest.extension.homepage,
            Some("https://example.com".to_string())
        );
        assert_eq!(
            manifest.extension.repository,
            Some("https://github.com/test/repo".to_string())
        );
        assert_eq!(manifest.config.len(), 3);
        assert_eq!(manifest.dependencies.len(), 2);
    }

    #[test]
    fn test_extension_minimal() {
        let content = r#"
[extension]
id = "min"
name = "Minimal"
version = "0.1.0"
description = "Bare minimum"
"#;

        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("min.toml")).unwrap();
        assert_eq!(manifest.extension.id, "min");
        assert!(manifest.extension.author.is_none());
        assert!(manifest.extension.main.is_none());
        assert!(manifest.tools.is_empty());
        assert!(manifest.events.is_empty());
        assert!(manifest.config.is_empty());
        assert!(manifest.dependencies.is_empty());
    }

    #[test]
    fn test_extension_with_multiple_tools() {
        let content = r#"
[extension]
id = "multi-tool"
name = "Multi Tool"
version = "1.0.0"
description = "Multiple tools"

[[tools]]
name = "tool1"
description = "First tool"
command = "echo 1"

[[tools]]
name = "tool2"
description = "Second tool"
script = "scripts/tool2.py"

[[tools]]
name = "tool3"
description = "Third tool"

[[tools.parameters]]
name = "param1"
type = "string"
required = true

[[tools.parameters]]
name = "param2"
type = "number"
required = false
default = 42
"#;

        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("multi.toml")).unwrap();
        assert_eq!(manifest.tools.len(), 3);
        assert_eq!(manifest.tools[0].command, Some("echo 1".to_string()));
        assert_eq!(
            manifest.tools[1].script,
            Some("scripts/tool2.py".to_string())
        );
        assert_eq!(manifest.tools[2].parameters.len(), 2);
        assert_eq!(
            manifest.tools[2].parameters[1].default,
            Some(serde_json::json!(42))
        );
    }

    #[test]
    fn test_extension_with_multiple_events() {
        let content = r#"
[extension]
id = "multi-event"
name = "Multi Event"
version = "1.0.0"
description = "Multiple events"

[[events]]
event = "task.start"
type = "command"
command = "echo start"

[[events]]
event = "task.complete"
type = "script"
script = "hooks/complete.sh"

[[events]]
event = "session.end"
type = "webhook"
url = "https://hooks.example.com/notify"
"#;

        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("events.toml")).unwrap();
        assert_eq!(manifest.events.len(), 3);
        assert_eq!(manifest.events[0].handler_type, "command");
        assert_eq!(manifest.events[1].handler_type, "script");
        assert_eq!(manifest.events[2].handler_type, "webhook");
        assert_eq!(
            manifest.events[2].url,
            Some("https://hooks.example.com/notify".to_string())
        );
    }

    #[test]
    fn test_tool_parameter_types() {
        let content = r#"
[extension]
id = "param-types"
name = "Parameter Types"
version = "1.0.0"
description = "Test parameter types"

[[tools]]
name = "typed_tool"
description = "Tool with various param types"

[[tools.parameters]]
name = "str_param"
type = "string"
description = "A string parameter"
required = true

[[tools.parameters]]
name = "num_param"
type = "number"
required = false
default = 0

[[tools.parameters]]
name = "bool_param"
type = "boolean"
required = false
default = false

[[tools.parameters]]
name = "arr_param"
type = "array"

[[tools.parameters]]
name = "obj_param"
type = "object"
"#;

        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("params.toml")).unwrap();
        let params = &manifest.tools[0].parameters;
        assert_eq!(params.len(), 5);
        assert_eq!(params[0].param_type, "string");
        assert!(params[0].required);
        assert_eq!(params[1].param_type, "number");
        assert!(!params[1].required);
        assert_eq!(params[2].param_type, "boolean");
        assert_eq!(params[3].param_type, "array");
        assert_eq!(params[4].param_type, "object");
    }

    // =========================================================================
    // Legacy format edge cases
    // =========================================================================

    #[test]
    fn test_legacy_with_model_only() {
        let content = r#"
[agent]
name = "model-only"
description = "Agent with model section only"

[model]
harness = "openai"
model = "gpt-4"
"#;

        let manifest =
            ExtensionManifest::from_str(content, &PathBuf::from("model-only.toml")).unwrap();
        assert_eq!(manifest.extension.name, "model-only");
        assert_eq!(
            manifest.config.get("harness"),
            Some(&serde_json::Value::String("openai".to_string()))
        );
        assert!(manifest.config.get("prompt_template").is_none());
    }

    #[test]
    fn test_legacy_with_prompt_only() {
        let content = r#"
[agent]
name = "prompt-only"
description = "Agent with prompt section only"

[prompt]
template = "You are a helpful assistant."
"#;

        let manifest =
            ExtensionManifest::from_str(content, &PathBuf::from("prompt-only.toml")).unwrap();
        assert_eq!(manifest.extension.name, "prompt-only");
        assert_eq!(
            manifest.config.get("prompt_template"),
            Some(&serde_json::Value::String(
                "You are a helpful assistant.".to_string()
            ))
        );
        assert!(manifest.config.get("harness").is_none());
    }

    #[test]
    fn test_legacy_partial_model_section() {
        // Model section with only harness, no model name
        let content = r#"
[agent]
name = "partial-model"
description = "Agent with partial model"

[model]
harness = "anthropic"
"#;

        let manifest =
            ExtensionManifest::from_str(content, &PathBuf::from("partial.toml")).unwrap();
        assert_eq!(
            manifest.config.get("harness"),
            Some(&serde_json::Value::String("anthropic".to_string()))
        );
        assert!(manifest.config.get("model").is_none());
    }

    #[test]
    fn test_legacy_into_manifest_sets_main_from_filename() {
        let content = r#"
[agent]
name = "test-agent"
description = "Test"
"#;

        // Test with different file stems
        let manifest1 =
            ExtensionManifest::from_str(content, &PathBuf::from("custom-agent.toml")).unwrap();
        assert_eq!(
            manifest1.extension.main,
            Some("custom-agent.toml".to_string())
        );

        let manifest2 =
            ExtensionManifest::from_str(content, &PathBuf::from("/path/to/special.toml")).unwrap();
        assert_eq!(manifest2.extension.main, Some("special.toml".to_string()));
    }

    #[test]
    fn test_legacy_id_format() {
        let content = r#"
[agent]
name = "my-special-agent"
description = "Test"
"#;

        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        assert_eq!(manifest.extension.id, "legacy.agent.my-special-agent");
        assert_eq!(manifest.extension.version, "1.0.0");
    }

    // =========================================================================
    // Unicode and special characters
    // =========================================================================

    #[test]
    fn test_unicode_in_extension_fields() {
        let content = r#"
[extension]
id = "unicode-ext"
name = "拡張機能テスト"
version = "1.0.0"
description = "Extension with émojis 🚀 and ünïcödé"
author = "日本語の著者"
"#;

        let manifest =
            ExtensionManifest::from_str(content, &PathBuf::from("unicode.toml")).unwrap();
        assert_eq!(manifest.extension.name, "拡張機能テスト");
        assert!(manifest.extension.description.contains("🚀"));
        assert_eq!(manifest.extension.author, Some("日本語の著者".to_string()));
    }

    #[test]
    fn test_multiline_strings() {
        let content = r#"
[extension]
id = "multiline"
name = "Multiline Test"
version = "1.0.0"
description = """
This is a multiline
description that spans
multiple lines.
"""
"#;

        let manifest =
            ExtensionManifest::from_str(content, &PathBuf::from("multiline.toml")).unwrap();
        assert!(manifest.extension.description.contains("multiline"));
        assert!(manifest.extension.description.contains("\n"));
    }

    #[test]
    fn test_legacy_unicode() {
        let content = r#"
[agent]
name = "unicode-agent"
description = "エージェント説明 with special chars: <>&\""

[prompt]
template = "你好，我是助手。"
"#;

        let manifest =
            ExtensionManifest::from_str(content, &PathBuf::from("unicode.toml")).unwrap();
        assert!(manifest.extension.description.contains("エージェント説明"));
        assert_eq!(
            manifest.config.get("prompt_template"),
            Some(&serde_json::Value::String("你好，我是助手。".to_string()))
        );
    }

    // =========================================================================
    // Error variant coverage
    // =========================================================================

    #[test]
    fn test_extension_error_display() {
        let io_err = ExtensionError::Io("file not found".to_string());
        assert_eq!(io_err.to_string(), "IO error: file not found");

        let parse_err = ExtensionError::Parse("invalid toml".to_string());
        assert_eq!(parse_err.to_string(), "Parse error: invalid toml");

        let validation_err = ExtensionError::Validation("missing field".to_string());
        assert_eq!(
            validation_err.to_string(),
            "Validation error: missing field"
        );

        let not_found_err = ExtensionError::NotFound("my-extension".to_string());
        assert_eq!(
            not_found_err.to_string(),
            "Extension not found: my-extension"
        );
    }

    // =========================================================================
    // Config value types
    // =========================================================================

    #[test]
    fn test_config_various_json_types() {
        let content = r#"
[extension]
id = "config-types"
name = "Config Types"
version = "1.0.0"
description = "Test various config types"

[config]
string_val = "hello"
int_val = 42
float_val = 3.14
bool_val = true
array_val = [1, 2, 3]
nested = { key = "value", num = 10 }
"#;

        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("config.toml")).unwrap();

        assert_eq!(
            manifest.config.get("string_val"),
            Some(&serde_json::json!("hello"))
        );
        assert_eq!(manifest.config.get("int_val"), Some(&serde_json::json!(42)));
        assert_eq!(
            manifest.config.get("float_val"),
            Some(&serde_json::json!(3.14))
        );
        assert_eq!(
            manifest.config.get("bool_val"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            manifest.config.get("array_val"),
            Some(&serde_json::json!([1, 2, 3]))
        );

        let nested = manifest.config.get("nested").unwrap();
        assert_eq!(nested.get("key"), Some(&serde_json::json!("value")));
        assert_eq!(nested.get("num"), Some(&serde_json::json!(10)));
    }

    // =========================================================================
    // Path edge cases for legacy conversion
    // =========================================================================

    #[test]
    fn test_legacy_path_without_extension() {
        let content = r#"
[agent]
name = "no-ext"
description = "Test"
"#;

        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("agentfile")).unwrap();
        assert_eq!(manifest.extension.main, Some("agentfile.toml".to_string()));
    }

    #[test]
    fn test_legacy_path_empty_filename() {
        let content = r#"
[agent]
name = "empty-path"
description = "Test"
"#;

        // Path with no file stem should use "unknown"
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("")).unwrap();
        assert_eq!(manifest.extension.main, Some("unknown.toml".to_string()));
    }

    // =========================================================================
    // Validation tests
    // =========================================================================

    #[test]
    fn test_validate_valid_manifest() {
        let content = r#"
[extension]
id = "valid-ext"
name = "Valid Extension"
version = "1.0.0"
description = "A valid extension"
"#;
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_id() {
        let content = r#"
[extension]
id = ""
name = "Test"
version = "1.0.0"
description = "Test"
"#;
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("id cannot be empty"));
    }

    #[test]
    fn test_validate_invalid_id_chars() {
        let content = r#"
[extension]
id = "invalid@id"
name = "Test"
version = "1.0.0"
description = "Test"
"#;
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("invalid characters"));
    }

    #[test]
    fn test_validate_valid_id_formats() {
        let valid_ids = [
            "my-extension",
            "my_extension",
            "my.extension",
            "ext123",
            "a.b.c-d_e",
        ];
        for id in valid_ids {
            let content = format!(
                r#"
[extension]
id = "{}"
name = "Test"
version = "1.0.0"
description = "Test"
"#,
                id
            );
            let manifest =
                ExtensionManifest::from_str(&content, &PathBuf::from("test.toml")).unwrap();
            assert!(manifest.validate().is_ok(), "ID '{}' should be valid", id);
        }
    }

    #[test]
    fn test_validate_invalid_version() {
        let content = r#"
[extension]
id = "test"
name = "Test"
version = "not-a-version"
description = "Test"
"#;
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("not a valid semantic version"));
    }

    #[test]
    fn test_validate_valid_versions() {
        let valid_versions = [
            "1.0.0",
            "0.1.0",
            "10.20.30",
            "1.0",
            "1.0.0-beta",
            "2.0.0-rc.1",
        ];
        for version in valid_versions {
            let content = format!(
                r#"
[extension]
id = "test"
name = "Test"
version = "{}"
description = "Test"
"#,
                version
            );
            let manifest =
                ExtensionManifest::from_str(&content, &PathBuf::from("test.toml")).unwrap();
            assert!(
                manifest.validate().is_ok(),
                "Version '{}' should be valid",
                version
            );
        }
    }

    #[test]
    fn test_validate_tool_empty_name() {
        let content = r#"
[extension]
id = "test"
name = "Test"
version = "1.0.0"
description = "Test"

[[tools]]
name = ""
description = "A tool"
"#;
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("tools[0].name cannot be empty"));
    }

    #[test]
    fn test_validate_tool_invalid_param_type() {
        let content = r#"
[extension]
id = "test"
name = "Test"
version = "1.0.0"
description = "Test"

[[tools]]
name = "my_tool"
description = "A tool"

[[tools.parameters]]
name = "param"
type = "invalid_type"
"#;
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("is not valid"));
    }

    #[test]
    fn test_validate_event_invalid_type() {
        let content = r#"
[extension]
id = "test"
name = "Test"
version = "1.0.0"
description = "Test"

[[events]]
event = "task.start"
type = "invalid"
"#;
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("is not valid"));
    }

    #[test]
    fn test_validate_event_command_missing_command() {
        let content = r#"
[extension]
id = "test"
name = "Test"
version = "1.0.0"
description = "Test"

[[events]]
event = "task.start"
type = "command"
"#;
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("no command specified"));
    }

    #[test]
    fn test_validate_event_script_missing_script() {
        let content = r#"
[extension]
id = "test"
name = "Test"
version = "1.0.0"
description = "Test"

[[events]]
event = "task.start"
type = "script"
"#;
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("no script specified"));
    }

    #[test]
    fn test_validate_event_webhook_missing_url() {
        let content = r#"
[extension]
id = "test"
name = "Test"
version = "1.0.0"
description = "Test"

[[events]]
event = "task.start"
type = "webhook"
"#;
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("no url specified"));
    }

    #[test]
    fn test_validate_multiple_errors() {
        let content = r#"
[extension]
id = ""
name = ""
version = ""
description = ""
"#;
        let manifest = ExtensionManifest::from_str(content, &PathBuf::from("test.toml")).unwrap();
        let err = manifest.validate().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("id cannot be empty"));
        assert!(msg.contains("name cannot be empty"));
        assert!(msg.contains("version cannot be empty"));
        assert!(msg.contains("description cannot be empty"));
    }

    // =========================================================================
    // Discovery tests
    // =========================================================================

    #[test]
    fn test_discovery_options_standard() {
        let opts = DiscoveryOptions::standard();
        assert_eq!(opts.max_depth, Some(10));
        assert!(opts.include_legacy);
        assert!(!opts.follow_symlinks);
        assert!(opts.validate);
        assert!(!opts.skip_errors);
    }

    #[test]
    fn test_discovery_options_lenient() {
        let opts = DiscoveryOptions::lenient();
        assert!(!opts.validate);
        assert!(opts.skip_errors);
    }

    #[test]
    fn test_discover_nonexistent_directory() {
        let result = discover(Path::new("/nonexistent/path"), DiscoveryOptions::default());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExtensionError::Io(_)));
    }

    #[test]
    fn test_discover_file_not_directory() {
        let temp = NamedTempFile::new().unwrap();
        let result = discover(temp.path(), DiscoveryOptions::default());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExtensionError::Io(_)));
        assert!(err.to_string().contains("not a directory"));
    }

    #[test]
    fn test_discover_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let result = discover(dir.path(), DiscoveryOptions::default()).unwrap();
        assert!(result.is_empty());
        assert_eq!(result.count(), 0);
    }

    #[test]
    fn test_discover_extension_toml() {
        let dir = tempfile::tempdir().unwrap();

        // Create extension.toml
        let ext_content = r#"
[extension]
id = "discovered-ext"
name = "Discovered Extension"
version = "1.0.0"
description = "Found via discovery"
"#;
        std::fs::write(dir.path().join("extension.toml"), ext_content).unwrap();

        let result = discover(dir.path(), DiscoveryOptions::standard()).unwrap();
        assert_eq!(result.count(), 1);
        assert_eq!(result.extensions[0].manifest.extension.id, "discovered-ext");
        assert!(!result.extensions[0].is_legacy);
    }

    #[test]
    fn test_discover_legacy_agent() {
        let dir = tempfile::tempdir().unwrap();

        // Create spawn-agents directory with legacy agent
        let agents_dir = dir.path().join("spawn-agents");
        std::fs::create_dir(&agents_dir).unwrap();

        let legacy_content = r#"
[agent]
name = "legacy-agent"
description = "A legacy agent"
"#;
        std::fs::write(agents_dir.join("agent.toml"), legacy_content).unwrap();

        let opts = DiscoveryOptions {
            include_legacy: true,
            ..DiscoveryOptions::default()
        };
        let result = discover(dir.path(), opts).unwrap();
        assert_eq!(result.count(), 1);
        assert!(result.extensions[0].is_legacy);
        assert_eq!(result.extensions[0].manifest.extension.name, "legacy-agent");
    }

    #[test]
    fn test_discover_skip_legacy_when_disabled() {
        let dir = tempfile::tempdir().unwrap();

        // Create spawn-agents directory
        let agents_dir = dir.path().join("spawn-agents");
        std::fs::create_dir(&agents_dir).unwrap();
        std::fs::write(
            agents_dir.join("agent.toml"),
            r#"[agent]
name = "legacy"
description = "test""#,
        )
        .unwrap();

        let opts = DiscoveryOptions {
            include_legacy: false,
            ..DiscoveryOptions::default()
        };
        let result = discover(dir.path(), opts).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_discover_nested_extensions() {
        let dir = tempfile::tempdir().unwrap();

        // Create nested structure
        let ext1 = dir.path().join("ext1");
        let ext2 = dir.path().join("subdir").join("ext2");
        std::fs::create_dir_all(&ext1).unwrap();
        std::fs::create_dir_all(&ext2).unwrap();

        std::fs::write(
            ext1.join("extension.toml"),
            r#"[extension]
id = "ext1"
name = "Extension 1"
version = "1.0.0"
description = "First"
"#,
        )
        .unwrap();

        std::fs::write(
            ext2.join("extension.toml"),
            r#"[extension]
id = "ext2"
name = "Extension 2"
version = "2.0.0"
description = "Second"
"#,
        )
        .unwrap();

        let result = discover(dir.path(), DiscoveryOptions::standard()).unwrap();
        assert_eq!(result.count(), 2);

        let ids: Vec<_> = result
            .extensions
            .iter()
            .map(|e| e.manifest.extension.id.as_str())
            .collect();
        assert!(ids.contains(&"ext1"));
        assert!(ids.contains(&"ext2"));
    }

    #[test]
    fn test_discover_max_depth() {
        let dir = tempfile::tempdir().unwrap();

        // Create deeply nested extension
        let deep_dir = dir.path().join("a").join("b").join("c").join("d");
        std::fs::create_dir_all(&deep_dir).unwrap();
        std::fs::write(
            deep_dir.join("extension.toml"),
            r#"[extension]
id = "deep"
name = "Deep Extension"
version = "1.0.0"
description = "Deeply nested"
"#,
        )
        .unwrap();

        // With max_depth=2, should not find it
        let opts = DiscoveryOptions {
            max_depth: Some(2),
            ..DiscoveryOptions::default()
        };
        let result = discover(dir.path(), opts).unwrap();
        assert!(result.is_empty());

        // With max_depth=5, should find it
        let opts = DiscoveryOptions {
            max_depth: Some(5),
            ..DiscoveryOptions::default()
        };
        let result = discover(dir.path(), opts).unwrap();
        assert_eq!(result.count(), 1);
    }

    #[test]
    fn test_discover_duplicate_ids_error() {
        let dir = tempfile::tempdir().unwrap();

        let ext1 = dir.path().join("ext1");
        let ext2 = dir.path().join("ext2");
        std::fs::create_dir_all(&ext1).unwrap();
        std::fs::create_dir_all(&ext2).unwrap();

        // Same ID in both
        let content = r#"[extension]
id = "duplicate-id"
name = "Extension"
version = "1.0.0"
description = "Test"
"#;
        std::fs::write(ext1.join("extension.toml"), content).unwrap();
        std::fs::write(ext2.join("extension.toml"), content).unwrap();

        // Without skip_errors, should error
        let opts = DiscoveryOptions {
            skip_errors: false,
            ..DiscoveryOptions::default()
        };
        let result = discover(dir.path(), opts);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExtensionError::DuplicateId(_)));
    }

    #[test]
    fn test_discover_duplicate_ids_skip() {
        let dir = tempfile::tempdir().unwrap();

        let ext1 = dir.path().join("ext1");
        let ext2 = dir.path().join("ext2");
        std::fs::create_dir_all(&ext1).unwrap();
        std::fs::create_dir_all(&ext2).unwrap();

        let content = r#"[extension]
id = "duplicate-id"
name = "Extension"
version = "1.0.0"
description = "Test"
"#;
        std::fs::write(ext1.join("extension.toml"), content).unwrap();
        std::fs::write(ext2.join("extension.toml"), content).unwrap();

        // With skip_errors, should succeed but record error
        let opts = DiscoveryOptions {
            skip_errors: true,
            ..DiscoveryOptions::default()
        };
        let result = discover(dir.path(), opts).unwrap();
        assert_eq!(result.count(), 1); // Only first one loaded
        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_discover_invalid_manifest_skip() {
        let dir = tempfile::tempdir().unwrap();

        // Valid extension
        let valid_dir = dir.path().join("valid");
        std::fs::create_dir_all(&valid_dir).unwrap();
        std::fs::write(
            valid_dir.join("extension.toml"),
            r#"[extension]
id = "valid"
name = "Valid"
version = "1.0.0"
description = "Valid ext"
"#,
        )
        .unwrap();

        // Invalid extension (bad TOML)
        let invalid_dir = dir.path().join("invalid");
        std::fs::create_dir_all(&invalid_dir).unwrap();
        std::fs::write(invalid_dir.join("extension.toml"), "invalid [[[ toml").unwrap();

        let opts = DiscoveryOptions {
            skip_errors: true,
            validate: true,
            ..DiscoveryOptions::default()
        };
        let result = discover(dir.path(), opts).unwrap();
        assert_eq!(result.count(), 1);
        assert!(result.has_errors());
        assert_eq!(result.skipped.len(), 1);
    }

    #[test]
    fn test_discover_validation_failure_skip() {
        let dir = tempfile::tempdir().unwrap();

        // Invalid extension (empty ID)
        std::fs::write(
            dir.path().join("extension.toml"),
            r#"[extension]
id = ""
name = "Test"
version = "1.0.0"
description = "Test"
"#,
        )
        .unwrap();

        let opts = DiscoveryOptions {
            skip_errors: true,
            validate: true,
            ..DiscoveryOptions::default()
        };
        let result = discover(dir.path(), opts).unwrap();
        assert!(result.is_empty());
        assert!(result.has_errors());
    }

    #[test]
    fn test_discovery_result_methods() {
        let mut result = DiscoveryResult::default();
        assert!(result.is_empty());
        assert!(!result.has_errors());
        assert_eq!(result.count(), 0);

        result.errors.push(ExtensionError::Io("test".to_string()));
        assert!(result.has_errors());
    }

    // =========================================================================
    // discover_all tests
    // =========================================================================

    #[test]
    fn test_discover_all_multiple_roots() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();

        std::fs::write(
            dir1.path().join("extension.toml"),
            r#"[extension]
id = "ext1"
name = "Ext 1"
version = "1.0.0"
description = "First"
"#,
        )
        .unwrap();

        std::fs::write(
            dir2.path().join("extension.toml"),
            r#"[extension]
id = "ext2"
name = "Ext 2"
version = "2.0.0"
description = "Second"
"#,
        )
        .unwrap();

        let roots = [dir1.path(), dir2.path()];
        let result = discover_all(&roots, DiscoveryOptions::standard()).unwrap();
        assert_eq!(result.count(), 2);
    }

    #[test]
    fn test_discover_all_skips_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("extension.toml"),
            r#"[extension]
id = "ext"
name = "Ext"
version = "1.0.0"
description = "Test"
"#,
        )
        .unwrap();

        let roots = [dir.path(), Path::new("/nonexistent/path")];
        let result = discover_all(&roots, DiscoveryOptions::standard()).unwrap();
        assert_eq!(result.count(), 1);
    }

    // =========================================================================
    // Registry tests
    // =========================================================================

    #[test]
    fn test_registry_new() {
        let registry = ExtensionRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_registry_load_from_discovery() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("extension.toml"),
            r#"[extension]
id = "test-ext"
name = "Test Extension"
version = "1.0.0"
description = "For testing"
"#,
        )
        .unwrap();

        let result = discover(dir.path(), DiscoveryOptions::standard()).unwrap();
        let mut registry = ExtensionRegistry::new();
        registry.load_from_discovery(result);

        assert!(!registry.is_empty());
        assert_eq!(registry.count(), 1);
        assert!(registry.has("test-ext"));
    }

    #[test]
    fn test_registry_get() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("extension.toml"),
            r#"[extension]
id = "my-ext"
name = "My Extension"
version = "1.0.0"
description = "Testing"
"#,
        )
        .unwrap();

        let mut registry = ExtensionRegistry::new();
        registry
            .discover_and_load(dir.path(), DiscoveryOptions::standard())
            .unwrap();

        let ext = registry.get("my-ext").unwrap();
        assert_eq!(ext.manifest.extension.name, "My Extension");

        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_get_or_error() {
        let registry = ExtensionRegistry::new();
        let result = registry.get_or_error("missing");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExtensionError::NotFound(_)));
    }

    #[test]
    fn test_registry_list_ids() {
        let dir = tempfile::tempdir().unwrap();

        let ext1 = dir.path().join("ext1");
        let ext2 = dir.path().join("ext2");
        std::fs::create_dir_all(&ext1).unwrap();
        std::fs::create_dir_all(&ext2).unwrap();

        std::fs::write(
            ext1.join("extension.toml"),
            r#"[extension]
id = "alpha"
name = "Alpha"
version = "1.0.0"
description = "First"
"#,
        )
        .unwrap();

        std::fs::write(
            ext2.join("extension.toml"),
            r#"[extension]
id = "beta"
name = "Beta"
version = "1.0.0"
description = "Second"
"#,
        )
        .unwrap();

        let mut registry = ExtensionRegistry::new();
        registry
            .discover_and_load(dir.path(), DiscoveryOptions::standard())
            .unwrap();

        let ids = registry.list_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"alpha"));
        assert!(ids.contains(&"beta"));
    }

    #[test]
    fn test_registry_remove() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("extension.toml"),
            r#"[extension]
id = "to-remove"
name = "To Remove"
version = "1.0.0"
description = "Test"
"#,
        )
        .unwrap();

        let mut registry = ExtensionRegistry::new();
        registry
            .discover_and_load(dir.path(), DiscoveryOptions::standard())
            .unwrap();

        assert!(registry.has("to-remove"));
        let removed = registry.remove("to-remove");
        assert!(removed.is_some());
        assert!(!registry.has("to-remove"));
    }

    #[test]
    fn test_registry_clear() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("extension.toml"),
            r#"[extension]
id = "test"
name = "Test"
version = "1.0.0"
description = "Test"
"#,
        )
        .unwrap();

        let mut registry = ExtensionRegistry::new();
        registry
            .discover_and_load(dir.path(), DiscoveryOptions::standard())
            .unwrap();

        assert!(!registry.is_empty());
        registry.clear();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_filter_legacy() {
        let dir = tempfile::tempdir().unwrap();

        // Modern extension
        let modern_dir = dir.path().join("modern");
        std::fs::create_dir_all(&modern_dir).unwrap();
        std::fs::write(
            modern_dir.join("extension.toml"),
            r#"[extension]
id = "modern"
name = "Modern"
version = "1.0.0"
description = "Modern ext"
"#,
        )
        .unwrap();

        // Legacy extension
        let legacy_dir = dir.path().join("spawn-agents");
        std::fs::create_dir_all(&legacy_dir).unwrap();
        std::fs::write(
            legacy_dir.join("agent.toml"),
            r#"[agent]
name = "legacy-agent"
description = "Legacy"
"#,
        )
        .unwrap();

        let mut registry = ExtensionRegistry::new();
        registry
            .discover_and_load(dir.path(), DiscoveryOptions::standard())
            .unwrap();

        let legacy = registry.legacy_extensions();
        let modern = registry.modern_extensions();

        assert_eq!(legacy.len(), 1);
        assert_eq!(modern.len(), 1);
        assert!(legacy[0].is_legacy);
        assert!(!modern[0].is_legacy);
    }

    #[test]
    fn test_registry_filter_custom() {
        let dir = tempfile::tempdir().unwrap();

        let ext1 = dir.path().join("ext1");
        let ext2 = dir.path().join("ext2");
        std::fs::create_dir_all(&ext1).unwrap();
        std::fs::create_dir_all(&ext2).unwrap();

        std::fs::write(
            ext1.join("extension.toml"),
            r#"[extension]
id = "v1-ext"
name = "V1 Extension"
version = "1.0.0"
description = "Version 1"
"#,
        )
        .unwrap();

        std::fs::write(
            ext2.join("extension.toml"),
            r#"[extension]
id = "v2-ext"
name = "V2 Extension"
version = "2.0.0"
description = "Version 2"
"#,
        )
        .unwrap();

        let mut registry = ExtensionRegistry::new();
        registry
            .discover_and_load(dir.path(), DiscoveryOptions::standard())
            .unwrap();

        // Filter by version starting with "2"
        let v2_exts = registry.filter(|e| e.manifest.extension.version.starts_with("2"));
        assert_eq!(v2_exts.len(), 1);
        assert_eq!(v2_exts[0].manifest.extension.id, "v2-ext");
    }

    #[test]
    fn test_discovered_extension_fields() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("extension.toml"),
            r#"[extension]
id = "field-test"
name = "Field Test"
version = "1.0.0"
description = "Testing fields"
"#,
        )
        .unwrap();

        let result = discover(dir.path(), DiscoveryOptions::standard()).unwrap();
        let ext = &result.extensions[0];

        assert_eq!(ext.manifest.extension.id, "field-test");
        assert_eq!(ext.path, dir.path().join("extension.toml"));
        assert_eq!(ext.directory, dir.path());
        assert!(!ext.is_legacy);
    }
}
