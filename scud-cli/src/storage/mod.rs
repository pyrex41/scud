use anyhow::{Context, Result};
use chrono::Local;
use fs2::FileExt;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::thread;
use std::time::Duration;

use crate::config::Config;
use crate::formats::{parse_scg, serialize_scg};
use crate::models::Phase;

/// Information about an archived phase
#[derive(Debug, Clone)]
pub struct ArchiveInfo {
    /// The filename of the archive (e.g., "2026-01-13_v1.scg")
    pub filename: String,
    /// Full path to the archive file
    pub path: PathBuf,
    /// The date extracted from the filename (e.g., "2026-01-13")
    pub date: String,
    /// The tag name if this is a single-phase archive, None if "all"
    pub tag: Option<String>,
    /// Number of tasks in the archive
    pub task_count: usize,
}

pub struct Storage {
    project_root: PathBuf,
    /// Cache for active group to avoid repeated file reads
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

    pub fn tasks_file(&self) -> PathBuf {
        self.scud_dir().join("tasks").join("tasks.scg")
    }

    fn active_tag_file(&self) -> PathBuf {
        self.scud_dir().join("active-tag")
    }

    pub fn config_file(&self) -> PathBuf {
        self.scud_dir().join("config.toml")
    }

    pub fn docs_dir(&self) -> PathBuf {
        self.scud_dir().join("docs")
    }

    pub fn guidance_dir(&self) -> PathBuf {
        self.scud_dir().join("guidance")
    }

    /// Load all .md files from .scud/guidance/ folder
    /// Returns concatenated content with file headers, or empty string if no files
    pub fn load_guidance(&self) -> Result<String> {
        let guidance_dir = self.guidance_dir();

        if !guidance_dir.exists() {
            return Ok(String::new());
        }

        let mut guidance_content = String::new();
        let mut entries: Vec<_> = fs::read_dir(&guidance_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
            .collect();

        // Sort by filename for consistent ordering
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            match fs::read_to_string(&path) {
                Ok(content) => {
                    if !guidance_content.is_empty() {
                        guidance_content.push_str("\n\n");
                    }
                    guidance_content.push_str(&format!("### {}\n\n{}", filename, content));
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to read guidance file {}: {}",
                        path.display(),
                        e
                    );
                }
            }
        }

