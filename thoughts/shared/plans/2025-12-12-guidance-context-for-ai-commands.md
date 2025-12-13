# Guidance Context for AI Commands Implementation Plan

## Overview

Add automatic guidance context loading from `.scud/guidance/` folder into AI prompts for both `parse` and `expand` commands. Documents in this folder will be automatically included in parsing and task expansion prompts to provide project-specific context. Add a `--no-guidance` flag to both commands to bypass this behavior.

## Current State Analysis

- `parse_prd.rs:55` - Calls `Prompts::parse_prd(&prd_content, num_tasks)` with just the PRD content
- `expand.rs:196-202` - Calls `Prompts::expand_task()` with title, description, complexity, details, and recommended subtasks
- `prompts.rs` - Contains prompt templates that need guidance context injection
- `storage/mod.rs` - Has `read_file()` method and `scud_dir()` for accessing `.scud/` folder
- `main.rs` - Command definitions with clap derive macros

### Key Discoveries:
- Storage already has `scud_dir()` method returning `.scud/` path
- Storage has `read_file()` for reading arbitrary files
- Both AI commands already use Storage for file operations
- Prompts module uses format strings that can easily accommodate additional context

## Desired End State

1. `.scud/guidance/` folder is automatically created during `scud init`
2. All `.md` files in `.scud/guidance/` are automatically loaded and included in:
   - `scud parse` prompts
   - `scud expand` prompts
3. Both commands have `--no-guidance` flag to skip loading guidance
4. Guidance content appears in prompts under a clear "Project Guidance" section
5. README and Quick Reference document the new feature

### Verification:
- Create `.scud/guidance/coding-standards.md` with content
- Run `scud parse` and verify guidance appears in LLM prompt (via debug output)
- Run `scud parse --no-guidance` and verify guidance is NOT included
- Same verification for `scud expand`

## What We're NOT Doing

- No recursive subdirectory scanning (only top-level files)
- No file type filtering beyond `.md` extension
- No maximum file size limits (user responsibility)
- No caching of guidance content (re-read each time)
- No guidance for other AI commands (analyze-complexity, reanalyze-deps)

## Implementation Approach

1. Add guidance loading method to Storage
2. Update Prompts functions to accept optional guidance parameter
3. Add `--no-guidance` flag to both commands in main.rs
4. Pass guidance to prompt functions from command implementations
5. Create guidance folder during init
6. Update documentation

---

## Phase 1: Add Guidance Loading to Storage

### Overview
Add a new method to Storage that reads all `.md` files from `.scud/guidance/` and concatenates them into a single string.

### Changes Required:

#### 1.1 Storage Module

**File**: `scud-cli/src/storage/mod.rs`
**Changes**: Add `guidance_dir()` and `load_guidance()` methods

```rust
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
        .filter(|e| {
            e.path().extension()
                .map(|ext| ext == "md")
                .unwrap_or(false)
        })
        .collect();

    // Sort by filename for consistent ordering
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let filename = path.file_name()
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
                eprintln!("Warning: Failed to read guidance file {}: {}", path.display(), e);
            }
        }
    }

    Ok(guidance_content)
}
```

#### 1.2 Create Guidance Directory on Init

**File**: `scud-cli/src/storage/mod.rs`
**Changes**: Update `initialize_with_config()` to create guidance directory

In the `initialize_with_config` function, after creating docs directories, add:

```rust
// Create guidance directory
fs::create_dir_all(scud_dir.join("guidance"))?;
```

### Success Criteria:

#### Automated Verification:
- [x] Code compiles: `cargo build -p scud`
- [x] Existing tests pass: `cargo test -p scud`
- [x] `scud init` creates `.scud/guidance/` directory

#### Manual Verification:
- [ ] Create a test guidance file and verify `load_guidance()` reads it

---

## Phase 2: Update Prompts Module

### Overview
Modify prompt functions to accept an optional guidance parameter and include it in the prompts.

### Changes Required:

