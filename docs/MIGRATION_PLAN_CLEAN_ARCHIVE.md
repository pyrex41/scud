# Plan: SCUD Clean Command with Archive Support

## Overview

Modify the `scud clean` command to archive tasks by default instead of deleting them. This preserves completed work while reducing context clutter when moving between project phases.

## Current Behavior

```bash
scud clean                 # Deletes ALL tasks (with confirmation)
scud clean --tag v1        # Deletes tasks for tag v1
scud clean --force         # Skip confirmation
```

**Problem**: Deleted tasks are gone forever. When working through multiple phases, users lose historical context.

## Desired Behavior

```bash
# Archive by default (moves to .scud/archive/)
scud clean                 # Archives all tasks
scud clean --tag v1        # Archives tag v1 only

# Keep specific tags
scud clean --keep v2       # Archives all EXCEPT v2
scud clean --keep v2,v3    # Archives all EXCEPT v2 and v3

# Actually delete (current behavior)
scud clean --delete        # Permanently deletes all tasks
scud clean --tag v1 --delete  # Permanently deletes v1

# Other options
scud clean --force         # Skip confirmation
scud clean --list          # List archived phases
scud clean --restore v1    # Restore archived phase
```

## Archive Structure

```
.scud/
├── tasks/
│   └── tasks.scg          # Active tasks
├── archive/
│   ├── 2026-01-13_v1.scg  # Archived phase
│   ├── 2026-01-13_v2.scg  # Another archived phase
│   └── 2026-01-10_all.scg # Full archive from clean --all
└── ...
```

Archive naming: `{date}_{tag}.scg` or `{date}_all.scg` for full archives.

---

## Phase 1: Update CLI Options

**Goal**: Add new flags to the Clean command.

**Changes**:

- [ ] Update `Clean` variant in `Commands` enum (`src/main.rs`)

```rust
/// Clear tasks (archives by default, use --delete to permanently remove)
Clean {
    /// Skip confirmation prompt
    #[arg(long)]
    force: bool,

    /// Only clean a specific tag
    #[arg(short, long)]
    tag: Option<String>,

    /// Tags to keep (comma-separated or repeat flag)
    #[arg(long, value_delimiter = ',')]
    keep: Vec<String>,

    /// Permanently delete instead of archiving
    #[arg(long)]
    delete: bool,

    /// List archived phases
    #[arg(long)]
    list: bool,

    /// Restore an archived phase
    #[arg(long)]
    restore: Option<String>,
},
```

- [ ] Update match arm in `main()`

```rust
Commands::Clean {
    force,
    tag,
    keep,
    delete,
    list,
    restore,
} => commands::clean::run(
    cli.project,
    force,
    tag.as_deref(),
    &keep,
    delete,
    list,
    restore.as_deref(),
),
```

**Success Criteria - Automated**:
- [ ] `cargo build` passes
- [ ] `scud clean --help` shows new options

---

## Phase 2: Add Archive Storage Functions

**Goal**: Add archive directory management to Storage.

**Changes**:

- [ ] Add archive methods to `Storage` (`src/storage/mod.rs`)

