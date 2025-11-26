use anyhow::{Context, Result};
use fs2::FileExt;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::thread;
use std::time::Duration;

use crate::config::Config;
use crate::formats::{parse_scg, serialize_scg};
use crate::models::{Phase, WorkflowState};

pub struct Storage {
    project_root: PathBuf,
    /// Cache for active group to avoid repeated workflow state loads
    /// Option<Option<String>> represents: None = not cached, Some(None) = no active group, Some(Some(tag)) = cached tag
    /// Uses RwLock for thread safety (useful for tests and potential daemon mode)
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

    pub fn tasks_file(&self) -> PathBuf {
        self.scud_dir().join("tasks").join("tasks.scg")
    }

    fn tasks_json_file(&self) -> PathBuf {
        self.scud_dir().join("tasks").join("tasks.json")
    }

    pub fn workflow_file(&self) -> PathBuf {
        self.scud_dir().join("workflow-state.json")
    }

    pub fn config_file(&self) -> PathBuf {
        self.scud_dir().join("config.toml")
    }

    pub fn docs_dir(&self) -> PathBuf {
        self.project_root.join("docs")
    }

    pub fn is_initialized(&self) -> bool {
        self.scud_dir().exists() && self.tasks_file().exists() && self.workflow_file().exists()
    }

    pub fn initialize(&self) -> Result<()> {
        let config = Config::default();
        self.initialize_with_config(&config)
    }

    pub fn initialize_with_config(&self, config: &Config) -> Result<()> {
        // Create .scud directory structure
        let scud_dir = self.scud_dir();
        fs::create_dir_all(scud_dir.join("tasks"))
            .context("Failed to create .scud/tasks directory")?;

        // Initialize config.toml
        let config_file = self.config_file();
        if !config_file.exists() {
            config.save(&config_file)?;
        }

        // Initialize tasks.scg with empty content (and JSON mirror for legacy tooling)
        let tasks_file = self.tasks_file();
        if !tasks_file.exists() {
            let empty_tasks: HashMap<String, Phase> = HashMap::new();
            self.save_tasks(&empty_tasks)?;
        } else {
            let json_path = self.tasks_json_file();
            if !json_path.exists() {
                let empty: HashMap<String, Phase> = HashMap::new();
                // Best effort: if parsing existing SCG fails, fall back to empty JSON
                match self.load_tasks() {
                    Ok(tasks) => self.write_tasks_json(&tasks)?,
                    Err(_) => self.write_tasks_json(&empty)?,
                }
            }
        }

        // Initialize workflow-state.json
        let workflow_file = self.workflow_file();
        if !workflow_file.exists() {
            let workflow_state = WorkflowState::new();
            self.save_workflow_state(&workflow_state)?;
        }

        // Create docs directories
        let docs = self.docs_dir();
        fs::create_dir_all(docs.join("prd"))?;
        fs::create_dir_all(docs.join("phases"))?;
        fs::create_dir_all(docs.join("architecture"))?;
        fs::create_dir_all(docs.join("retrospectives"))?;

        // Update .gitignore
        self.update_gitignore()?;

        Ok(())
    }

    pub fn load_config(&self) -> Result<Config> {
        let config_file = self.config_file();
        if !config_file.exists() {
            return Ok(Config::default());
        }
        Config::load(&config_file)
    }