#### 2.1 Update parse_prd Function

**File**: `scud-cli/src/llm/prompts.rs`
**Changes**: Add guidance parameter to `parse_prd()`

```rust
pub fn parse_prd(phase_content: &str, num_tasks: u32, guidance: Option<&str>) -> String {
    let guidance_section = guidance
        .filter(|g| !g.is_empty())
        .map(|g| format!(
            r#"

## Project Guidance

The following project-specific guidance should inform your task breakdown:

{}

"#, g))
        .unwrap_or_default();

    format!(
        r#"You are a Scrum Master parsing a phase into actionable development tasks.
{}
Phase Content:
{}

Parse this phase into approximately {} discrete, actionable tasks. Return a JSON array of tasks with the following structure:
..."#,
        guidance_section, phase_content, num_tasks
        // ... rest of prompt unchanged
    )
}
```

#### 2.2 Update expand_task Function

**File**: `scud-cli/src/llm/prompts.rs`
**Changes**: Add guidance parameter to `expand_task()`

```rust
pub fn expand_task(
    task_title: &str,
    task_description: &str,
    complexity: u32,
    existing_details: Option<&str>,
    recommended_subtasks: usize,
    guidance: Option<&str>,
) -> String {
    let context = existing_details
        .map(|d| format!("\nExisting Technical Details:\n{}\n", d))
        .unwrap_or_default();

    let guidance_section = guidance
        .filter(|g| !g.is_empty())
        .map(|g| format!(
            r#"

## Project Guidance

The following project-specific guidance should inform your subtask breakdown:

{}

"#, g))
        .unwrap_or_default();

    format!(
        r#"You are breaking down a development task into smaller, manageable subtasks.
{}
Original Task (Complexity {}): {}
Description: {}{}

Break this task down into approximately {} subtasks based on its complexity.
..."#,
        guidance_section, complexity, task_title, task_description, context, recommended_subtasks
        // ... rest of prompt unchanged
    )
}
```

### Success Criteria:

#### Automated Verification:
- [x] Code compiles: `cargo build -p scud` (deferred - callers need updating)
- [ ] Tests pass: `cargo test -p scud`

#### Manual Verification:
- [ ] Prompt functions accept new parameter correctly

---

## Phase 3: Update Commands with --no-guidance Flag

### Overview
Add `--no-guidance` flag to both `parse` and `expand` commands, and wire up guidance loading.

### Changes Required:

#### 3.1 Update Parse Command Definition

**File**: `scud-cli/src/main.rs`
**Changes**: Add `--no-guidance` flag to Parse command

```rust
/// Parse PRD/phase markdown into tasks (AI-powered)
#[command(alias = "parse-prd")]
Parse {
    /// Path to PRD/phase markdown file
    file: PathBuf,

    /// Phase tag to create
    #[arg(short, long)]
    tag: String,

    /// Number of tasks to generate (default: 10)
    #[arg(short = 'n', long, default_value = "10")]
    num_tasks: u32,

    /// Append tasks to existing tag instead of replacing
    #[arg(long)]
    append: bool,

    /// Skip loading guidance from .scud/guidance/
    #[arg(long)]
    no_guidance: bool,
},
```

#### 3.2 Update Expand Command Definition

**File**: `scud-cli/src/main.rs`
**Changes**: Add `--no-guidance` flag to Expand command

```rust
/// Expand complex task into subtasks (AI-powered)
Expand {
    /// Specific task ID to expand (expands all in current tag if not provided)
    #[arg(short = 'i', long)]
    task: Option<String>,

    /// Expand all tasks across ALL tags (default: current tag only)
    #[arg(short, long)]
    all: bool,

    /// Phase tag (uses active phase if not provided)
    #[arg(short, long)]
    tag: Option<String>,

    /// Skip loading guidance from .scud/guidance/
    #[arg(long)]
    no_guidance: bool,
},
```

#### 3.3 Update Parse Command Handler

