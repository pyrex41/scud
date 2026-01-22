//! File-based task storage with locking.
//!
//! Manages reading and writing of task data to the filesystem with:
//! - File locking for safe concurrent access
//! - Caching of active phase selection
//! - Atomic read-modify-write operations

use anyhow::{Context, Result};
use fs2::FileExt;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::thread;
use std::time::Duration;

use crate::formats::{parse_scg, serialize_scg};
use crate::models::{Phase, TaskStatus};

/// File-based storage for SCUD tasks
pub struct Storage {
    project_root: PathBuf,
    /// Cache for active group to avoid repeated file reads
    /// Option<Option<String>> represents: None = not cached, Some(None) = no active group, Some(Some(tag)) = cached tag
    /// Uses RwLock for thread safety
    active_group_cache: RwLock<Option<Option<String>>>,
}

impl Storage {
    pub fn new(project_root: Option<PathBuf>) -> Self {
        let root = project_root.unwrap_or_else(|| std::env::current_dir().unwrap());
        Storage {
            project_root: root,
            active_group_cache: RwLock::new(None),
        }
    }

    /// Get the project root directory
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Acquire an exclusive file lock with retry logic
    fn acquire_lock_with_retry(&self, file: &File, max_retries: u32) -> Result<()> {
        let mut retries = 0;
        let mut delay_ms = 10;

        loop {
            match file.try_lock_exclusive() {
                Ok(_) => return Ok(()),
                Err(_) if retries < max_retries => {
                    retries += 1;
                    thread::sleep(Duration::from_millis(delay_ms));
                    delay_ms = (delay_ms * 2).min(1000); // Exponential backoff, max 1s
                }
                Err(e) => {
                    anyhow::bail!(
                        "Failed to acquire file lock after {} retries: {}",
                        max_retries,
                        e
                    )
                }
            }
        }
    }

    /// Perform a locked write operation on a file
    fn write_with_lock<F>(&self, path: &Path, writer: F) -> Result<()>
    where
        F: FnOnce() -> Result<String>,
    {
        use std::io::Write;

        let dir = path.parent().unwrap();
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }

        // Open file for writing
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .with_context(|| format!("Failed to open file for writing: {}", path.display()))?;

        // Acquire lock with retry
        self.acquire_lock_with_retry(&file, 10)?;

        // Generate content and write through the locked handle
        let content = writer()?;
        file.write_all(content.as_bytes())
            .with_context(|| format!("Failed to write to {}", path.display()))?;
        file.flush()
            .with_context(|| format!("Failed to flush {}", path.display()))?;