```rust
impl Storage {
    /// Get the archive directory path
    pub fn archive_dir(&self) -> PathBuf {
        self.scud_dir().join("archive")
    }

    /// Ensure archive directory exists
    pub fn ensure_archive_dir(&self) -> Result<()> {
        let dir = self.archive_dir();
        if !dir.exists() {
            std::fs::create_dir_all(&dir)
                .context("Failed to create archive directory")?;
        }
        Ok(())
    }

    /// Generate archive filename for a tag or all tasks
    pub fn archive_filename(&self, tag: Option<&str>) -> String {
        let date = chrono::Local::now().format("%Y-%m-%d");
        match tag {
            Some(t) => format!("{}_{}.scg", date, t),
            None => format!("{}_all.scg", date),
        }
    }

    /// Archive a single phase/tag
    pub fn archive_phase(&self, tag: &str, phases: &HashMap<String, Phase>) -> Result<PathBuf> {
        self.ensure_archive_dir()?;

        let phase = phases.get(tag)
            .ok_or_else(|| anyhow::anyhow!("Tag '{}' not found", tag))?;

        // Create a single-phase map for serialization
        let mut archive_data: HashMap<String, Phase> = HashMap::new();
        archive_data.insert(tag.to_string(), phase.clone());

        let filename = self.archive_filename(Some(tag));
        let archive_path = self.archive_dir().join(&filename);

        // Handle duplicate filenames by adding counter
        let final_path = self.unique_archive_path(&archive_path);

        // Serialize to SCG format
        let content = crate::formats::scg::serialize(&archive_data)?;
        std::fs::write(&final_path, content)?;

        Ok(final_path)
    }

    /// Archive all phases
    pub fn archive_all(&self, phases: &HashMap<String, Phase>) -> Result<PathBuf> {
        self.ensure_archive_dir()?;

        let filename = self.archive_filename(None);
        let archive_path = self.archive_dir().join(&filename);
        let final_path = self.unique_archive_path(&archive_path);

        let content = crate::formats::scg::serialize(phases)?;
        std::fs::write(&final_path, content)?;

        Ok(final_path)
    }

    /// Get unique path by appending counter if file exists
    fn unique_archive_path(&self, base_path: &PathBuf) -> PathBuf {
        if !base_path.exists() {
            return base_path.clone();
        }

        let stem = base_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("archive");
        let ext = base_path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("scg");
        let parent = base_path.parent().unwrap_or(Path::new("."));

        for i in 1..100 {
            let new_name = format!("{}_{}.{}", stem, i, ext);
            let new_path = parent.join(new_name);
            if !new_path.exists() {
                return new_path;
            }
        }

        // Fallback with timestamp
        let ts = chrono::Local::now().format("%H%M%S");
        parent.join(format!("{}_{}.{}", stem, ts, ext))
    }

    /// List all archives
    pub fn list_archives(&self) -> Result<Vec<ArchiveInfo>> {
        let archive_dir = self.archive_dir();
        if !archive_dir.exists() {
            return Ok(Vec::new());
        }

        let mut archives = Vec::new();
        for entry in std::fs::read_dir(&archive_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "scg").unwrap_or(false) {
                let filename = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                // Parse filename: YYYY-MM-DD_tagname.scg
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

        // Sort by date descending
        archives.sort_by(|a, b| b.date.cmp(&a.date));
        Ok(archives)
    }

    fn parse_archive_filename(filename: &str) -> (String, Option<String>) {
        let name = filename.trim_end_matches(".scg");
        let parts: Vec<&str> = name.splitn(2, '_').collect();

        if parts.len() == 2 {
            let date = parts[0].to_string();
            let tag = if parts[1] == "all" {
                None
            } else {
                Some(parts[1].to_string())
            };
            (date, tag)
        } else {
            (name.to_string(), None)
        }
    }

    /// Load an archive file
    pub fn load_archive(&self, path: &PathBuf) -> Result<HashMap<String, Phase>> {
        let content = std::fs::read_to_string(path)?;
        crate::formats::scg::parse(&content)
    }

    /// Restore an archive (merge into current tasks)
    pub fn restore_archive(&self, archive_name: &str, replace: bool) -> Result<Vec<String>> {
        let archive_dir = self.archive_dir();

        // Find matching archive
        let archive_path = if archive_name.ends_with(".scg") {
            archive_dir.join(archive_name)
        } else {
            // Search for matching archive
            let mut found = None;
            for entry in std::fs::read_dir(&archive_dir)? {
                let entry = entry?;
                let filename = entry.file_name().to_string_lossy().to_string();
                if filename.contains(archive_name) {
                    found = Some(entry.path());
                    break;
                }
            }
            found.ok_or_else(|| anyhow::anyhow!("Archive '{}' not found", archive_name))?
        };

        if !archive_path.exists() {
            anyhow::bail!("Archive file not found: {:?}", archive_path);
        }

        let archived_phases = self.load_archive(&archive_path)?;
        let mut current_phases = self.load_tasks()?;
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
}

/// Information about an archived phase
#[derive(Debug)]
pub struct ArchiveInfo {
    pub filename: String,
    pub path: PathBuf,
    pub date: String,
    pub tag: Option<String>,
    pub task_count: usize,
}
```