    fn update_gitignore(&self) -> Result<()> {
        let gitignore_path = self.project_root.join(".gitignore");
        let entry = "\n# SCUD\n.scud/\n";

        if gitignore_path.exists() {
            let content = fs::read_to_string(&gitignore_path)?;
            if !content.contains(".scud/") {
                fs::write(&gitignore_path, format!("{}{}", content, entry))?;
            }
        } else {
            fs::write(&gitignore_path, entry)?;
        }

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
        })?;

        // Keep JSON mirror for Node-based tooling (validator, MCP resources)
        self.write_tasks_json(tasks)?;

        Ok(())
    }

    pub fn load_workflow_state(&self) -> Result<WorkflowState> {
        let path = self.workflow_file();
        if !path.exists() {
            anyhow::bail!(
                "Workflow state not found: {}\nRun: scud init",
                path.display()
            );
        }

        let content = self.read_with_lock(&path)?;
        let state: WorkflowState = serde_json::from_str(&content)
            .with_context(|| "Failed to parse workflow-state.json".to_string())?;

        Ok(state)
    }

    pub fn save_workflow_state(&self, state: &WorkflowState) -> Result<()> {
        let path = self.workflow_file();
        self.write_with_lock(&path, || {
            serde_json::to_string_pretty(state)
                .with_context(|| "Failed to serialize workflow state to JSON".to_string())
        })
    }

    fn write_tasks_json(&self, tasks: &HashMap<String, Phase>) -> Result<()> {
        let json_path = self.tasks_json_file();
        self.write_with_lock(&json_path, || {
            serde_json::to_string_pretty(tasks)
                .with_context(|| "Failed to serialize tasks to JSON".to_string())
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

        // Load from file and cache (write lock)
        let state = self.load_workflow_state()?;
        let active = state.active_group.clone();

        // Store in cache
        *self.active_group_cache.write().unwrap() = Some(active.clone());

        Ok(active)
    }

    pub fn set_active_group(&self, group_tag: &str) -> Result<()> {
        let tasks = self.load_tasks()?;
        if !tasks.contains_key(group_tag) {
            anyhow::bail!("Task group '{}' not found", group_tag);
        }

        let mut state = self.load_workflow_state()?;
        state.active_group = Some(group_tag.to_string());
        state.update();
        self.save_workflow_state(&state)?;

        // Update cache
        *self.active_group_cache.write().unwrap() = Some(Some(group_tag.to_string()));

        Ok(())
    }

    /// Clear the active group cache
    /// Useful when workflow state is modified externally or for testing
    pub fn clear_cache(&self) {
        *self.active_group_cache.write().unwrap() = None;
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
        // Note: truncate(false) is explicit - we read first, then truncate manually after
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

    pub fn read_file(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_storage() -> (Storage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(Some(temp_dir.path().to_path_buf()));
        storage.initialize().unwrap();
        (storage, temp_dir)
    }

    #[test]
    fn test_write_with_lock_creates_file() {
        let (storage, _temp_dir) = create_test_storage();
        let test_file = storage.scud_dir().join("test.json");

        storage
            .write_with_lock(&test_file, || Ok(r#"{"test": "data"}"#.to_string()))
            .unwrap();

        assert!(test_file.exists());
        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, r#"{"test": "data"}"#);
    }

    #[test]
    fn test_read_with_lock_reads_existing_file() {
        let (storage, _temp_dir) = create_test_storage();
        let test_file = storage.scud_dir().join("test.json");

        // Create a file
        fs::write(&test_file, r#"{"test": "data"}"#).unwrap();

        // Read with lock
        let content = storage.read_with_lock(&test_file).unwrap();
        assert_eq!(content, r#"{"test": "data"}"#);
    }

    #[test]
    fn test_read_with_lock_fails_on_missing_file() {
        let (storage, _temp_dir) = create_test_storage();
        let test_file = storage.scud_dir().join("nonexistent.json");

        let result = storage.read_with_lock(&test_file);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("File not found"));
    }

    #[test]
    fn test_save_and_load_tasks_with_locking() {
        let (storage, _temp_dir) = create_test_storage();
        let mut tasks = HashMap::new();

        let epic = crate::models::Phase::new("TEST-1".to_string());
        tasks.insert("TEST-1".to_string(), epic);

        // Save tasks
        storage.save_tasks(&tasks).unwrap();

        // Load tasks
        let loaded_tasks = storage.load_tasks().unwrap();

        assert_eq!(tasks.len(), loaded_tasks.len());
        assert!(loaded_tasks.contains_key("TEST-1"));
        assert_eq!(loaded_tasks.get("TEST-1").unwrap().name, "TEST-1");
    }

    #[test]
    fn test_save_and_load_workflow_state_with_locking() {
        let (storage, _temp_dir) = create_test_storage();

        let mut state = crate::models::WorkflowState::new();
        state.active_group = Some("TEST-1".to_string());

        // Save state
        storage.save_workflow_state(&state).unwrap();

        // Load state
        let loaded_state = storage.load_workflow_state().unwrap();

        assert_eq!(loaded_state.active_group, Some("TEST-1".to_string()));
    }

    #[test]
    fn test_concurrent_writes_dont_corrupt_data() {
        use std::sync::Arc;
        use std::thread;

        let (storage, _temp_dir) = create_test_storage();
        let storage = Arc::new(storage);
        let mut handles = vec![];

        // Spawn 10 threads that each write tasks
        for i in 0..10 {
            let storage_clone = Arc::clone(&storage);
            let handle = thread::spawn(move || {
                let mut tasks = HashMap::new();
                let epic = crate::models::Phase::new(format!("EPIC-{}", i));
                tasks.insert(format!("EPIC-{}", i), epic);

                // Each thread writes multiple times
                for _ in 0..5 {
                    storage_clone.save_tasks(&tasks).unwrap();
                    thread::sleep(Duration::from_millis(1));
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify that the file is still valid JSON
        let tasks = storage.load_tasks().unwrap();
        // Should have the last written data (from one of the threads)
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn test_lock_retry_on_contention() {
        use std::sync::Arc;

        let (storage, _temp_dir) = create_test_storage();
        let storage = Arc::new(storage);
        let test_file = storage.scud_dir().join("lock-test.json");

        // Create file
        storage
            .write_with_lock(&test_file, || Ok(r#"{"initial": "data"}"#.to_string()))
            .unwrap();

        // Open and lock the file
        let file = OpenOptions::new().write(true).open(&test_file).unwrap();
        file.lock_exclusive().unwrap();

        // Try to acquire lock with retry in another thread
        let storage_clone = Arc::clone(&storage);
        let test_file_clone = test_file.clone();
        let handle = thread::spawn(move || {
            // This should retry and succeed after lock release
            storage_clone.write_with_lock(&test_file_clone, || {
                Ok(r#"{"updated": "data"}"#.to_string())
            })
        });

        // Keep lock for a bit
        thread::sleep(Duration::from_millis(200));

        // Release lock
        file.unlock().unwrap();
        drop(file);

        // The write should have succeeded after retrying
        let result = handle.join().unwrap();
        assert!(result.is_ok());
    }

    // ==================== Error Handling Tests ====================

    #[test]
    fn test_load_tasks_with_malformed_json() {
        let (storage, _temp_dir) = create_test_storage();
        let tasks_file = storage.tasks_file();

        // Write malformed JSON
        fs::write(&tasks_file, r#"{"invalid": json here}"#).unwrap();

        // Should return error
        let result = storage.load_tasks();
        assert!(result.is_err());
    }

    #[test]
    fn test_load_workflow_state_with_malformed_json() {
        let (storage, _temp_dir) = create_test_storage();
        let workflow_file = storage.workflow_file();

        // Write malformed JSON
        fs::write(&workflow_file, r#"not valid json at all"#).unwrap();

        // Should return error
        let result = storage.load_workflow_state();
        assert!(result.is_err());
    }

    #[test]
    fn test_load_tasks_with_empty_file() {
        let (storage, _temp_dir) = create_test_storage();
        let tasks_file = storage.tasks_file();

        // Write empty file
        fs::write(&tasks_file, "").unwrap();

        // Empty SCG file is valid and returns empty HashMap
        let result = storage.load_tasks();
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_load_tasks_missing_file_creates_default() {
        let (storage, _temp_dir) = create_test_storage();
        // Don't create tasks file

        // Should return empty HashMap (default)
        let tasks = storage.load_tasks().unwrap();
        assert_eq!(tasks.len(), 0);
    }

    #[test]
    fn test_load_workflow_state_missing_file_creates_default() {
        let (storage, _temp_dir) = create_test_storage();
        // Don't create workflow state file

        // Should return default WorkflowState
        let state = storage.load_workflow_state().unwrap();
        assert_eq!(state.current_phase, "ideation");
        assert_eq!(state.active_group, None);
    }

    #[test]
    fn test_save_tasks_creates_directory_if_missing() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(Some(temp_dir.path().to_path_buf()));
        // Don't call initialize()

        let mut tasks = HashMap::new();
        let epic = crate::models::Phase::new("TEST-1".to_string());
        tasks.insert("TEST-1".to_string(), epic);

        // Should create directory and file
        let result = storage.save_tasks(&tasks);
        assert!(result.is_ok());

        assert!(storage.scud_dir().exists());
        assert!(storage.tasks_file().exists());
    }

    #[test]
    fn test_write_with_lock_handles_directory_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(Some(temp_dir.path().to_path_buf()));

        let nested_file = temp_dir
            .path()
            .join("deeply")
            .join("nested")
            .join("test.json");

        // Should create all parent directories
        let result = storage.write_with_lock(&nested_file, || Ok("{}".to_string()));
        assert!(result.is_ok());
        assert!(nested_file.exists());
    }

    #[test]
    fn test_load_tasks_with_invalid_structure() {
        let (storage, _temp_dir) = create_test_storage();
        let tasks_file = storage.tasks_file();

        // Write valid JSON but invalid structure (array instead of object)
        fs::write(&tasks_file, r#"["not", "an", "object"]"#).unwrap();

        // Should return error
        let result = storage.load_tasks();
        assert!(result.is_err());
    }

    #[test]
    fn test_load_workflow_state_with_missing_fields() {
        let (storage, _temp_dir) = create_test_storage();
        let workflow_file = storage.workflow_file();

        // Write JSON with missing required fields
        fs::write(&workflow_file, r#"{"version": "1.0.0"}"#).unwrap();

        // Should return error (missing current_phase, etc.)
        let result = storage.load_workflow_state();
        assert!(result.is_err());
    }

    #[test]
    fn test_save_and_load_with_unicode_content() {
        let (storage, _temp_dir) = create_test_storage();

        let mut tasks = HashMap::new();
        let mut epic = crate::models::Phase::new("TEST-UNICODE".to_string());

        // Add task with unicode content
        let task = crate::models::Task::new(
            "task-1".to_string(),
            "测试 Unicode 🚀".to_string(),
            "Descripción en español 日本語".to_string(),
        );
        epic.add_task(task);

        tasks.insert("TEST-UNICODE".to_string(), epic);

        // Save and load
        storage.save_tasks(&tasks).unwrap();
        let loaded_tasks = storage.load_tasks().unwrap();

        let loaded_epic = loaded_tasks.get("TEST-UNICODE").unwrap();
        let loaded_task = loaded_epic.get_task("task-1").unwrap();
        assert_eq!(loaded_task.title, "测试 Unicode 🚀");
        assert_eq!(loaded_task.description, "Descripción en español 日本語");
    }

    #[test]
    fn test_save_and_load_with_large_dataset() {
        let (storage, _temp_dir) = create_test_storage();

        let mut tasks = HashMap::new();

        // Create 100 epics with 50 tasks each
        for i in 0..100 {
            let mut epic = crate::models::Phase::new(format!("EPIC-{}", i));

            for j in 0..50 {
                let task = crate::models::Task::new(
                    format!("task-{}-{}", i, j),
                    format!("Task {} of Epic {}", j, i),
                    format!("Description for task {}-{}", i, j),
                );
                epic.add_task(task);
            }

            tasks.insert(format!("EPIC-{}", i), epic);
        }

        // Save and load
        storage.save_tasks(&tasks).unwrap();
        let loaded_tasks = storage.load_tasks().unwrap();

        assert_eq!(loaded_tasks.len(), 100);
        for i in 0..100 {
            let epic = loaded_tasks.get(&format!("EPIC-{}", i)).unwrap();
            assert_eq!(epic.tasks.len(), 50);
        }
    }

    #[test]
    fn test_concurrent_read_and_write() {
        use std::sync::Arc;
        use std::thread;

        let (storage, _temp_dir) = create_test_storage();
        let storage = Arc::new(storage);

        // Initialize with some data
        let mut tasks = HashMap::new();
        let epic = crate::models::Phase::new("INITIAL".to_string());
        tasks.insert("INITIAL".to_string(), epic);
        storage.save_tasks(&tasks).unwrap();

        let mut handles = vec![];

        // Spawn 5 readers
        for _ in 0..5 {
            let storage_clone = Arc::clone(&storage);
            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    let _ = storage_clone.load_tasks();
                    thread::sleep(Duration::from_millis(1));
                }
            });
            handles.push(handle);
        }

        // Spawn 2 writers
        for i in 0..2 {
            let storage_clone = Arc::clone(&storage);
            let handle = thread::spawn(move || {
                for j in 0..5 {
                    let mut tasks = HashMap::new();
                    let epic = crate::models::Phase::new(format!("WRITER-{}-{}", i, j));
                    tasks.insert(format!("WRITER-{}-{}", i, j), epic);
                    storage_clone.save_tasks(&tasks).unwrap();
                    thread::sleep(Duration::from_millis(2));
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // File should still be valid
        let tasks = storage.load_tasks().unwrap();
        assert_eq!(tasks.len(), 1); // Last write wins
    }

    // ==================== Active Epic Cache Tests ====================

    #[test]
    fn test_active_epic_cached_on_second_call() {
        let (storage, _temp_dir) = create_test_storage();

        // Set active epic
        let mut tasks = HashMap::new();
        tasks.insert("TEST-1".to_string(), Epic::new("TEST-1".to_string()));
        storage.save_tasks(&tasks).unwrap();
        storage.set_active_group("TEST-1").unwrap();

        // First call - loads from file
        let active1 = storage.get_active_group().unwrap();
        assert_eq!(active1, Some("TEST-1".to_string()));

        // Modify file directly (bypass storage methods)
        let workflow_file = storage.workflow_file();
        let mut state = storage.load_workflow_state().unwrap();
        state.active_group = Some("DIFFERENT".to_string());
        fs::write(&workflow_file, serde_json::to_string(&state).unwrap()).unwrap();

        // Second call - should return cached value (not file value)
        let active2 = storage.get_active_group().unwrap();
        assert_eq!(active2, Some("TEST-1".to_string())); // Still cached

        // After cache clear - should reload from file
        storage.clear_cache();
        let active3 = storage.get_active_group().unwrap();
        assert_eq!(active3, Some("DIFFERENT".to_string())); // From file
    }

    #[test]
    fn test_cache_invalidated_on_set_active_epic() {
        let (storage, _temp_dir) = create_test_storage();

        let mut tasks = HashMap::new();
        tasks.insert("EPIC-1".to_string(), Epic::new("EPIC-1".to_string()));
        tasks.insert("EPIC-2".to_string(), Epic::new("EPIC-2".to_string()));
        storage.save_tasks(&tasks).unwrap();

        storage.set_active_group("EPIC-1").unwrap();
        assert_eq!(
            storage.get_active_group().unwrap(),
            Some("EPIC-1".to_string())
        );

        // Change active epic - should update cache
        storage.set_active_group("EPIC-2").unwrap();
        assert_eq!(
            storage.get_active_group().unwrap(),
            Some("EPIC-2".to_string())
        );
    }

    #[test]
    fn test_cache_with_no_active_epic() {
        let (storage, _temp_dir) = create_test_storage();

        // Load when no active epic is set
        let active = storage.get_active_group().unwrap();
        assert_eq!(active, None);

        // Should cache the None value
        let active2 = storage.get_active_group().unwrap();
        assert_eq!(active2, None);
    }

    // ==================== Lazy Epic Loading Tests ====================

    #[test]
    fn test_load_single_epic_from_many() {
        let (storage, _temp_dir) = create_test_storage();

        // Create 50 epics
        let mut tasks = HashMap::new();
        for i in 0..50 {
            tasks.insert(format!("EPIC-{}", i), Epic::new(format!("EPIC-{}", i)));
        }
        storage.save_tasks(&tasks).unwrap();

        // Load single epic - should only deserialize that one
        let epic = storage.load_group("EPIC-25").unwrap();
        assert_eq!(epic.name, "EPIC-25");
    }

    #[test]
    fn test_load_epic_not_found() {
        let (storage, _temp_dir) = create_test_storage();

        let tasks = HashMap::new();
        storage.save_tasks(&tasks).unwrap();

        let result = storage.load_group("NONEXISTENT");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_load_epic_matches_full_load() {
        let (storage, _temp_dir) = create_test_storage();

        let mut tasks = HashMap::new();
        let mut epic = Epic::new("TEST-1".to_string());
        epic.add_task(crate::models::Task::new(
            "task-1".to_string(),
            "Test".to_string(),
            "Desc".to_string(),
        ));
        tasks.insert("TEST-1".to_string(), epic.clone());
        storage.save_tasks(&tasks).unwrap();

        // Load via both methods
        let epic_lazy = storage.load_group("TEST-1").unwrap();
        let tasks_full = storage.load_tasks().unwrap();
        let epic_full = tasks_full.get("TEST-1").unwrap();

        // Should be identical
        assert_eq!(epic_lazy.name, epic_full.name);
        assert_eq!(epic_lazy.tasks.len(), epic_full.tasks.len());
    }

    #[test]
    fn test_load_active_epic() {
        let (storage, _temp_dir) = create_test_storage();

        let mut tasks = HashMap::new();
        let mut epic = Epic::new("ACTIVE-1".to_string());
        epic.add_task(crate::models::Task::new(
            "task-1".to_string(),
            "Test".to_string(),
            "Desc".to_string(),
        ));
        tasks.insert("ACTIVE-1".to_string(), epic);
        storage.save_tasks(&tasks).unwrap();
        storage.set_active_group("ACTIVE-1").unwrap();

        // Load active epic directly
        let epic = storage.load_active_group().unwrap();
        assert_eq!(epic.name, "ACTIVE-1");
        assert_eq!(epic.tasks.len(), 1);
    }

    #[test]
    fn test_load_active_epic_when_none_set() {
        let (storage, _temp_dir) = create_test_storage();

        // Should error when no active epic
        let result = storage.load_active_group();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No active task group"));
    }

    #[test]
    fn test_update_epic_without_loading_all() {
        let (storage, _temp_dir) = create_test_storage();

        let mut tasks = HashMap::new();
        tasks.insert("EPIC-1".to_string(), Epic::new("EPIC-1".to_string()));
        tasks.insert("EPIC-2".to_string(), Epic::new("EPIC-2".to_string()));
        storage.save_tasks(&tasks).unwrap();

        // Update only EPIC-1
        let mut epic1 = storage.load_group("EPIC-1").unwrap();
        epic1.add_task(crate::models::Task::new(
            "new-task".to_string(),
            "New".to_string(),
            "Desc".to_string(),
        ));
        storage.update_group("EPIC-1", &epic1).unwrap();

        // Verify update
        let loaded = storage.load_group("EPIC-1").unwrap();
        assert_eq!(loaded.tasks.len(), 1);

        // Verify EPIC-2 unchanged
        let epic2 = storage.load_group("EPIC-2").unwrap();
        assert_eq!(epic2.tasks.len(), 0);
    }
}