**File**: `scud-cli/src/main.rs`
**Changes**: Pass `no_guidance` to parse_prd::run

Update the match arm:
```rust
Commands::Parse {
    file,
    tag,
    num_tasks,
    append,
    no_guidance,
} => commands::ai::parse_prd::run(cli.project, &file, &tag, num_tasks, append, no_guidance).await,
```

#### 3.4 Update Expand Command Handler

**File**: `scud-cli/src/main.rs`
**Changes**: Pass `no_guidance` to expand::run

Update the match arm:
```rust
Commands::Expand { task, all, tag, no_guidance } => {
    commands::ai::expand::run(cli.project, task.as_deref(), all, tag.as_deref(), no_guidance).await
}
```

#### 3.5 Update parse_prd.rs Implementation

**File**: `scud-cli/src/commands/ai/parse_prd.rs`
**Changes**: Load guidance and pass to prompt

Update function signature:
```rust
pub async fn run(
    project_root: Option<PathBuf>,
    file_path: &Path,
    tag: &str,
    num_tasks: u32,
    append: bool,
    no_guidance: bool,
) -> Result<()> {
```

After reading PRD content, add:
```rust
// Load guidance unless disabled
let guidance = if no_guidance {
    None
} else {
    match storage.load_guidance() {
        Ok(g) if !g.is_empty() => {
            println!("{}", "Loading project guidance...".blue());
            Some(g)
        }
        Ok(_) => None,
        Err(e) => {
            eprintln!("{} Failed to load guidance: {}", "Warning:".yellow(), e);
            None
        }
    }
};
```

Update prompt call:
```rust
let prompt = Prompts::parse_prd(&prd_content, num_tasks, guidance.as_deref());
```

#### 3.6 Update expand.rs Implementation

**File**: `scud-cli/src/commands/ai/expand.rs`
**Changes**: Load guidance and pass to prompt

Update function signature:
```rust
pub async fn run(
    project_root: Option<PathBuf>,
    task_id: Option<&str>,
    all_tags: bool,
    tag: Option<&str>,
    no_guidance: bool,
) -> Result<()> {
```

After creating client, add:
```rust
// Load guidance unless disabled
let guidance = if no_guidance {
    None
} else {
    match storage.load_guidance() {
        Ok(g) if !g.is_empty() => {
            println!("{}", "Loading project guidance...".blue());
            Some(g)
        }
        Ok(_) => None,
        Err(e) => {
            eprintln!("{} Failed to load guidance: {}", "Warning:".yellow(), e);
            None
        }
    }
};
let guidance_ref = guidance.as_deref();
```

Update the prompt call inside the async block (need to clone guidance for the async move):
```rust
let guidance_clone = guidance_ref.map(|s| s.to_string());
// ... inside async move block:
let prompt = Prompts::expand_task(
    &title,
    &description,
    complexity,
    details.as_deref(),
    recommended_subtasks,
    guidance_clone.as_deref(),
);
```

### Success Criteria:

#### Automated Verification:
- [x] Code compiles: `cargo build -p scud`
- [x] Tests pass: `cargo test -p scud`
- [x] `scud parse --help` shows `--no-guidance` flag
- [x] `scud expand --help` shows `--no-guidance` flag

#### Manual Verification:
- [ ] Create `.scud/guidance/test.md` with content
- [ ] Run `scud parse` and see "Loading project guidance..." message
- [ ] Run `scud parse --no-guidance` and do NOT see guidance message
- [ ] Same for `scud expand`

---

## Phase 4: Update Documentation

### Overview
Update README.md and QUICK_REFERENCE.md to document the new guidance feature.

### Changes Required:

#### 4.1 Update README.md

**File**: `README.md`
**Changes**: Add guidance section and update command references

Add new section after "File Structure":
```markdown
### Project Guidance

You can provide project-specific context that will be automatically included in AI prompts. Create markdown files in `.scud/guidance/`:

```bash
# Example: Add coding standards
echo "# Coding Standards
- Use TypeScript strict mode
- All functions must have JSDoc comments
- Maximum function length: 50 lines" > .scud/guidance/coding-standards.md

