# Consolidate Node.js Commands into Rust CLI

## Overview

Move all Node.js-intercepted commands (`init`, `view`) into the Rust CLI so the npm package becomes a pure binary wrapper. This eliminates the hybrid architecture where some commands run in Node.js and others delegate to Rust.

## Current State Analysis

### Hybrid Architecture Problem
- `bin/scud.js` intercepts `view`, `init`, `install`, `status`, `validate` commands
- Other commands delegate to Rust binary via `spawnSync()`
- This creates maintenance burden and inconsistent behavior

### Key Discoveries:
- `scud-cli/src/main.rs:83` - `view` is aliased to `list` (wrong - should open browser)
- `scud-cli/src/commands/init.rs:10-110` - Rust `init` exists but missing agent installation
- `scud-cli/src/commands/config.rs:484-697` - Agent installation logic already exists in Rust
- `bin/scud.js:669-1820` - HTML viewer is ~1150 lines of JS (styles + script)

## Desired End State

After implementation:
1. `scud view` opens an interactive HTML viewer in the browser (implemented in Rust)
2. `scud init` initializes SCUD AND installs all agents/commands automatically
3. `scud install` command is removed (users use `scud config agents add --all`)
4. npm package's `bin/scud.js` simply executes the Rust binary for ALL commands
5. No Node.js code path for any scud command

### Verification:
- `scud view` opens browser with task visualization
- `scud init` in fresh directory creates `.scud/`, `.claude/commands/scud/`, and updates `CLAUDE.md`
- `npm install -g scud-task && scud --version` works without Node.js runtime (binary only)

## What We're NOT Doing

- Removing the npm package entirely (still needed for distribution)
- Changing the HTML viewer's visual design or features
- Modifying other Rust commands
- Supporting `scud status` or `scud validate` in Rust (these can be deprecated)

## Implementation Approach

1. Port the HTML viewer to Rust using embedded string literals
2. Enhance `init` to call agent installation after setup
3. Remove `view` alias from `list` command, create separate `View` command
4. Simplify `bin/scud.js` to pure passthrough
5. Remove `install` command from help/docs

---

## Phase 1: Port HTML Viewer to Rust

### Overview
Create a new `scud view` command in Rust that generates the HTML viewer and opens it in the default browser.

### Changes Required:

#### 1.1 Add Dependencies

**File**: `scud-cli/Cargo.toml`
**Changes**: Add `webbrowser` crate for cross-platform browser opening

```toml
[dependencies]
# ... existing deps ...
webbrowser = "1.0"
```

#### 1.2 Create View Command Module

**File**: `scud-cli/src/commands/view.rs` (new file)
**Changes**: Create the view command implementation

```rust
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use crate::storage::Storage;

// Embed the static assets at compile time
const VIEWER_STYLES: &str = include_str!("../../assets/viewer.css");
const VIEWER_SCRIPT: &str = include_str!("../../assets/viewer.js");

pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    let storage = Storage::new(project_root);
    
    if !storage.is_initialized() {
        anyhow::bail!("SCUD is not initialized. Run: scud init");
    }
    
    // Load tasks from SCG file
    let tasks_data = storage.load_tasks()?;
    
    // Compute waves for each phase
    let waves_data = compute_all_waves(&tasks_data);
    
    // Generate HTML
    let html = generate_viewer_html(&tasks_data, &waves_data);
    
    // Write to temp file
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("scud-view-{}.html", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()));
    
    fs::write(&temp_file, html)?;
    
    println!("✅ Opening SCUD viewer...");
    webbrowser::open(temp_file.to_str().unwrap())?;
    
    Ok(())
}

fn compute_all_waves(tasks_data: &std::collections::HashMap<String, crate::models::Phase>) 
    -> std::collections::HashMap<String, Vec<Wave>> {
    // Reuse logic from commands/waves.rs
    // ... implementation details ...
}

fn generate_viewer_html(
    tasks_data: &std::collections::HashMap<String, crate::models::Phase>,
    waves_data: &std::collections::HashMap<String, Vec<Wave>>,
) -> String {
    let tasks_json = serde_json::to_string_pretty(tasks_data).unwrap_or_default();
    let waves_json = serde_json::to_string_pretty(waves_data).unwrap_or_default();
    
    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>SCUD Task Viewer</title>
  <script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script>
  <style>
{styles}
  </style>
</head>
<body>
  <!-- ... HTML structure ... -->
  <script>
    const TASKS_DATA = {tasks_json};
    const WAVES_DATA = {waves_json};
{script}
  </script>
</body>
</html>"#,
        styles = VIEWER_STYLES,
        tasks_json = tasks_json,
        waves_json = waves_json,
        script = VIEWER_SCRIPT)
}
```

