//! Filesystem layout for pipeline runs.
//!
//! ```text
//! runs/{run_id}/
//!     checkpoint.json
//!     manifest.json
//!     {node_id}/
//!         status.json
//!         prompt.md
//!         response.md
//!     artifacts/
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Manages the filesystem layout for a single pipeline run.
#[derive(Debug, Clone)]
pub struct RunDirectory {
    root: PathBuf,
}

/// Manifest for a pipeline run.
#[derive(Debug, Serialize, Deserialize)]
pub struct RunManifest {
    pub run_id: String,
    pub pipeline_name: String,
    pub pipeline_file: String,
    pub started_at: String,
    pub status: String,
}

impl RunDirectory {
    /// Create a new run directory.
    ///
    /// Creates the directory structure if it doesn't exist.
    pub fn create(base_dir: &Path, run_id: &str) -> Result<Self> {
        let root = base_dir.join("runs").join(run_id);
        std::fs::create_dir_all(&root).context("Failed to create run directory")?;
        std::fs::create_dir_all(root.join("artifacts"))
            .context("Failed to create artifacts directory")?;
        Ok(Self { root })
    }

    /// Open an existing run directory.
    pub fn open(base_dir: &Path, run_id: &str) -> Result<Self> {
        let root = base_dir.join("runs").join(run_id);
        if !root.exists() {
            anyhow::bail!("Run directory not found: {}", root.display());
        }
        Ok(Self { root })
    }

    /// Get the root path of this run directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the checkpoint file path.
    pub fn checkpoint_path(&self) -> PathBuf {
        self.root.join("checkpoint.json")
    }

    /// Get the manifest file path.
    pub fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }

    /// Get the node directory path, creating it if needed.
    pub fn node_dir(&self, node_id: &str) -> Result<PathBuf> {
        let dir = self.root.join(sanitize_id(node_id));
        std::fs::create_dir_all(&dir)
            .context(format!("Failed to create node directory for '{}'", node_id))?;
        Ok(dir)
    }

    /// Write the prompt for a node.
    pub fn write_prompt(&self, node_id: &str, prompt: &str) -> Result<()> {
        let dir = self.node_dir(node_id)?;
        std::fs::write(dir.join("prompt.md"), prompt)
            .context("Failed to write prompt file")?;
        Ok(())
    }

    /// Write the response for a node.
    pub fn write_response(&self, node_id: &str, response: &str) -> Result<()> {
        let dir = self.node_dir(node_id)?;
        std::fs::write(dir.join("response.md"), response)
            .context("Failed to write response file")?;
        Ok(())
    }

    /// Write node status.
    pub fn write_status(&self, node_id: &str, status: &serde_json::Value) -> Result<()> {
        let dir = self.node_dir(node_id)?;
        let json = serde_json::to_string_pretty(status)?;
        std::fs::write(dir.join("status.json"), json)
            .context("Failed to write status file")?;
        Ok(())
    }

    /// Write the run manifest.
    pub fn write_manifest(&self, manifest: &RunManifest) -> Result<()> {
        let json = serde_json::to_string_pretty(manifest)?;
        std::fs::write(self.manifest_path(), json)
            .context("Failed to write manifest")?;
        Ok(())
    }

    /// Read the run manifest.
    pub fn read_manifest(&self) -> Result<RunManifest> {
        let json = std::fs::read_to_string(self.manifest_path())
            .context("Failed to read manifest")?;
        serde_json::from_str(&json).context("Failed to parse manifest")
    }

    /// Write an artifact file.
    pub fn write_artifact(&self, name: &str, content: &[u8]) -> Result<PathBuf> {
        let path = self.root.join("artifacts").join(name);
        std::fs::write(&path, content).context("Failed to write artifact")?;
        Ok(path)
    }

    /// Read the response for a node.
    pub fn read_response(&self, node_id: &str) -> Result<String> {
        let path = self.root.join(sanitize_id(node_id)).join("response.md");
        std::fs::read_to_string(&path).context("Failed to read response")
    }
}

/// Sanitize a node ID for use as a directory name.
fn sanitize_id(id: &str) -> String {
    id.replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_directory_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let run = RunDirectory::create(dir.path(), "test-run-001").unwrap();

        assert!(run.root().exists());
        assert!(run.root().join("artifacts").exists());

        // Write and read prompt/response
        run.write_prompt("task_a", "Do the thing").unwrap();
        run.write_response("task_a", "I did the thing").unwrap();
        assert_eq!(run.read_response("task_a").unwrap(), "I did the thing");

        // Write manifest
        let manifest = RunManifest {
            run_id: "test-run-001".into(),
            pipeline_name: "test".into(),
            pipeline_file: "test.dot".into(),
            started_at: "2025-01-01T00:00:00Z".into(),
            status: "running".into(),
        };
        run.write_manifest(&manifest).unwrap();
        let loaded = run.read_manifest().unwrap();
        assert_eq!(loaded.run_id, "test-run-001");

        // Open existing
        let opened = RunDirectory::open(dir.path(), "test-run-001").unwrap();
        assert!(opened.root().exists());
    }

    #[test]
    fn test_sanitize_id() {
        assert_eq!(sanitize_id("simple"), "simple");
        assert_eq!(sanitize_id("with spaces"), "with_spaces");
        assert_eq!(sanitize_id("node-1"), "node-1");
        assert_eq!(sanitize_id("a/b.c"), "a_b_c");
    }
}
