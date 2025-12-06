---
date: 2025-12-06T17:38:52Z
researcher: Claude
git_commit: 21b72a750f994b63615b09369947c2f15c4acaea
branch: master
repository: pyrex41/scud
topic: "Tag Management Commands and --all-tags Flag Behavior"
tags: [research, codebase, tags, waves, expand, cli]
status: complete
last_updated: 2025-12-06
last_updated_by: Claude
---

# Research: Tag Management Commands and --all-tags Flag Behavior

**Date**: 2025-12-06T17:38:52Z
**Researcher**: Claude
**Git Commit**: 21b72a750f994b63615b09369947c2f15c4acaea
**Branch**: master
**Repository**: pyrex41/scud

## Research Question

The user asked about:
1. The Skud Help commands being out of date with actual tag management commands
2. The `--all` flag behavior for the `expand` command
3. The `--all-tags` flag behavior for the `waves` command

## Summary

The codebase has two distinct "all" flags with different purposes:

1. **`--all` flag** (expand command only): Expands all tasks **within the current/specified tag** that meet criteria (complexity >= 3)
2. **`--all-tags` flag** (waves, mermaid, reanalyze-deps commands): Operates **across all tags** instead of just one tag

The current help text in the command markdown files is minimal and does not fully explain the nuanced behavior of these flags.

## Detailed Findings

### Help Text Locations

Help text exists in multiple locations:

| Location | Purpose |
|----------|---------|
| `scud-cli/src/main.rs` | Clap argument definitions (authoritative for CLI `--help`) |
| `bin/scud.js` | Node.js shim help screen (lines 45-94) |
| `.claude/commands/scud/*.md` | Claude Code slash commands |
| `.opencode/command/*.md` | OpenCode slash commands |

### Tag Management CLI Commands

#### `scud tags [<tag>]`

**CLI Definition** (`main.rs:76-80`):
```rust
/// List phase tags or set active tag
Tags {
    /// Tag to set as active (lists tags if not provided)
    tag: Option<String>,
}
```

**Current Help Text** (`.claude/commands/scud/task-tags.md`):
- Lists all tags or sets active tag
- Shows available tags and indicates which is active
- No mention of any flags

**Actual Behavior** (`commands/tags.rs:9-98`):
- Without argument: Lists all tags with task counts and shows which is active
- With argument: Sets the specified tag as active

### The `--all` Flag (Expand Command)

**CLI Definition** (`main.rs:202-214`):
```rust
/// Expand complex task into subtasks (AI-powered)
Expand {
    /// Task ID to expand
    task_id: Option<String>,

    /// Expand all tasks with complexity > 13
    #[arg(short, long)]
    all: bool,

    /// Phase tag (uses active phase if not provided)
    #[arg(short, long)]
    tag: Option<String>,
}
```

**Note**: The help text says "complexity > 13" but the actual implementation uses "complexity >= 3".

**Implementation** (`commands/ai/expand.rs:54-102`):

```rust
let tasks_to_expand: Vec<...> =
    if let Some(id) = task_id {
        // Single task expansion
        vec![...]
    } else if expand_all {
        // Expand all tasks in THIS tag that need expansion
        epic.tasks
            .iter()
            .filter(|t| t.needs_expansion())  // complexity >= 3
            .map(...)
            .collect()
    } else {
        anyhow::bail!("Specify a task ID or use --all to expand all tasks with complexity >=3");
    };
```

**Current Behavior**:
- `scud expand --all` expands all tasks **within the current/specified tag** that have complexity >= 3
- It does NOT expand tasks across all tags

### The `--all-tags` Flag (Waves, Mermaid, ReanalyzeDeps)

#### Waves Command

**CLI Definition** (`main.rs:156-169`):
```rust
/// Plan parallel execution waves based on task dependencies
Waves {
    /// Phase tag (uses active phase if not provided)
    #[arg(short, long)]
    tag: Option<String>,

    /// Maximum parallel tasks per round (default: 5, min: 1)
    #[arg(short = 'n', long, default_value = "5")]
    max_parallel: usize,

    /// Plan across all phases
    #[arg(long)]
    all_tags: bool,
}
```

**Implementation** (`commands/waves.rs:29-44`):

```rust
let phase_tags: Vec<String> = if all_tags {
    all_tasks.keys().cloned().collect()  // ALL tags
} else if let Some(t) = tag {
    vec![t.to_string()]  // Specific tag
} else {
    // Use active phase
    let active = storage.get_active_group()?;
    match active {
        Some(t) => vec![t],
        None => anyhow::bail!("No active task group..."),
    }
};
```

**Current Behavior**:
- `scud waves` - Shows waves for **active tag only**
- `scud waves --tag foo` - Shows waves for **tag "foo" only**
- `scud waves --all-tags` - Shows waves for **ALL tags** combined