#### 1.3 Create Asset Files

**File**: `scud-cli/assets/viewer.css` (new file)
**Changes**: Extract CSS from `bin/scud.js:746-1202` into separate file

**File**: `scud-cli/assets/viewer.js` (new file)  
**Changes**: Extract JavaScript from `bin/scud.js:1208-1819` into separate file

#### 1.4 Update Commands Module

**File**: `scud-cli/src/commands/mod.rs`
**Changes**: Add view module

```rust
pub mod view;  // Add this line
```

#### 1.5 Update Main CLI - Remove view alias from List

**File**: `scud-cli/src/main.rs`
**Changes**: 
1. Remove `#[command(alias = "view")]` from `List` command (line 83)
2. Add new `View` command variant

```rust
// Change line 82-100 from:
#[command(alias = "view")]
List { ... }

// To:
List { ... }  // Remove alias

// Add new command after List:
/// Open interactive task viewer in browser
View,
```

#### 1.6 Update Main Match Statement

**File**: `scud-cli/src/main.rs`
**Changes**: Add match arm for View command

```rust
// Add in the match statement (around line 370):
Commands::View => commands::view::run(cli.project),
```

### Success Criteria:

#### Automated Verification:
- [x] Rust compiles: `cd scud-cli && cargo build --release`
- [x] Unit tests pass: `cd scud-cli && cargo test`
- [x] `./target/release/scud view --help` shows view command help

#### Manual Verification:
- [ ] `scud view` opens browser with task viewer
- [ ] All four tabs work (Tasks, Waves, Diagram, Stats)
- [ ] Task detail panel opens when clicking tasks
- [ ] Mermaid diagrams render correctly
- [ ] Pan/zoom works on diagrams

**Implementation Note**: After completing this phase, pause for manual verification before proceeding.

---

## Phase 2: Enhance Init Command

### Overview
Modify `scud init` to automatically install all SCUD agents, skills, and commands after creating the `.scud/` directory structure.

### Changes Required:

#### 2.1 Update Init Command

**File**: `scud-cli/src/commands/init.rs`
**Changes**: Call agent installation after successful init

```rust
use crate::commands::config;

pub fn run(project_root: Option<PathBuf>, provider_arg: Option<String>) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if storage.is_initialized() {
        println!("{}", "✓ SCUD is already initialized".green());
        return Ok(());
    }

    // ... existing provider selection and config creation ...

    storage.initialize_with_config(&config)?;

    println!("\n{}", "✅ SCUD initialized successfully!".green().bold());
    
    // NEW: Auto-install all agents and commands
    println!("\n{}", "Installing SCUD agents and commands...".blue());
    if let Err(e) = config::agents_add(project_root.clone(), None, true) {
        println!("{}", format!("⚠ Could not install agents: {}", e).yellow());
        println!("  You can install them later with: scud config agents add --all");
    }
    
    // NEW: Update CLAUDE.md with SCUD instructions
    if let Err(e) = update_claude_md(&storage) {
        println!("{}", format!("⚠ Could not update CLAUDE.md: {}", e).yellow());
    }

    // ... rest of existing output ...
    
    Ok(())
}

fn update_claude_md(storage: &Storage) -> Result<()> {
    let claude_md_path = storage.project_root().join("CLAUDE.md");
    
    let scud_section = r#"
## SCUD Task Management

This project uses SCUD for AI-driven task management.

### Quick Start
- `scud tags` - List available phases
- `scud next` - Find next available task  
- `scud set-status <id> in-progress` - Claim a task
- `scud view` - Open interactive task viewer

### Slash Commands
Use `/scud:` commands in Claude Code for task operations.
"#;

    let marker = "## SCUD Task Management";
    
    if claude_md_path.exists() {
        let content = std::fs::read_to_string(&claude_md_path)?;
        if content.contains(marker) {
            return Ok(()); // Already has SCUD section
        }
        // Append to existing file
        let new_content = format!("{}\n{}", content.trim_end(), scud_section);
        std::fs::write(&claude_md_path, new_content)?;
    } else {
        // Create new file
        std::fs::write(&claude_md_path, scud_section.trim_start())?;
    }
    
    println!("  {} Updated CLAUDE.md with SCUD instructions", "✓".green());
    Ok(())
}
```

### Success Criteria:

#### Automated Verification:
- [x] Rust compiles: `cd scud-cli && cargo build --release`
- [x] Unit tests pass: `cd scud-cli && cargo test`

#### Manual Verification:
- [ ] Fresh `scud init` creates `.scud/` directory
- [ ] Fresh `scud init` creates `.claude/commands/scud/*.md` files
- [ ] Fresh `scud init` creates `.claude/skills/scud-tasks/` directory
- [ ] Fresh `scud init` adds SCUD section to CLAUDE.md
- [ ] Running `scud init` twice doesn't duplicate CLAUDE.md content