# Example: Add architecture notes
echo "# Architecture
- Frontend: React with hooks
- Backend: Express.js
- Database: PostgreSQL" > .scud/guidance/architecture.md
```

All `.md` files in this folder are automatically loaded when running `scud parse` or `scud expand`. Use `--no-guidance` to skip loading guidance.
```

Update AI Commands section to mention `--no-guidance`:
```markdown
### AI Commands (Requires XAI_API_KEY)
```bash
scud parse <file> --tag <tag>      # Parse PRD/doc into tasks
scud parse <file> --tag <tag> --no-guidance  # Parse without project guidance
scud analyze-complexity            # Analyze task complexity
scud expand --all                  # Break down complex tasks
scud expand --all --no-guidance    # Expand without project guidance
```
```

Update File Structure to include guidance folder:
```markdown
## File Structure

```
.scud/
├── tasks/tasks.scg           # All tasks in SCG format
├── config.toml               # Provider/model settings
├── active-tag                # Currently active tag
├── current-task              # Active task ID (for commits)
├── guidance/                 # Project guidance for AI prompts
│   └── *.md                  # Markdown files auto-loaded
└── logs/                     # Task log entries
```
```

#### 4.2 Update QUICK_REFERENCE.md

**File**: `docs/reference/QUICK_REFERENCE.md`
**Changes**: Add guidance section and flag references

Update AI Commands section:
```markdown
### AI Commands (require XAI_API_KEY)
```bash
scud parse <file> --tag <tag>      # Parse PRD into tasks
scud parse <file> --tag <tag> --no-guidance  # Skip guidance
scud analyze-complexity            # Score all tasks
scud analyze-complexity --task <id> # Score specific task
scud expand <id>                   # Split complex task
scud expand --all                  # Split all tasks >13
scud expand --all --no-guidance    # Skip guidance
```
```

Add new section before "Troubleshooting":
```markdown
## Project Guidance

Provide project context for AI commands by adding `.md` files to `.scud/guidance/`:

```bash
.scud/guidance/
├── coding-standards.md    # Your coding conventions
├── architecture.md        # System architecture notes
└── tech-stack.md          # Technology decisions
```

Files are automatically loaded for `parse` and `expand`. Use `--no-guidance` to skip.
```

### Success Criteria:

#### Automated Verification:
- [x] README.md updated with guidance documentation
- [x] QUICK_REFERENCE.md updated with guidance documentation

#### Manual Verification:
- [ ] Documentation is clear and accurate
- [ ] Examples are helpful

---

## Testing Strategy

### Unit Tests:
- Test `load_guidance()` with empty directory
- Test `load_guidance()` with multiple .md files
- Test `load_guidance()` with non-.md files (should be ignored)
- Test `load_guidance()` when directory doesn't exist

### Integration Tests:
- Run full `scud parse` with guidance files
- Verify guidance appears in generated prompts

### Manual Testing Steps:
1. Run `scud init` in a new directory, verify `.scud/guidance/` exists
2. Create `.scud/guidance/test.md` with "Use TypeScript"
3. Run `scud parse some-prd.md --tag test` and verify "Loading project guidance..." appears
4. Run `scud parse some-prd.md --tag test --no-guidance` and verify no guidance message
5. Run `scud expand --all` with guidance and verify message appears
6. Run `scud expand --all --no-guidance` and verify no guidance message

## Performance Considerations

- Guidance files are read synchronously on each command invocation
- No caching implemented (keeps implementation simple)
- Large guidance files will add to prompt token count
- Users should keep guidance concise to avoid token limits

## References

- Related commands: `scud-cli/src/commands/ai/parse_prd.rs`, `scud-cli/src/commands/ai/expand.rs`
- Prompts: `scud-cli/src/llm/prompts.rs`
- Storage: `scud-cli/src/storage/mod.rs`