**Success Criteria - Automated**:
- [ ] `cargo build` passes
- [ ] Unit tests for archive functions pass

---

## Phase 3: Update Clean Command Implementation

**Goal**: Implement archive-by-default behavior.

**Changes**:

- [ ] Rewrite `src/commands/clean.rs`

```rust
use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    force: bool,
    tag: Option<&str>,
    keep: &[String],
    delete: bool,
    list: bool,
    restore: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Handle --list
    if list {
        return list_archives(&storage);
    }

    // Handle --restore
    if let Some(archive_name) = restore {
        return restore_archive(&storage, archive_name, force);
    }

    let mut all_tasks = storage.load_tasks()?;

    if all_tasks.is_empty() {
        println!("{}", "No tasks to clean.".yellow());
        return Ok(());
    }

    // Determine what to clean (respecting --keep)
    let tags_to_clean: Vec<String> = if let Some(tag_name) = tag {
        if !all_tasks.contains_key(tag_name) {
            anyhow::bail!("Tag '{}' not found", tag_name);
        }
        if keep.contains(&tag_name.to_string()) {
            anyhow::bail!("Cannot clean tag '{}' - it's in the keep list", tag_name);
        }
        vec![tag_name.to_string()]
    } else {
        all_tasks
            .keys()
            .filter(|t| !keep.contains(t))
            .cloned()
            .collect()
    };

    if tags_to_clean.is_empty() {
        println!("{}", "No tags to clean (all kept).".yellow());
        return Ok(());
    }

    // Build confirmation message
    let action = if delete { "delete" } else { "archive" };
    let task_count: usize = tags_to_clean
        .iter()
        .filter_map(|t| all_tasks.get(t))
        .map(|p| p.tasks.len())
        .sum();

    let confirm_msg = if tags_to_clean.len() == 1 {
        format!(
            "{} {} tasks from tag '{}'?",
            action.to_uppercase(),
            task_count.to_string().cyan(),
            tags_to_clean[0].cyan()
        )
    } else {
        let kept_msg = if !keep.is_empty() {
            format!(" (keeping: {})", keep.join(", ").green())
        } else {
            String::new()
        };
        format!(
            "{} {} tasks across {} tags?{}",
            action.to_uppercase(),
            task_count.to_string().cyan(),
            tags_to_clean.len().to_string().cyan(),
            kept_msg
        )
    };

    // Confirm unless --force
    if !force {
        println!();
        if delete {
            println!(
                "{}",
                "⚠ WARNING: --delete permanently removes tasks!".red().bold()
            );
        } else {
            println!(
                "{}",
                "Tasks will be archived to .scud/archive/".blue()
            );
        }
        println!();

        let confirmed = Confirm::new()
            .with_prompt(confirm_msg)
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{}", "Cancelled.".yellow());
            return Ok(());
        }
    }

    // Perform the clean
    if delete {
        // Permanent delete (original behavior)
        for tag_name in &tags_to_clean {
            all_tasks.remove(tag_name);
        }
        println!();
        println!(
            "{} Deleted {} tag(s)",
            "✓".green(),
            tags_to_clean.len()
        );
    } else {
        // Archive first, then remove
        let mut archived_files = Vec::new();

        if tags_to_clean.len() == all_tasks.len() && keep.is_empty() {
            // Archive all at once
            let path = storage.archive_all(&all_tasks)?;
            archived_files.push(path);
            all_tasks.clear();
        } else {
            // Archive individual tags
            for tag_name in &tags_to_clean {
                let path = storage.archive_phase(tag_name, &all_tasks)?;
                archived_files.push(path);
                all_tasks.remove(tag_name);
            }
        }

        println!();
        println!(
            "{} Archived {} tag(s):",
            "✓".green(),
            tags_to_clean.len()
        );
        for path in &archived_files {
            println!(
                "    {}",
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string())
                    .dimmed()
            );
        }
    }

    // Clear active tag if it was cleaned
    for tag_name in &tags_to_clean {
        if let Ok(Some(active)) = storage.get_active_group() {
            if &active == tag_name {
                let _ = storage.clear_active_group();
            }
        }
    }

    storage.save_tasks(&all_tasks)?;

    println!();
    Ok(())
}

fn list_archives(storage: &Storage) -> Result<()> {
    let archives = storage.list_archives()?;

    if archives.is_empty() {
        println!("{}", "No archives found.".yellow());
        println!();
        println!("Archives are created when running: scud clean");
        return Ok(());
    }

    println!();
    println!("{}", "Archived phases:".blue().bold());
    println!();

    for archive in archives {
        let tag_display = archive
            .tag
            .as_ref()
            .map(|t| t.cyan().to_string())
            .unwrap_or_else(|| "(all)".dimmed().to_string());

        println!(
            "  {} {} - {} tasks",
            archive.date.dimmed(),
            tag_display,
            archive.task_count
        );
        println!("    {}", archive.filename.dimmed());
    }

    println!();
    println!(
        "{}",
        "Restore with: scud clean --restore <name>".dimmed()
    );
    println!();

    Ok(())
}

fn restore_archive(storage: &Storage, archive_name: &str, replace: bool) -> Result<()> {
    println!();
    println!("Restoring archive: {}", archive_name.cyan());

    let restored = storage.restore_archive(archive_name, replace)?;

    if restored.is_empty() {
        println!(
            "{}",
            "No tags restored (already exist). Use --force to replace.".yellow()
        );
    } else {
        println!();
        println!("{} Restored {} tag(s):", "✓".green(), restored.len());
        for tag in &restored {
            println!("    {}", tag.cyan());
        }
    }

    println!();
    Ok(())
}
```