**Current Help Text** (`.claude/commands/scud/task-waves.md` and `.opencode/command/task-waves.md`):
```
Arguments: `[--tag <tag>] [--max-parallel <n>] [--all-tags]`
```

The help mentions `--all-tags` but doesn't explain what it does.

#### Mermaid Command

**CLI Definition** (`main.rs:332-341`):
```rust
Mermaid {
    /// Phase tag (uses active phase if not provided)
    #[arg(short, long)]
    tag: Option<String>,

    /// Include all phases in the diagram
    #[arg(long)]
    all_tags: bool,
}
```

**Implementation** (`commands/mermaid.rs:8-23`):
Same pattern as waves - collects all phase keys when `all_tags` is true.

#### ReanalyzeDeps Command

**CLI Definition** (`main.rs:216-233`):
```rust
ReanalyzeDeps {
    /// Tag to analyze (default: all tags)
    #[arg(short, long)]
    tag: Option<String>,

    /// Analyze all tags (default if no tag specified)
    #[arg(long)]
    all_tags: bool,
    ...
}
```

**Implementation** (`commands/ai/reanalyze_deps.rs:44-52`):
```rust
let phases_to_analyze: Vec<String> = match tag {
    Some(t) if !all_tags => {
        vec![t.to_string()]
    }
    _ => all_phases.keys().cloned().collect(),  // Default to all
};
```

**Note**: This command defaults to all tags if no tag is specified (unlike waves/mermaid).

### Help Text Inconsistencies

| File | Issue |
|------|-------|
| `bin/scud.js:75` | Shows `expand [<id>] [--all]` but doesn't explain the flag |
| `main.rs:207` | Says "complexity > 13" but code uses >= 3 |
| `.claude/commands/scud/task-waves.md` | Shows `--all-tags` argument but doesn't explain behavior |
| `.opencode/command/task-waves.md` | Same as above |
| Both task-tags.md files | No mention of how tag switching affects other commands |

## Code References

- `scud-cli/src/main.rs:76-80` - Tags command CLI definition
- `scud-cli/src/main.rs:156-169` - Waves command CLI definition with `--all-tags`
- `scud-cli/src/main.rs:202-214` - Expand command CLI definition with `--all`
- `scud-cli/src/main.rs:216-233` - ReanalyzeDeps command CLI definition
- `scud-cli/src/commands/waves.rs:29-44` - Waves `--all-tags` implementation
- `scud-cli/src/commands/ai/expand.rs:54-102` - Expand `--all` implementation
- `scud-cli/src/commands/ai/reanalyze_deps.rs:44-52` - ReanalyzeDeps default behavior
- `.claude/commands/scud/task-tags.md` - Claude Code slash command help
- `.claude/commands/scud/task-waves.md` - Claude Code slash command help
- `bin/scud.js:45-94` - Node.js shim help screen

## Architecture Documentation

### Tag Resolution Priority

All commands that accept `--tag` follow this resolution order (implemented in `helpers.rs:30-80`):

1. **Explicit `--tag`** - Use the specified tag
2. **Active tag** - Use the tag set via `scud tags <tag>`
3. **Interactive selection** - Prompt user to choose (if TTY available)
4. **Error** - Fail if no tag can be determined

### Two Different "All" Concepts

| Flag | Scope | Commands | Description |
|------|-------|----------|-------------|
| `--all` | Within one tag | `expand` | Process all matching tasks in current/specified tag |
| `--all-tags` | Across all tags | `waves`, `mermaid`, `reanalyze-deps` | Process tasks from ALL tags in the project |

## Desired Behavior Changes

The user has clarified the intended behavior for the `expand` command:

### Current vs Desired Expand Command Behavior

| Usage | Current Behavior | Desired Behavior |
|-------|------------------|------------------|
| `scud expand` | Error: requires `--all` or task ID | Expand all tasks in **current tag** |
| `scud expand --all` | Expand all tasks in current tag | Expand all tasks in **ALL tags** |
| `scud expand <task_id>` | Expand specific task | **Remove this form** |
| `scud expand --task <task_id>` | N/A | Expand specific task in current tag |

### Required Code Changes

**1. CLI Definition** (`main.rs:202-214`):
```rust
/// Expand complex task into subtasks (AI-powered)
Expand {
    /// Specific task ID to expand
    #[arg(short = 'i', long)]
    task: Option<String>,

    /// Expand all tasks across ALL tags (default: current tag only)
    #[arg(short, long)]
    all: bool,

    /// Phase tag (uses active phase if not provided)
    #[arg(short, long)]
    tag: Option<String>,
}
```

**2. Implementation** (`commands/ai/expand.rs:54-102`):
- Default behavior (no flags): expand all tasks in current/specified tag
- `--all` flag: expand all tasks across ALL tags
- `--task <id>` flag: expand single task in current/specified tag

**3. Help Text Updates**:
- Fix "complexity > 13" to "complexity >= 3" in main.rs
- Update slash command markdown files to explain the new behavior