**Implementation Note**: Test in a fresh directory to verify full init flow.

---

## Phase 3: Simplify NPM Package

### Overview
Remove Node.js command interception so the npm package is a pure binary wrapper.

### Changes Required:

#### 3.1 Simplify bin/scud.js

**File**: `bin/scud.js`
**Changes**: Replace entire file with simple binary passthrough

```javascript
#!/usr/bin/env node

/**
 * SCUD CLI - Binary wrapper
 * All commands are handled by the Rust binary
 */

const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');

// Find the Rust binary
const homedir = require('os').homedir();
const cargoBinary = path.join(homedir, '.cargo', 'bin', 'scud');
const localRelease = path.join(__dirname, '..', 'scud-cli', 'target', 'release', 'scud');
const localDebug = path.join(__dirname, '..', 'scud-cli', 'target', 'debug', 'scud');

let scudBinary = null;
if (fs.existsSync(cargoBinary)) {
  scudBinary = cargoBinary;
} else if (fs.existsSync(localRelease)) {
  scudBinary = localRelease;
} else if (fs.existsSync(localDebug)) {
  scudBinary = localDebug;
}

if (!scudBinary) {
  console.error('❌ SCUD CLI not found.');
  console.error('   Install with: cargo install scud-cli');
  console.error('   Or build locally: cd scud-cli && cargo build --release');
  process.exit(1);
}

// Pass all arguments to the Rust binary
const result = spawnSync(scudBinary, process.argv.slice(2), { stdio: 'inherit' });
process.exit(result.status || 0);
```

#### 3.2 Remove Unused Files

**Files to delete**:
- `bin/install.js` - No longer needed (Rust handles init)
- `src/validators/scud-validator.js` - No longer needed

#### 3.3 Update package.json

**File**: `package.json`
**Changes**: Remove unused dependencies and scripts

```json
{
  "dependencies": {
    // Remove "open" - no longer needed
  }
}
```

#### 3.4 Update Help Text in Rust

**File**: `scud-cli/src/main.rs`
**Changes**: Remove references to `scud install` command in help/about text

### Success Criteria:

#### Automated Verification:
- [x] `npm install` completes without errors
- [x] `node bin/scud.js --version` shows version
- [ ] `node bin/scud.js view` opens browser (manual test)

#### Manual Verification:
- [ ] All commands work through the simplified wrapper
- [ ] `scud init` in fresh project works end-to-end
- [ ] `scud view` works after init
- [ ] No Node.js-specific errors appear

**Implementation Note**: Test with both `cargo install` and npm install scenarios.

---

## Phase 4: Documentation Updates

### Overview
Update documentation to reflect the consolidated architecture.

### Changes Required:

#### 4.1 Update README

**File**: `README.md` (or equivalent)
**Changes**: 
- Remove references to `scud install` command
- Update init documentation to mention automatic agent installation
- Add `scud view` documentation

#### 4.2 Update Help Text

**File**: `scud-cli/src/main.rs`  
**Changes**: Ensure command descriptions are accurate

### Success Criteria:

#### Automated Verification:
- [x] `scud --help` shows correct commands
- [x] `scud init --help` shows correct description
- [x] `scud view --help` shows correct description

#### Manual Verification:
- [x] README accurately describes current behavior (updated)
- [x] No references to removed `install` command (verified)

---

## Testing Strategy

### Unit Tests:
- Wave computation produces correct output
- HTML generation includes all required elements
- CLAUDE.md update is idempotent

### Integration Tests:
- Full init → view workflow in fresh directory
- Agent files are correctly installed
- Browser opens with valid HTML

### Manual Testing Steps:
1. Create fresh directory, run `scud init`, verify all files created
2. Run `scud view`, verify browser opens with working viewer
3. Click through all tabs (Tasks, Waves, Diagram, Stats)
4. Verify task detail panel works
5. Verify Mermaid diagrams render
6. Run `scud init` again, verify no duplicates in CLAUDE.md

## Performance Considerations

- HTML generation should be fast (<100ms)
- Embedded assets increase binary size by ~50KB (acceptable)
- Browser opening is async, command returns immediately

## Migration Notes

- Users with existing installations will need to update
- `scud install` should print deprecation warning pointing to `config agents add --all`
- Old Node.js wrapper will continue working during transition

## References

- Research document: `thoughts/shared/research/2025-12-12-npm-wrapper-command-interception.md`
- Current Node.js viewer: `bin/scud.js:669-1820`
- Rust agent installation: `scud-cli/src/commands/config.rs:484-697`
- Rust init: `scud-cli/src/commands/init.rs`