**Success Criteria - Automated**:
- [ ] `cargo build` passes
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes

**Success Criteria - Manual**:
- [ ] `scud clean --tag v1` creates archive in `.scud/archive/`
- [ ] `scud clean --list` shows archived phases
- [ ] `scud clean --restore <name>` restores archived phase
- [ ] `scud clean --delete` permanently removes tasks
- [ ] `scud clean --keep v2` archives all except v2

---

## Phase 4: Add Tests

**Goal**: Comprehensive tests for archive functionality.

**Changes**:

- [ ] Add tests to `src/commands/clean.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_storage() -> (TempDir, Storage) {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(Some(temp_dir.path().to_path_buf()));
        storage.init(None).unwrap();
        (temp_dir, storage)
    }

    #[test]
    fn test_archive_single_tag() {
        let (_temp, storage) = setup_test_storage();

        // Create test tasks
        let mut phases = std::collections::HashMap::new();
        phases.insert("v1".to_string(), crate::models::Phase {
            tag: "v1".to_string(),
            tasks: vec![/* test task */],
        });
        storage.save_tasks(&phases).unwrap();

        // Archive
        let path = storage.archive_phase("v1", &phases).unwrap();
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("v1"));
    }

    #[test]
    fn test_archive_all() {
        let (_temp, storage) = setup_test_storage();

        let mut phases = std::collections::HashMap::new();
        phases.insert("v1".to_string(), crate::models::Phase::default());
        phases.insert("v2".to_string(), crate::models::Phase::default());
        storage.save_tasks(&phases).unwrap();

        let path = storage.archive_all(&phases).unwrap();
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("all"));
    }

    #[test]
    fn test_list_archives() {
        let (_temp, storage) = setup_test_storage();

        let mut phases = std::collections::HashMap::new();
        phases.insert("v1".to_string(), crate::models::Phase::default());
        storage.save_tasks(&phases).unwrap();

        storage.archive_phase("v1", &phases).unwrap();

        let archives = storage.list_archives().unwrap();
        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0].tag, Some("v1".to_string()));
    }

    #[test]
    fn test_restore_archive() {
        let (_temp, storage) = setup_test_storage();

        // Create and archive
        let mut phases = std::collections::HashMap::new();
        phases.insert("v1".to_string(), crate::models::Phase::default());
        storage.save_tasks(&phases).unwrap();

        let archive_path = storage.archive_phase("v1", &phases).unwrap();
        let archive_name = archive_path.file_name().unwrap().to_str().unwrap();

        // Clear current tasks
        storage.save_tasks(&std::collections::HashMap::new()).unwrap();

        // Restore
        let restored = storage.restore_archive(archive_name, false).unwrap();
        assert_eq!(restored, vec!["v1".to_string()]);

        let current = storage.load_tasks().unwrap();
        assert!(current.contains_key("v1"));
    }
}
```

