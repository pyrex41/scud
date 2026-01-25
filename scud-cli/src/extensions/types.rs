use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for an extension
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtensionId(pub String);

impl fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ExtensionId {
    fn from(s: String) -> Self {
        ExtensionId(s)
    }
}

impl From<&str> for ExtensionId {
    fn from(s: &str) -> Self {
        ExtensionId(s.to_string())
    }
}

/// Function signature for extension-provided tools
pub type ToolFn =
    fn(&[serde_json::Value]) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>;