        // Lock is automatically released when file is dropped
        Ok(())
    }

    /// Perform a locked read operation on a file
    fn read_with_lock(&self, path: &Path) -> Result<String> {
        use std::io::Read;

        if !path.exists() {
            anyhow::bail!("File not found: {}", path.display());
        }

        // Open file for reading
        let mut file = OpenOptions::new()
            .read(true)
            .open(path)
            .with_context(|| format!("Failed to open file for reading: {}", path.display()))?;

        // Acquire shared lock (allows multiple readers)
        file.lock_shared()
            .with_context(|| format!("Failed to acquire read lock on {}", path.display()))?;

        // Read content through the locked handle
        let mut content = String::new();
        file.read_to_string(&mut content)
            .with_context(|| format!("Failed to read from {}", path.display()))?;

        // Lock is automatically released when file is dropped
        Ok(content)
    }

    pub fn scud_dir(&self) -> PathBuf {
        self.project_root.join(".scud")
    }

    /// Get the path to the tasks file
    pub fn tasks_file(&self) -> PathBuf {
        self.scud_dir().join("tasks").join("tasks.scg")
    }

    /// Get the path to the active tag file
    fn active_tag_file(&self) -> PathBuf {
        self.scud_dir().join("active-tag")
    }

    pub fn is_initialized(&self) -> bool {
        self.scud_dir().exists() && self.tasks_file().exists()
    }

    /// Initialize storage directories (minimal version without config)
    pub fn initialize_dirs(&self) -> Result<()> {
        let scud_dir = self.scud_dir();
        fs::create_dir_all(scud_dir.join("tasks"))
            .context("Failed to create .scud/tasks directory")?;
        Ok(())
    }

    pub fn load_tasks(&self) -> Result<HashMap<String, Phase>> {
        let path = self.tasks_file();
        if !path.exists() {
            anyhow::bail!("Tasks file not found: {}\nRun: scud init", path.display());
        }

        let content = self.read_with_lock(&path)?;
        self.parse_multi_phase_scg(&content)
    }

    /// Parse multi-phase SCG format (multiple phases separated by ---)
    fn parse_multi_phase_scg(&self, content: &str) -> Result<HashMap<String, Phase>> {
        let mut phases = HashMap::new();

        // Empty file returns empty map
        if content.trim().is_empty() {
            return Ok(phases);
        }

        // Split by phase separator (---)
        let sections: Vec<&str> = content.split("\n---\n").collect();

        for section in sections {
            let section = section.trim();
            if section.is_empty() {
                continue;
            }

            // Parse the phase section
            let phase = parse_scg(section).with_context(|| "Failed to parse SCG section")?;

            phases.insert(phase.name.clone(), phase);
        }

        Ok(phases)
    }

    pub fn save_tasks(&self, tasks: &HashMap<String, Phase>) -> Result<()> {
        let path = self.tasks_file();
        self.write_with_lock(&path, || {
            // Sort phases by tag for consistent output
            let mut sorted_tags: Vec<_> = tasks.keys().collect();
            sorted_tags.sort();

            let mut output = String::new();
            for (i, tag) in sorted_tags.iter().enumerate() {
                if i > 0 {
                    output.push_str("\n---\n\n");
                }
                let phase = tasks.get(*tag).unwrap();
                output.push_str(&serialize_scg(phase));
            }

            Ok(output)
        })
    }

    pub fn get_active_group(&self) -> Result<Option<String>> {
        // Check cache first (read lock)
        {
            let cache = self.active_group_cache.read().unwrap();
            if let Some(cached) = cache.as_ref() {
                return Ok(cached.clone());
            }
        }

        // Load from active-tag file
        let active_tag_path = self.active_tag_file();
        let active = if active_tag_path.exists() {
            let content = fs::read_to_string(&active_tag_path)
                .with_context(|| format!("Failed to read {}", active_tag_path.display()))?;
            let tag = content.trim();
            if tag.is_empty() {
                None
            } else {
                Some(tag.to_string())
            }
        } else {
            None
        };

        // Store in cache
        *self.active_group_cache.write().unwrap() = Some(active.clone());

        Ok(active)
    }

    pub fn set_active_group(&self, group_tag: &str) -> Result<()> {
        let tasks = self.load_tasks()?;
        if !tasks.contains_key(group_tag) {
            anyhow::bail!("Task group '{}' not found", group_tag);
        }

        // Write to active-tag file
        let active_tag_path = self.active_tag_file();
        fs::write(&active_tag_path, group_tag)
            .with_context(|| format!("Failed to write {}", active_tag_path.display()))?;

        // Update cache
        *self.active_group_cache.write().unwrap() = Some(Some(group_tag.to_string()));

        Ok(())
    }

    /// Clear the active group cache
    /// Useful when active-tag file is modified externally or for testing
    pub fn clear_cache(&self) {
        *self.active_group_cache.write().unwrap() = None;
    }

    /// Clear the active group setting (remove the active-tag file)
    pub fn clear_active_group(&self) -> Result<()> {
        let active_tag_path = self.active_tag_file();
        if active_tag_path.exists() {
            fs::remove_file(&active_tag_path)
                .with_context(|| format!("Failed to remove {}", active_tag_path.display()))?;
        }
        *self.active_group_cache.write().unwrap() = Some(None);
        Ok(())
    }

    /// Load a single task group by tag
    /// Parses the SCG file and extracts the requested group
    pub fn load_group(&self, group_tag: &str) -> Result<Phase> {
        let path = self.tasks_file();
        let content = self.read_with_lock(&path)?;

        let groups = self.parse_multi_phase_scg(&content)?;

        groups
            .get(group_tag)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Task group '{}' not found", group_tag))
    }

    /// Load the active task group directly (optimized)
    /// Combines get_active_group() and load_group() in one call
    pub fn load_active_group(&self) -> Result<Phase> {
        let active_tag = self
            .get_active_group()?
            .ok_or_else(|| anyhow::anyhow!("No active task group. Run: scud use-tag <tag>"))?;

        self.load_group(&active_tag)
    }

    /// Update a single task group atomically
    /// Holds exclusive lock across read-modify-write cycle to prevent races
    pub fn update_group(&self, group_tag: &str, group: &Phase) -> Result<()> {
        use std::io::{Read, Seek, SeekFrom, Write};

        let path = self.tasks_file();

        let dir = path.parent().unwrap();
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }

        // Open file for read+write with exclusive lock held throughout
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("Failed to open file: {}", path.display()))?;

        // Acquire exclusive lock with retry (held for entire operation)
        self.acquire_lock_with_retry(&file, 10)?;

        // Read current content while holding lock
        let mut content = String::new();
        file.read_to_string(&mut content)
            .with_context(|| format!("Failed to read from {}", path.display()))?;

        // Parse, modify, and serialize
        let mut groups = self.parse_multi_phase_scg(&content)?;
        groups.insert(group_tag.to_string(), group.clone());

        let mut sorted_tags: Vec<_> = groups.keys().collect();
        sorted_tags.sort();

        let mut output = String::new();
        for (i, tag) in sorted_tags.iter().enumerate() {
            if i > 0 {
                output.push_str("\n---\n\n");
            }
            let grp = groups.get(*tag).unwrap();
            output.push_str(&serialize_scg(grp));
        }

        // Truncate and write back while still holding lock
        file.seek(SeekFrom::Start(0))
            .with_context(|| "Failed to seek to beginning of file")?;
        file.set_len(0).with_context(|| "Failed to truncate file")?;
        file.write_all(output.as_bytes())
            .with_context(|| format!("Failed to write to {}", path.display()))?;
        file.flush()
            .with_context(|| format!("Failed to flush {}", path.display()))?;

        // Lock released when file is dropped
        Ok(())
    }

    /// Update a single task's status within a group
    /// Convenience method that loads, modifies, and saves the group atomically
    pub fn update_task_status(
        &self,
        group_tag: &str,
        task_id: &str,
        status: TaskStatus,
    ) -> Result<()> {
        let mut group = self.load_group(group_tag)?;

        let task = group
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| {
                anyhow::anyhow!("Task '{}' not found in group '{}'", task_id, group_tag)
            })?;

        task.status = status;
        self.update_group(group_tag, &group)
    }

    pub fn read_file(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_storage() -> (Storage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(Some(temp_dir.path().to_path_buf()));
        storage.initialize_dirs().unwrap();

        // Create empty tasks file
        let tasks_file = storage.tasks_file();
        fs::write(&tasks_file, "").unwrap();

        (storage, temp_dir)
    }

    #[test]
    fn test_save_and_load_tasks() {
        let (storage, _temp_dir) = create_test_storage();
        let mut tasks = HashMap::new();

        let phase = Phase::new("TEST-1".to_string());
        tasks.insert("TEST-1".to_string(), phase);

        storage.save_tasks(&tasks).unwrap();
        let loaded_tasks = storage.load_tasks().unwrap();

        assert_eq!(tasks.len(), loaded_tasks.len());
        assert!(loaded_tasks.contains_key("TEST-1"));
        assert_eq!(loaded_tasks.get("TEST-1").unwrap().name, "TEST-1");
    }

    #[test]
    fn test_load_single_group() {
        let (storage, _temp_dir) = create_test_storage();

        let mut tasks = HashMap::new();
        tasks.insert("PHASE-A".to_string(), Phase::new("PHASE-A".to_string()));
        tasks.insert("PHASE-B".to_string(), Phase::new("PHASE-B".to_string()));
        storage.save_tasks(&tasks).unwrap();

        let phase = storage.load_group("PHASE-A").unwrap();
        assert_eq!(phase.name, "PHASE-A");
    }

    #[test]
    fn test_load_group_not_found() {
        let (storage, _temp_dir) = create_test_storage();

        let tasks = HashMap::new();
        storage.save_tasks(&tasks).unwrap();

        let result = storage.load_group("NONEXISTENT");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_active_group_caching() {
        let (storage, _temp_dir) = create_test_storage();

        let mut tasks = HashMap::new();
        tasks.insert("TEST-1".to_string(), Phase::new("TEST-1".to_string()));
        storage.save_tasks(&tasks).unwrap();
        storage.set_active_group("TEST-1").unwrap();

        // First call
        let active1 = storage.get_active_group().unwrap();
        assert_eq!(active1, Some("TEST-1".to_string()));

        // Modify file directly
        let active_tag_path = storage.active_tag_file();
        fs::write(&active_tag_path, "DIFFERENT").unwrap();

        // Second call should return cached value
        let active2 = storage.get_active_group().unwrap();
        assert_eq!(active2, Some("TEST-1".to_string()));

        // After cache clear should reload
        storage.clear_cache();
        let active3 = storage.get_active_group().unwrap();
        assert_eq!(active3, Some("DIFFERENT".to_string()));
    }

    #[test]
    fn test_update_group() {
        let (storage, _temp_dir) = create_test_storage();

        let mut tasks = HashMap::new();
        tasks.insert("PHASE-1".to_string(), Phase::new("PHASE-1".to_string()));
        storage.save_tasks(&tasks).unwrap();

        // Update
        let mut phase = storage.load_group("PHASE-1").unwrap();
        phase.add_task(crate::models::Task::new(
            "task-1".to_string(),
            "Test".to_string(),
            "Desc".to_string(),
        ));
        storage.update_group("PHASE-1", &phase).unwrap();

        // Verify
        let loaded = storage.load_group("PHASE-1").unwrap();
        assert_eq!(loaded.tasks.len(), 1);
    }
}
