use anyhow::{Context, Result};
use fs2::FileExt;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::models::{Epic, WorkflowState};

pub struct Storage {
    project_root: PathBuf,
}

impl Storage {
    pub fn new(project_root: Option<PathBuf>) -> Self {
        let root = project_root.unwrap_or_else(|| std::env::current_dir().unwrap());
        Storage { project_root: root }
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
        let dir = path.parent().unwrap();
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }

        // Open file for writing
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .with_context(|| format!("Failed to open file for writing: {}", path.display()))?;

        // Acquire lock with retry
        self.acquire_lock_with_retry(&file, 10)?;

        // Generate content and write
        let content = writer()?;
        fs::write(path, content)
            .with_context(|| format!("Failed to write to {}", path.display()))?;

        // Lock is automatically released when file is dropped
        Ok(())
    }

    /// Perform a locked read operation on a file
    fn read_with_lock(&self, path: &Path) -> Result<String> {
        if !path.exists() {
            anyhow::bail!("File not found: {}", path.display());
        }

        // Open file for reading
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .with_context(|| format!("Failed to open file for reading: {}", path.display()))?;

        // Acquire shared lock (allows multiple readers)
        file.lock_shared()
            .with_context(|| format!("Failed to acquire read lock on {}", path.display()))?;

        // Read content
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read from {}", path.display()))?;

        // Lock is automatically released when file is dropped
        Ok(content)
    }

    pub fn taskmaster_dir(&self) -> PathBuf {
        self.project_root.join(".taskmaster")
    }

    pub fn tasks_file(&self) -> PathBuf {
        self.taskmaster_dir().join("tasks").join("tasks.json")
    }

    pub fn workflow_file(&self) -> PathBuf {
        self.taskmaster_dir().join("workflow-state.json")
    }

    pub fn docs_dir(&self) -> PathBuf {
        self.project_root.join("docs")
    }

    pub fn is_initialized(&self) -> bool {
        self.taskmaster_dir().exists()
            && self.tasks_file().exists()
            && self.workflow_file().exists()
    }

    pub fn initialize(&self) -> Result<()> {
        // Create .taskmaster directory structure
        let taskmaster = self.taskmaster_dir();
        fs::create_dir_all(taskmaster.join("tasks"))
            .context("Failed to create .taskmaster/tasks directory")?;

        // Initialize tasks.json with empty object
        let tasks_file = self.tasks_file();
        if !tasks_file.exists() {
            let empty_tasks: HashMap<String, Epic> = HashMap::new();
            self.save_tasks(&empty_tasks)?;
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
        fs::create_dir_all(docs.join("epics"))?;
        fs::create_dir_all(docs.join("architecture"))?;
        fs::create_dir_all(docs.join("retrospectives"))?;

        // Update .gitignore
        self.update_gitignore()?;

        Ok(())
    }

    fn update_gitignore(&self) -> Result<()> {
        let gitignore_path = self.project_root.join(".gitignore");
        let entry = "\n# SCUD Task Master\n.taskmaster/\n";

        if gitignore_path.exists() {
            let content = fs::read_to_string(&gitignore_path)?;
            if !content.contains(".taskmaster/") {
                fs::write(&gitignore_path, format!("{}{}", content, entry))?;
            }
        } else {
            fs::write(&gitignore_path, entry)?;
        }

        Ok(())
    }

    pub fn load_tasks(&self) -> Result<HashMap<String, Epic>> {
        let path = self.tasks_file();
        if !path.exists() {
            anyhow::bail!("Tasks file not found: {}\nRun: scud init", path.display());
        }

        let content = self.read_with_lock(&path)?;
        let tasks: HashMap<String, Epic> = serde_json::from_str(&content)
            .with_context(|| "Failed to parse tasks.json".to_string())?;

        Ok(tasks)
    }

    pub fn save_tasks(&self, tasks: &HashMap<String, Epic>) -> Result<()> {
        let path = self.tasks_file();
        self.write_with_lock(&path, || {
            serde_json::to_string_pretty(tasks)
                .with_context(|| "Failed to serialize tasks to JSON".to_string())
        })
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

    pub fn get_active_epic(&self) -> Result<Option<String>> {
        let state = self.load_workflow_state()?;
        Ok(state.active_epic)
    }

    pub fn set_active_epic(&self, epic_tag: &str) -> Result<()> {
        let tasks = self.load_tasks()?;
        if !tasks.contains_key(epic_tag) {
            anyhow::bail!("Epic '{}' not found", epic_tag);
        }

        let mut state = self.load_workflow_state()?;
        state.active_epic = Some(epic_tag.to_string());
        state.update();
        self.save_workflow_state(&state)?;

        Ok(())
    }

    pub fn read_file(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path.display()))
    }

    // Epic Groups management
    pub fn groups_file(&self) -> PathBuf {
        self.taskmaster_dir().join("epic-groups.json")
    }

    pub fn load_groups(&self) -> Result<crate::models::EpicGroups> {
        let path = self.groups_file();
        if !path.exists() {
            return Ok(crate::models::EpicGroups::new());
        }

        let content = self.read_with_lock(&path)?;
        let groups: crate::models::EpicGroups = serde_json::from_str(&content)
            .with_context(|| "Failed to parse epic-groups.json".to_string())?;

        Ok(groups)
    }

    pub fn save_groups(&self, groups: &crate::models::EpicGroups) -> Result<()> {
        let path = self.groups_file();
        self.write_with_lock(&path, || {
            serde_json::to_string_pretty(groups)
                .with_context(|| "Failed to serialize groups to JSON".to_string())
        })
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
        let test_file = storage.taskmaster_dir().join("test.json");

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
        let test_file = storage.taskmaster_dir().join("test.json");

        // Create a file
        fs::write(&test_file, r#"{"test": "data"}"#).unwrap();

        // Read with lock
        let content = storage.read_with_lock(&test_file).unwrap();
        assert_eq!(content, r#"{"test": "data"}"#);
    }

    #[test]
    fn test_read_with_lock_fails_on_missing_file() {
        let (storage, _temp_dir) = create_test_storage();
        let test_file = storage.taskmaster_dir().join("nonexistent.json");

        let result = storage.read_with_lock(&test_file);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("File not found"));
    }

    #[test]
    fn test_save_and_load_tasks_with_locking() {
        let (storage, _temp_dir) = create_test_storage();
        let mut tasks = HashMap::new();

        let epic = crate::models::Epic::new("TEST-1".to_string());
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
        state.active_epic = Some("TEST-1".to_string());

        // Save state
        storage.save_workflow_state(&state).unwrap();

        // Load state
        let loaded_state = storage.load_workflow_state().unwrap();

        assert_eq!(loaded_state.active_epic, Some("TEST-1".to_string()));
    }

    #[test]
    fn test_save_and_load_groups_with_locking() {
        let (storage, _temp_dir) = create_test_storage();

        let mut groups = crate::models::EpicGroups::new();
        let group = crate::models::EpicGroup::new(
            "group-1".to_string(),
            "Test Group".to_string(),
            vec!["epic-1".to_string()],
        );
        groups.add_group(group);

        // Save groups
        storage.save_groups(&groups).unwrap();

        // Load groups
        let loaded_groups = storage.load_groups().unwrap();

        assert!(loaded_groups.get_group("group-1").is_some());
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
                let epic = crate::models::Epic::new(format!("EPIC-{}", i));
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
        let (storage, _temp_dir) = create_test_storage();
        let test_file = storage.taskmaster_dir().join("lock-test.json");

        // Create file
        storage
            .write_with_lock(&test_file, || Ok(r#"{"initial": "data"}"#.to_string()))
            .unwrap();

        // Open and lock the file
        let file = OpenOptions::new()
            .write(true)
            .open(&test_file)
            .unwrap();
        file.lock_exclusive().unwrap();

        // Try to acquire lock with retry in another thread
        let storage_clone = storage.clone();
        let test_file_clone = test_file.clone();
        let handle = thread::spawn(move || {
            // This should retry and eventually fail after max retries
            let result = storage_clone.write_with_lock(&test_file_clone, || {
                Ok(r#"{"updated": "data"}"#.to_string())
            });
            result
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
}

impl Clone for Storage {
    fn clone(&self) -> Self {
        Storage {
            project_root: self.project_root.clone(),
        }
    }
}
