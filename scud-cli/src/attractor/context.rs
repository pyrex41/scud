//! Thread-safe key-value execution context for pipeline runs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Thread-safe key-value context shared across pipeline execution.
///
/// Stores arbitrary JSON values keyed by string names. Supports
/// isolated clones for parallel branches and atomic update application.
#[derive(Debug, Clone)]
pub struct Context {
    inner: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl Context {
    /// Create an empty context.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a context with initial values.
    pub fn with_values(values: HashMap<String, serde_json::Value>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(values)),
        }
    }

    /// Set a value in the context.
    pub async fn set(&self, key: impl Into<String>, value: serde_json::Value) {
        self.inner.write().await.insert(key.into(), value);
    }

    /// Get a value from the context.
    pub async fn get(&self, key: &str) -> Option<serde_json::Value> {
        self.inner.read().await.get(key).cloned()
    }

    /// Get a string value from the context.
    pub async fn get_str(&self, key: &str) -> Option<String> {
        self.get(key)
            .await
            .and_then(|v| v.as_str().map(String::from))
    }

    /// Take a snapshot of the current context state.
    pub async fn snapshot(&self) -> HashMap<String, serde_json::Value> {
        self.inner.read().await.clone()
    }

    /// Create an isolated clone for parallel branches.
    ///
    /// Changes to the clone do not affect the original.
    pub async fn clone_isolated(&self) -> Self {
        Self::with_values(self.snapshot().await)
    }

    /// Apply a batch of updates atomically.
    pub async fn apply_updates(&self, updates: &HashMap<String, serde_json::Value>) {
        let mut inner = self.inner.write().await;
        for (key, value) in updates {
            inner.insert(key.clone(), value.clone());
        }
    }

    /// Check if the context contains a key.
    pub async fn contains_key(&self, key: &str) -> bool {
        self.inner.read().await.contains_key(key)
    }

    /// Get the number of entries.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    /// Check if the context is empty.
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializable snapshot of a context for checkpointing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub values: HashMap<String, serde_json::Value>,
}

impl From<HashMap<String, serde_json::Value>> for ContextSnapshot {
    fn from(values: HashMap<String, serde_json::Value>) -> Self {
        Self { values }
    }
}

impl ContextSnapshot {
    /// Restore a Context from this snapshot.
    pub fn restore(&self) -> Context {
        Context::with_values(self.values.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get() {
        let ctx = Context::new();
        ctx.set("name", serde_json::json!("Alice")).await;
        assert_eq!(
            ctx.get("name").await,
            Some(serde_json::json!("Alice"))
        );
    }

    #[tokio::test]
    async fn test_get_str() {
        let ctx = Context::new();
        ctx.set("greeting", serde_json::json!("hello")).await;
        assert_eq!(ctx.get_str("greeting").await, Some("hello".to_string()));

        ctx.set("number", serde_json::json!(42)).await;
        assert_eq!(ctx.get_str("number").await, None);
    }

    #[tokio::test]
    async fn test_snapshot() {
        let ctx = Context::new();
        ctx.set("a", serde_json::json!(1)).await;
        ctx.set("b", serde_json::json!(2)).await;
        let snap = ctx.snapshot().await;
        assert_eq!(snap.len(), 2);
    }

    #[tokio::test]
    async fn test_clone_isolated() {
        let ctx = Context::new();
        ctx.set("shared", serde_json::json!("original")).await;

        let clone = ctx.clone_isolated().await;
        clone.set("shared", serde_json::json!("modified")).await;

        // Original should be unchanged
        assert_eq!(
            ctx.get_str("shared").await,
            Some("original".to_string())
        );
        assert_eq!(
            clone.get_str("shared").await,
            Some("modified".to_string())
        );
    }

    #[tokio::test]
    async fn test_apply_updates() {
        let ctx = Context::new();
        let mut updates = HashMap::new();
        updates.insert("x".into(), serde_json::json!(10));
        updates.insert("y".into(), serde_json::json!(20));
        ctx.apply_updates(&updates).await;
        assert_eq!(ctx.len().await, 2);
    }

    #[tokio::test]
    async fn test_snapshot_roundtrip() {
        let ctx = Context::new();
        ctx.set("key", serde_json::json!("value")).await;
        let snap = ContextSnapshot::from(ctx.snapshot().await);
        let json = serde_json::to_string(&snap).unwrap();
        let restored_snap: ContextSnapshot = serde_json::from_str(&json).unwrap();
        let restored = restored_snap.restore();
        assert_eq!(
            restored.get_str("key").await,
            Some("value".to_string())
        );
    }
}