**Success Criteria - Automated**:
- [ ] All tests pass

---

## Phase 5: Update Documentation

**Goal**: Document the new archive behavior.

**Changes**:

- [ ] Update help text in CLI (already done in Phase 1)

- [ ] Add to README/CHANGELOG

```markdown
## [1.34.0] - 2026-01-XX

### Changed
- `scud clean` now archives tasks by default instead of deleting
  - Tasks moved to `.scud/archive/{date}_{tag}.scg`
  - Use `--delete` flag to permanently remove tasks
  - Use `--keep <tag>` to exclude tags from cleaning

### Added
- `scud clean --list` - List archived phases
- `scud clean --restore <name>` - Restore archived phase
- `scud clean --keep <tag>` - Keep specific tags when cleaning
```

**Success Criteria - Manual**:
- [ ] `scud clean --help` shows updated documentation
- [ ] README reflects new behavior

---

## Usage Examples

### Basic Archive Operations

```bash
# Archive all tasks (default behavior)
scud clean

# Archive specific tag
scud clean --tag v1

# Archive all except v2
scud clean --keep v2

# Archive all except v2 and v3
scud clean --keep v2,v3
# or
scud clean --keep v2 --keep v3
```

### Viewing and Restoring Archives

```bash
# List archived phases
scud clean --list

# Restore a specific archive
scud clean --restore v1              # Match by tag name
scud clean --restore 2026-01-13_v1   # Match by filename
scud clean --restore 2026-01-13_v1.scg  # Full filename

# Force restore (replace existing tags)
scud clean --restore v1 --force
```

### Permanent Deletion

```bash
# Permanently delete (no archive)
scud clean --delete

# Delete specific tag
scud clean --tag v1 --delete

# Delete without confirmation
scud clean --tag v1 --delete --force
```

### Multi-Phase Workflow

```bash
# Phase 1: Initial development
scud generate prd-phase1.md --tag v1
# ... work on v1 ...

# Archive v1, start phase 2
scud clean --tag v1
scud generate prd-phase2.md --tag v2

# Later: restore v1 to check something
scud clean --list
scud clean --restore v1
```

---

## File Reference Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `src/main.rs` | Modify | Update Clean command options |
| `src/commands/clean.rs` | Rewrite | Implement archive-by-default |
| `src/storage/mod.rs` | Modify | Add archive storage methods |
| `CHANGELOG.md` | Modify | Document new behavior |

---

## Dependencies

- `chrono` crate for date formatting (already a dependency)

---

## Backward Compatibility

- **Breaking**: Default behavior changes from delete to archive
- **Migration**: Users who relied on delete behavior must now use `--delete` flag
- **Risk**: Low - archiving is safer than deleting

Consider adding a deprecation notice in the first release:

```rust
println!("{}", "Note: 'scud clean' now archives by default. Use --delete to permanently remove.".yellow());
```