        Ok(guidance_content)
    }

    pub fn is_initialized(&self) -> bool {
        self.scud_dir().exists() && self.tasks_file().exists()
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

        // Initialize tasks.scg with empty content
        let tasks_file = self.tasks_file();
        if !tasks_file.exists() {
            let empty_tasks: HashMap<String, Phase> = HashMap::new();
            self.save_tasks(&empty_tasks)?;
        }

        // Create docs directories
        let docs = self.docs_dir();
        fs::create_dir_all(docs.join("prd"))?;
        fs::create_dir_all(docs.join("phases"))?;
        fs::create_dir_all(docs.join("architecture"))?;
        fs::create_dir_all(docs.join("retrospectives"))?;

        // Create guidance directory for project-specific AI context
        fs::create_dir_all(self.guidance_dir())?;

        // Create CLAUDE.md with agent instructions
        self.create_agent_instructions()?;

        Ok(())
    }

    pub fn load_config(&self) -> Result<Config> {
        let config_file = self.config_file();
        if !config_file.exists() {
            return Ok(Config::default());
        }
        Config::load(&config_file)
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

    // ==================== Archive Methods ====================

    /// Get the archive directory path
    pub fn archive_dir(&self) -> PathBuf {
        self.scud_dir().join("archive")
    }

    /// Ensure archive directory exists
    pub fn ensure_archive_dir(&self) -> Result<()> {
        let dir = self.archive_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir).context("Failed to create archive directory")?;
        }
        Ok(())
    }

    /// Generate archive filename for a tag or all tasks
    /// Format: {YYYY-MM-DD}_{tag}.scg or {YYYY-MM-DD}_all.scg
    pub fn archive_filename(&self, tag: Option<&str>) -> String {
        let date = Local::now().format("%Y-%m-%d");
        match tag {
            Some(t) => format!("{}_{}.scg", date, t),
            None => format!("{}_all.scg", date),
        }
    }

    /// Get unique archive path by appending counter if file exists
    /// Tries base path, then base_1, base_2, etc. up to 100
    /// Falls back to timestamp suffix if all counters exhausted
    fn unique_archive_path(&self, base_path: &Path) -> PathBuf {
        if !base_path.exists() {
            return base_path.to_path_buf();
        }

        let stem = base_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("archive");
        let ext = base_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("scg");
        let parent = base_path.parent().unwrap_or(Path::new("."));

        for i in 1..100 {
            let new_name = format!("{}_{}.{}", stem, i, ext);
            let new_path = parent.join(&new_name);
            if !new_path.exists() {
                return new_path;
            }
        }

        // Fallback with timestamp
        let ts = Local::now().format("%H%M%S");
        parent.join(format!("{}_{}.{}", stem, ts, ext))
    }

    /// Archive a single phase/tag
    /// Returns the path to the created archive file
    pub fn archive_phase(&self, tag: &str, phases: &HashMap<String, Phase>) -> Result<PathBuf> {
        self.ensure_archive_dir()?;

        let phase = phases
            .get(tag)
            .ok_or_else(|| anyhow::anyhow!("Tag '{}' not found", tag))?;

        let filename = self.archive_filename(Some(tag));
        let archive_path = self.archive_dir().join(&filename);
        let final_path = self.unique_archive_path(&archive_path);

        // Serialize single phase to SCG format
        let content = serialize_scg(phase);
        fs::write(&final_path, content)
            .with_context(|| format!("Failed to write archive: {}", final_path.display()))?;

        Ok(final_path)
    }

    /// Archive all phases together
    /// Returns the path to the created archive file
    pub fn archive_all(&self, phases: &HashMap<String, Phase>) -> Result<PathBuf> {
        self.ensure_archive_dir()?;

        let filename = self.archive_filename(None);
        let archive_path = self.archive_dir().join(&filename);
        let final_path = self.unique_archive_path(&archive_path);

        // Serialize all phases (same format as tasks.scg)
        let mut sorted_tags: Vec<_> = phases.keys().collect();
        sorted_tags.sort();

        let mut output = String::new();
        for (i, tag) in sorted_tags.iter().enumerate() {
            if i > 0 {
                output.push_str("\n---\n\n");
            }
            let phase = phases.get(*tag).unwrap();
            output.push_str(&serialize_scg(phase));
        }

        fs::write(&final_path, output)
            .with_context(|| format!("Failed to write archive: {}", final_path.display()))?;

        Ok(final_path)
    }

    /// Parse archive filename to extract date and tag
    /// Returns (date, tag) where tag is None if "all"
    pub fn parse_archive_filename(filename: &str) -> (String, Option<String>) {
        let name = filename.trim_end_matches(".scg");

        // Handle filenames with counter suffix: YYYY-MM-DD_tag_N
        // or just YYYY-MM-DD_tag
        let parts: Vec<&str> = name.splitn(2, '_').collect();

        if parts.len() == 2 {
            let date = parts[0].to_string();
            let rest = parts[1];

            // Check if rest ends with _N (counter suffix)
            // We need to detect if there's a trailing _NUMBER
            if let Some(last_underscore) = rest.rfind('_') {
                let potential_counter = &rest[last_underscore + 1..];
                if potential_counter.chars().all(|c| c.is_ascii_digit())
                    && !potential_counter.is_empty()
                {
                    // Has a counter suffix, extract tag without it
                    let tag_part = &rest[..last_underscore];
                    let tag = if tag_part == "all" {
                        None
                    } else {
                        Some(tag_part.to_string())
                    };
                    return (date, tag);
                }
            }

            // No counter suffix
            let tag = if rest == "all" {
                None
            } else {
                Some(rest.to_string())
            };
            (date, tag)
        } else {
            // Fallback for malformed filenames
            (name.to_string(), None)
        }
    }

    /// List all archives in the archive directory
    /// Returns sorted by date descending (newest first)
    pub fn list_archives(&self) -> Result<Vec<ArchiveInfo>> {
        let archive_dir = self.archive_dir();
        if !archive_dir.exists() {
            return Ok(Vec::new());
        }

        let mut archives = Vec::new();
        for entry in fs::read_dir(&archive_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "scg").unwrap_or(false) {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                let (date, tag) = Self::parse_archive_filename(&filename);

                // Get task count by loading the archive
                let task_count = match self.load_archive(&path) {
                    Ok(phases) => phases.values().map(|p| p.tasks.len()).sum(),
                    Err(_) => 0,
                };

                archives.push(ArchiveInfo {
                    filename,
                    path,
                    date,
                    tag,
                    task_count,
                });
            }
        }

        // Sort by date descending (newest first)
        archives.sort_by(|a, b| b.date.cmp(&a.date));
        Ok(archives)
    }

    /// Load an archive file
    /// Returns the phases contained in the archive
    pub fn load_archive(&self, path: &Path) -> Result<HashMap<String, Phase>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read archive: {}", path.display()))?;

        self.parse_multi_phase_scg(&content)
    }

    /// Restore an archive by merging it into current tasks.
    ///
    /// # Arguments
    /// * `archive_name` - filename or partial match (e.g., "v1", "2026-01-13_v1", "2026-01-13_v1.scg")
    /// * `replace` - if true, replace existing tags; if false, skip existing
    ///
    /// # Returns
    /// The list of restored tag names
    pub fn restore_archive(&self, archive_name: &str, replace: bool) -> Result<Vec<String>> {
        let archive_dir = self.archive_dir();

        // Find matching archive
        let archive_path = if archive_name.ends_with(".scg") {
            let path = archive_dir.join(archive_name);
            if !path.exists() {
                anyhow::bail!("Archive file not found: {}", archive_name);
            }
            path
        } else {
            // Search for matching archive
            let mut found = None;
            if archive_dir.exists() {
                for entry in fs::read_dir(&archive_dir)? {
                    let entry = entry?;
                    let filename = entry.file_name().to_string_lossy().to_string();
                    if filename.contains(archive_name) {
                        found = Some(entry.path());
                        break;
                    }
                }
            }
            found.ok_or_else(|| anyhow::anyhow!("Archive '{}' not found", archive_name))?
        };

        let archived_phases = self.load_archive(&archive_path)?;
        let mut current_phases = self.load_tasks().unwrap_or_default();
        let mut restored_tags = Vec::new();

        for (tag, phase) in archived_phases {
            if replace || !current_phases.contains_key(&tag) {
                current_phases.insert(tag.clone(), phase);
                restored_tags.push(tag);
            }
        }

        self.save_tasks(&current_phases)?;
        Ok(restored_tags)
    }

    /// Create or update CLAUDE.md with SCUD agent instructions
    fn create_agent_instructions(&self) -> Result<()> {
        let claude_md_path = self.project_root.join("CLAUDE.md");

        let scud_instructions = r#"
## SCUD Task Management

This project uses SCUD Task Manager for task management.

### Session Workflow

1. **Start of session**: Run `scud warmup` to orient yourself
   - Shows current working directory and recent git history
   - Displays active tag, task counts, and any stale locks
   - Identifies the next available task

2. **Get a task**: Use `/scud:next` or `scud next`
   - Shows the next available task based on DAG dependencies
   - Use `scud set-status <id> in-progress` to mark you're working on it

3. **Work on the task**: Implement the requirements
   - Reference task details with `/scud:task-show <id>`
   - Dependencies are automatically tracked by the DAG

4. **Commit with context**: Use `scud commit -m "message"` or `scud commit -a -m "message"`
   - Automatically prefixes commits with `[TASK-ID]`
   - Uses task title as default commit message if none provided

5. **Complete the task**: Mark done with `/scud:task-status <id> done`
   - The stop hook will prompt for task completion

### Progress Journaling

Keep a brief progress log during complex tasks:

```
## Progress Log

### Session: 2025-01-15
- Investigated auth module, found issue in token refresh
- Updated refresh logic to handle edge case
- Tests passing, ready for review
```

This helps maintain continuity across sessions and provides context for future work.

### Key Commands

- `scud warmup` - Session orientation
- `scud next` - Find next available task
- `scud show <id>` - View task details
- `scud set-status <id> <status>` - Update task status
- `scud commit` - Task-aware git commit
- `scud stats` - View completion statistics
"#;

        if claude_md_path.exists() {
            // Append to existing CLAUDE.md if SCUD section doesn't exist
            let content = fs::read_to_string(&claude_md_path)
                .with_context(|| "Failed to read existing CLAUDE.md")?;

            if !content.contains("## SCUD Task Management") {
                let mut new_content = content;
                new_content.push_str(scud_instructions);
                fs::write(&claude_md_path, new_content)
                    .with_context(|| "Failed to update CLAUDE.md")?;
            }
        } else {
            // Create new CLAUDE.md
            let content = format!("# Project Instructions\n{}", scud_instructions);
            fs::write(&claude_md_path, content).with_context(|| "Failed to create CLAUDE.md")?;
        }

        Ok(())
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
        tasks.insert("TEST-1".to_string(), Phase::new("TEST-1".to_string()));
        storage.save_tasks(&tasks).unwrap();
        storage.set_active_group("TEST-1").unwrap();

        // First call - loads from file
        let active1 = storage.get_active_group().unwrap();
        assert_eq!(active1, Some("TEST-1".to_string()));

        // Modify file directly (bypass storage methods)
        let active_tag_file = storage.active_tag_file();
        fs::write(&active_tag_file, "DIFFERENT").unwrap();

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
        tasks.insert("EPIC-1".to_string(), Phase::new("EPIC-1".to_string()));
        tasks.insert("EPIC-2".to_string(), Phase::new("EPIC-2".to_string()));
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
            tasks.insert(format!("EPIC-{}", i), Phase::new(format!("EPIC-{}", i)));
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
        let mut epic = Phase::new("TEST-1".to_string());
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
        let mut epic = Phase::new("ACTIVE-1".to_string());
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
        tasks.insert("EPIC-1".to_string(), Phase::new("EPIC-1".to_string()));
        tasks.insert("EPIC-2".to_string(), Phase::new("EPIC-2".to_string()));
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

    // ==================== Archive Tests ====================

    #[test]
    fn test_archive_dir() {
        let (storage, _temp_dir) = create_test_storage();
        let archive_dir = storage.archive_dir();
        assert!(archive_dir.ends_with(".scud/archive"));
    }

    #[test]
    fn test_ensure_archive_dir() {
        let (storage, _temp_dir) = create_test_storage();

        // Directory shouldn't exist initially
        assert!(!storage.archive_dir().exists());

        // Create it
        storage.ensure_archive_dir().unwrap();
        assert!(storage.archive_dir().exists());

        // Second call should be idempotent
        storage.ensure_archive_dir().unwrap();
        assert!(storage.archive_dir().exists());
    }

    #[test]
    fn test_archive_filename_with_tag() {
        let (storage, _temp_dir) = create_test_storage();
        let filename = storage.archive_filename(Some("v1"));

        // Should match pattern YYYY-MM-DD_v1.scg
        assert!(filename.ends_with("_v1.scg"));
        assert!(filename.len() == 17); // YYYY-MM-DD_v1.scg = 17 chars
    }

    #[test]
    fn test_archive_filename_all() {
        let (storage, _temp_dir) = create_test_storage();
        let filename = storage.archive_filename(None);

        // Should match pattern YYYY-MM-DD_all.scg
        assert!(filename.ends_with("_all.scg"));
        assert!(filename.len() == 18); // YYYY-MM-DD_all.scg = 18 chars
    }

    #[test]
    fn test_parse_archive_filename_simple() {
        let (date, tag) = Storage::parse_archive_filename("2026-01-13_v1.scg");
        assert_eq!(date, "2026-01-13");
        assert_eq!(tag, Some("v1".to_string()));
    }

    #[test]
    fn test_parse_archive_filename_all() {
        let (date, tag) = Storage::parse_archive_filename("2026-01-13_all.scg");
        assert_eq!(date, "2026-01-13");
        assert_eq!(tag, None);
    }

    #[test]
    fn test_parse_archive_filename_with_counter() {
        let (date, tag) = Storage::parse_archive_filename("2026-01-13_v1_2.scg");
        assert_eq!(date, "2026-01-13");
        assert_eq!(tag, Some("v1".to_string()));
    }

    #[test]
    fn test_parse_archive_filename_all_with_counter() {
        let (date, tag) = Storage::parse_archive_filename("2026-01-13_all_5.scg");
        assert_eq!(date, "2026-01-13");
        assert_eq!(tag, None);
    }

    #[test]
    fn test_archive_single_phase() {
        let (storage, _temp_dir) = create_test_storage();

        // Create test phases
        let mut phases = HashMap::new();
        let mut phase = Phase::new("v1".to_string());
        phase.add_task(crate::models::Task::new(
            "task-1".to_string(),
            "Test Task".to_string(),
            "Description".to_string(),
        ));
        phases.insert("v1".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        // Archive
        let archive_path = storage.archive_phase("v1", &phases).unwrap();

        assert!(archive_path.exists());
        assert!(archive_path.to_string_lossy().contains("v1"));
        assert!(archive_path.extension().unwrap() == "scg");
    }

    #[test]
    fn test_archive_all_phases() {
        let (storage, _temp_dir) = create_test_storage();

        let mut phases = HashMap::new();
        phases.insert("v1".to_string(), Phase::new("v1".to_string()));
        phases.insert("v2".to_string(), Phase::new("v2".to_string()));
        storage.save_tasks(&phases).unwrap();

        let archive_path = storage.archive_all(&phases).unwrap();

        assert!(archive_path.exists());
        assert!(archive_path.to_string_lossy().contains("all"));

        // Verify it contains both phases
        let loaded = storage.load_archive(&archive_path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded.contains_key("v1"));
        assert!(loaded.contains_key("v2"));
    }

    #[test]
    fn test_archive_nonexistent_tag() {
        let (storage, _temp_dir) = create_test_storage();

        let phases = HashMap::new();
        let result = storage.archive_phase("nonexistent", &phases);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_unique_archive_path_no_collision() {
        let (storage, _temp_dir) = create_test_storage();
        storage.ensure_archive_dir().unwrap();

        let base_path = storage.archive_dir().join("test.scg");
        let result = storage.unique_archive_path(&base_path);

        assert_eq!(result, base_path);
    }

    #[test]
    fn test_unique_archive_path_with_collision() {
        let (storage, _temp_dir) = create_test_storage();
        storage.ensure_archive_dir().unwrap();

        // Create existing file
        let base_path = storage.archive_dir().join("test.scg");
        fs::write(&base_path, "existing").unwrap();

        // Should get test_1.scg
        let result = storage.unique_archive_path(&base_path);
        assert!(result.to_string_lossy().contains("test_1.scg"));
    }

    #[test]
    fn test_unique_archive_path_multiple_collisions() {
        let (storage, _temp_dir) = create_test_storage();
        storage.ensure_archive_dir().unwrap();

        // Create existing files
        let base_path = storage.archive_dir().join("test.scg");
        fs::write(&base_path, "existing").unwrap();
        fs::write(storage.archive_dir().join("test_1.scg"), "existing").unwrap();
        fs::write(storage.archive_dir().join("test_2.scg"), "existing").unwrap();

        // Should get test_3.scg
        let result = storage.unique_archive_path(&base_path);
        assert!(result.to_string_lossy().contains("test_3.scg"));
    }

    #[test]
    fn test_list_archives_empty() {
        let (storage, _temp_dir) = create_test_storage();

        let archives = storage.list_archives().unwrap();
        assert!(archives.is_empty());
    }

    #[test]
    fn test_list_archives_with_archives() {
        let (storage, _temp_dir) = create_test_storage();

        // Create test phases and archive them
        let mut phases = HashMap::new();
        let mut phase = Phase::new("v1".to_string());
        phase.add_task(crate::models::Task::new(
            "task-1".to_string(),
            "Test".to_string(),
            "Desc".to_string(),
        ));
        phases.insert("v1".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        storage.archive_phase("v1", &phases).unwrap();

        let archives = storage.list_archives().unwrap();
        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0].tag, Some("v1".to_string()));
        assert_eq!(archives[0].task_count, 1);
    }

    #[test]
    fn test_load_archive() {
        let (storage, _temp_dir) = create_test_storage();

        let mut phases = HashMap::new();
        let mut phase = Phase::new("test-tag".to_string());
        phase.add_task(crate::models::Task::new(
            "task-1".to_string(),
            "Test Title".to_string(),
            "Test Description".to_string(),
        ));
        phases.insert("test-tag".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        let archive_path = storage.archive_phase("test-tag", &phases).unwrap();

        // Load and verify
        let loaded = storage.load_archive(&archive_path).unwrap();
        assert_eq!(loaded.len(), 1);
        let loaded_phase = loaded.get("test-tag").unwrap();
        assert_eq!(loaded_phase.tasks.len(), 1);
        assert_eq!(loaded_phase.tasks[0].title, "Test Title");
    }

    #[test]
    fn test_restore_archive_empty_tasks() {
        let (storage, _temp_dir) = create_test_storage();

        // Create and archive a phase
        let mut phases = HashMap::new();
        phases.insert("v1".to_string(), Phase::new("v1".to_string()));
        storage.save_tasks(&phases).unwrap();

        let archive_path = storage.archive_phase("v1", &phases).unwrap();
        let archive_name = archive_path.file_name().unwrap().to_str().unwrap();

        // Clear current tasks
        storage.save_tasks(&HashMap::new()).unwrap();
        let empty_check = storage.load_tasks().unwrap();
        assert!(empty_check.is_empty());

        // Restore
        let restored = storage.restore_archive(archive_name, false).unwrap();
        assert_eq!(restored, vec!["v1".to_string()]);

        // Verify restored
        let current = storage.load_tasks().unwrap();
        assert!(current.contains_key("v1"));
    }

    #[test]
    fn test_restore_archive_no_replace() {
        let (storage, _temp_dir) = create_test_storage();

        // Create initial phase with task
        let mut phases = HashMap::new();
        let mut phase = Phase::new("v1".to_string());
        phase.add_task(crate::models::Task::new(
            "original".to_string(),
            "Original".to_string(),
            "Desc".to_string(),
        ));
        phases.insert("v1".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        // Archive it
        let archive_path = storage.archive_phase("v1", &phases).unwrap();
        let archive_name = archive_path.file_name().unwrap().to_str().unwrap();

        // Modify current tasks
        let mut current = storage.load_tasks().unwrap();
        current
            .get_mut("v1")
            .unwrap()
            .add_task(crate::models::Task::new(
                "new".to_string(),
                "New".to_string(),
                "Desc".to_string(),
            ));
        storage.save_tasks(&current).unwrap();

        // Restore without replace - should not overwrite
        let restored = storage.restore_archive(archive_name, false).unwrap();
        assert!(restored.is_empty()); // Nothing restored since v1 exists

        // Verify current still has both tasks
        let final_tasks = storage.load_tasks().unwrap();
        assert_eq!(final_tasks.get("v1").unwrap().tasks.len(), 2);
    }

    #[test]
    fn test_restore_archive_with_replace() {
        let (storage, _temp_dir) = create_test_storage();

        // Create initial phase with task
        let mut phases = HashMap::new();
        let mut phase = Phase::new("v1".to_string());
        phase.add_task(crate::models::Task::new(
            "original".to_string(),
            "Original".to_string(),
            "Desc".to_string(),
        ));
        phases.insert("v1".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        // Archive it
        let archive_path = storage.archive_phase("v1", &phases).unwrap();
        let archive_name = archive_path.file_name().unwrap().to_str().unwrap();

        // Modify current tasks
        let mut current = storage.load_tasks().unwrap();
        current
            .get_mut("v1")
            .unwrap()
            .add_task(crate::models::Task::new(
                "new".to_string(),
                "New".to_string(),
                "Desc".to_string(),
            ));
        storage.save_tasks(&current).unwrap();

        // Restore with replace - should overwrite
        let restored = storage.restore_archive(archive_name, true).unwrap();
        assert_eq!(restored, vec!["v1".to_string()]);

        // Verify archive version restored (only 1 task)
        let final_tasks = storage.load_tasks().unwrap();
        assert_eq!(final_tasks.get("v1").unwrap().tasks.len(), 1);
    }

    #[test]
    fn test_restore_archive_partial_match() {
        let (storage, _temp_dir) = create_test_storage();

        let mut phases = HashMap::new();
        phases.insert("myproject".to_string(), Phase::new("myproject".to_string()));
        storage.save_tasks(&phases).unwrap();

        storage.archive_phase("myproject", &phases).unwrap();

        // Clear and restore using partial name
        storage.save_tasks(&HashMap::new()).unwrap();

        let restored = storage.restore_archive("myproject", false).unwrap();
        assert_eq!(restored, vec!["myproject".to_string()]);
    }

    #[test]
    fn test_restore_archive_not_found() {
        let (storage, _temp_dir) = create_test_storage();

        let result = storage.restore_archive("nonexistent", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}
