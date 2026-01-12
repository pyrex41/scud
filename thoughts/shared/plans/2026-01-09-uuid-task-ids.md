# UUID Task ID Support Implementation Plan

## Overview

Add support for UUID task IDs to SCUD so external tools like Descartes can consume SCUD tasks. Currently SCUD uses sequential numeric IDs ("1", "2", "4.1"), but Descartes expects UUID format (32-character hex strings).

## Current State Analysis

### ID Generation Points
1. **`parse_prd.rs:114`** - Initial task creation uses sequential numbers:
   ```rust
   let task_id = (start_id + idx).to_string();  // "1", "2", "3"
   ```

2. **`expand.rs:295`** - Subtask IDs use dot notation:
   ```rust
   let new_id = format!("{}.{}", parent_id, idx + 1);  // "4.1", "4.2"
   ```

### ID Storage
- `Task.id: String` at `task.rs:72` - Already supports arbitrary string formats
- Parent-child relationships stored explicitly in `parent_id` and `subtasks` fields
- IDs also implicitly encode hierarchy via dot notation (redundant with explicit fields)

### ID Validation
- `task.rs:177-199` - Allows alphanumeric, hyphen, underscore, colon, dot
- Max 100 characters - UUIDs fit (32-36 chars)

### ID Sorting
- `scg.rs:343-363` - `natural_sort_ids()` assumes numeric segments
- Would fail with UUIDs, needs fallback to lexicographic or timestamp-based sorting

### Key Discoveries
- Task model already has `created_at: Option<String>` - can use for ordering
- Parent-child relationships already stored explicitly - don't need dot notation
- ID validation already permissive enough for UUIDs

## Desired End State

After implementation:
1. `scud parse PRD.md --tag phoenix --id-format uuid` generates tasks with UUID IDs
2. `scud expand --task <uuid>` generates subtasks with new UUIDs
3. External tools like Descartes can consume SCUD tasks without ID format errors
4. Existing sequential ID format remains the default for backwards compatibility
5. Human-friendly display shows short prefixes (first 8 chars) in CLI output

### Verification
- Unit tests pass for UUID generation and parsing
- Round-trip test: parse → expand → save → load preserves all data
- `scud list` displays readable output with UUID tasks
- External tool integration works (manual verification with Descartes)

## What We're NOT Doing

- **Migrating existing tasks** - Old projects keep numeric IDs
- **Mixing ID formats** - A phase uses either all UUIDs or all numeric
- **Changing default behavior** - Sequential IDs remain the default
- **Adding UUID validation** - We'll use the `uuid` crate for generation only

## Implementation Approach

Add an `--id-format` CLI option to `scud parse` that controls ID generation. When `uuid` format is selected, generate UUIDs for all new tasks. Update sorting to handle non-numeric IDs gracefully. Store the ID format choice in phase metadata for consistency during expansion.

## Phase 1: Add uuid Crate Dependency

### Overview
Add the `uuid` crate to generate UUIDs.

### Changes Required:

#### 1.1 Cargo.toml

**File**: `scud-cli/Cargo.toml`
**Changes**: Add uuid dependency with v4 feature

```toml
uuid = { version = "1", features = ["v4"] }
```

### Success Criteria:

#### Automated Verification:
- [x] Cargo builds successfully: `cd scud-cli && cargo build`
- [x] No dependency conflicts: `cargo check`

---

## Phase 2: Add ID Format Configuration

### Overview
Add ID format option to parse command and store choice in phase metadata.

### Changes Required:

#### 2.1 CLI Option

**File**: `scud-cli/src/main.rs`
**Changes**: Add `--id-format` argument to Parse command

```rust
/// Parse PRD/phase markdown into tasks (AI-powered)
#[command(alias = "parse-prd")]
Parse {
    // ... existing fields ...

    /// Task ID format: sequential (default) or uuid
    #[arg(long, default_value = "sequential")]
    id_format: String,
},
```

#### 2.2 Phase Metadata

**File**: `scud-cli/src/models/phase.rs` (or wherever Phase is defined)
**Changes**: Add `id_format` field to Phase struct

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum IdFormat {
    #[default]
    Sequential,
    Uuid,
}

// In Phase struct:
#[serde(default)]
pub id_format: IdFormat,
```

#### 2.3 Pass Format to Parse

**File**: `scud-cli/src/main.rs`
**Changes**: Pass id_format to parse command

```rust
Commands::Parse {
    file,
    tag,
    num_tasks,
    append,
    no_guidance,
    id_format,
} => commands::ai::parse_prd::run(
    cli.project, &file, &tag, num_tasks, append, no_guidance,
    &id_format,  // New parameter
).await,
```

### Success Criteria:

#### Automated Verification:
- [x] Compiles: `cargo build`
- [x] CLI help shows new option: `scud parse --help | grep id-format`
- [x] Tests pass: `cargo test`

---

## Phase 3: Implement UUID Generation in parse_prd

### Overview
Generate UUIDs when `--id-format uuid` is specified.

### Changes Required:

#### 3.1 Parse PRD Changes

**File**: `scud-cli/src/commands/ai/parse_prd.rs`
**Changes**:
- Add `id_format` parameter to `run()` function
- Generate UUIDs when format is "uuid"
- Store id_format in Phase

```rust
use uuid::Uuid;

pub async fn run(
    project_root: Option<PathBuf>,
    file_path: &Path,
    tag: &str,
    num_tasks: u32,
    append: bool,
    no_guidance: bool,
    id_format: &str,  // New parameter
) -> Result<()> {
    // ... existing code ...

    let use_uuid = id_format == "uuid";

    // When creating group
    let mut group = if append && all_tasks.contains_key(tag) {
        // ... existing append logic ...
    } else {
        let mut new_group = Phase::new(tag.to_string());
        if use_uuid {
            new_group.id_format = IdFormat::Uuid;
        }
        new_group
    };

    for (idx, parsed) in parsed_tasks.iter().enumerate() {
        let task_id = if use_uuid {
            Uuid::new_v4().to_string().replace("-", "")  // 32-char hex
        } else {
            (start_id + idx).to_string()
        };

        // ... rest of task creation ...
    }

    // ... rest of function ...
}
```

### Success Criteria:

#### Automated Verification:
- [x] Compiles: `cargo build`
- [x] Unit test for UUID format: `cargo test parse_prd`
- [x] Integration: Parse test PRD with `--id-format uuid`, verify IDs are 32-char hex

#### Manual Verification:
- [ ] Run `scud parse test.md --tag test --id-format uuid`
- [ ] Verify `scud list` shows UUID task IDs
- [ ] Verify tasks can be referenced by UUID in `scud show <uuid>`

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 4: Update Expand for UUID Subtasks

### Overview
When expanding a task in a UUID-format phase, generate new UUIDs for subtasks instead of dot notation.

### Changes Required:

#### 4.1 Expand Command Changes

**File**: `scud-cli/src/commands/ai/expand.rs`
**Changes**: Check phase's id_format and generate appropriate subtask IDs

```rust
use uuid::Uuid;

// In the expansion loop, around line 295:
let new_id = if epic.id_format == IdFormat::Uuid {
    Uuid::new_v4().to_string().replace("-", "")
} else {
    format!("{}.{}", parent_id, idx + 1)
};
```

Note: Parent-child relationship is already stored via `parent_id` and `subtasks` fields, so removing dot notation doesn't lose any information.

### Success Criteria:

#### Automated Verification:
- [x] Compiles: `cargo build`
- [x] Tests pass: `cargo test expand`

#### Manual Verification:
- [ ] Create UUID phase, add complex task, run `scud expand --task <uuid>`
- [ ] Verify subtasks have new UUID IDs (not dot notation)
- [ ] Verify parent-child relationship preserved in `scud show <parent-uuid>`

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 5: Fix Natural Sort for UUIDs

### Overview
Update `natural_sort_ids()` to handle UUIDs gracefully by falling back to lexicographic comparison or timestamp-based ordering.

### Changes Required:

#### 5.1 SCG Format Sorting

**File**: `scud-cli/src/formats/scg.rs`
**Changes**: Update natural_sort_ids to handle non-numeric IDs

```rust
/// Natural sort for task IDs with UUID fallback
/// Numeric IDs: "1" < "2" < "10", "1.1" < "1.2" < "1.10"
/// UUIDs: Lexicographic comparison
fn natural_sort_ids(a: &str, b: &str) -> std::cmp::Ordering {
    // Check if both look like numeric IDs (contain only digits and dots)
    let a_is_numeric = a.chars().all(|c| c.is_ascii_digit() || c == '.');
    let b_is_numeric = b.chars().all(|c| c.is_ascii_digit() || c == '.');

    if a_is_numeric && b_is_numeric {
        // Existing numeric sort logic
        let a_parts: Vec<&str> = a.split('.').collect();
        let b_parts: Vec<&str> = b.split('.').collect();

        for (ap, bp) in a_parts.iter().zip(b_parts.iter()) {
            match (ap.parse::<u32>(), bp.parse::<u32>()) {
                (Ok(an), Ok(bn)) => {
                    if an != bn {
                        return an.cmp(&bn);
                    }
                }
                _ => {
                    if ap != bp {
                        return ap.cmp(bp);
                    }
                }
            }
        }
        a_parts.len().cmp(&b_parts.len())
    } else {
        // UUID or mixed: fall back to lexicographic
        a.cmp(b)
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] Compiles: `cargo build`
- [x] Add unit test for UUID sorting: `cargo test natural_sort`
- [x] Existing numeric sort tests still pass

---

## Phase 6: Human-Readable Display

### Overview
Show shortened UUID prefixes in CLI output for readability.

### Changes Required:

#### 6.1 List Command Display

**File**: `scud-cli/src/commands/list.rs`
**Changes**: Truncate long IDs in display (show first 8 chars with "...")

```rust
fn format_task_id(id: &str) -> String {
    if id.len() > 12 {
        format!("{}...", &id[..8])
    } else {
        id.to_string()
    }
}
```

Apply this in the task listing output.

#### 6.2 Show Command Display

**File**: `scud-cli/src/commands/show.rs`
**Changes**: Show full ID in header, shortened in references

### Success Criteria:

#### Automated Verification:
- [x] Compiles: `cargo build`
- [x] Tests pass: `cargo test`

#### Manual Verification:
- [ ] `scud list` shows readable output with truncated UUIDs
- [ ] `scud show <uuid>` shows full UUID in details

---

## Phase 7: SCG Format Updates

### Overview
Store id_format in SCG @meta section for persistence.

### Changes Required:

#### 7.1 Serialize ID Format

**File**: `scud-cli/src/formats/scg.rs`
**Changes**: Add id_format to @meta section

In `serialize_scg()`:
```rust
output.push_str("@meta {\n");
output.push_str(&format!("  name {}\n", phase.name));
output.push_str(&format!("  id_format {}\n", phase.id_format.as_str()));
output.push_str(&format!("  updated {}\n", now));
output.push_str("}\n\n");
```

In `parse_scg()`:
```rust
Some("meta") => {
    if let Some((key, value)) = trimmed.split_once(char::is_whitespace) {
        let value = value.trim();
        match key {
            "name" => { /* existing */ }
            "id_format" => {
                phase.id_format = match value {
                    "uuid" => IdFormat::Uuid,
                    _ => IdFormat::Sequential,
                };
            }
            _ => {}
        }
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] Round-trip test: save and load phase with UUID format
- [x] Existing SCG files (without id_format) load correctly as Sequential

---

## Phase 8: Tests and Documentation

### Overview
Add comprehensive tests and update documentation.

### Changes Required:

#### 8.1 Unit Tests

**File**: `scud-cli/src/commands/ai/parse_prd.rs` (or new test file)
**Changes**: Add tests for UUID generation

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_uuid_format_generates_valid_uuids() {
        // Test that generated IDs are valid 32-char hex strings
    }

    #[test]
    fn test_sequential_format_generates_numbers() {
        // Test default behavior unchanged
    }
}
```

#### 8.2 Integration Tests

Add end-to-end test that:
1. Parses PRD with `--id-format uuid`
2. Expands a task
3. Saves and loads
4. Verifies all relationships preserved

#### 8.3 Documentation

**File**: `scud-cli/README.md` or `docs/`
**Changes**: Document `--id-format` option

```markdown
### UUID Task IDs

For integration with external tools that expect UUID task IDs:

```bash
scud parse requirements.md --tag myproject --id-format uuid
```

This generates tasks with 32-character UUID identifiers instead of sequential numbers.
```

### Success Criteria:

#### Automated Verification:
- [x] All existing tests pass: `cargo test`
- [x] New UUID-specific tests pass
- [x] Clippy clean: `cargo clippy`
- [x] Release build succeeds: `cargo build --release`

#### Manual Verification:
- [x] README includes UUID task ID documentation
- [x] Help text is clear for `--id-format` option

---

## Testing Strategy

### Unit Tests:
- UUID generation produces valid 32-char hex strings
- Sequential format still works (regression test)
- natural_sort_ids handles both formats correctly
- SCG round-trip preserves id_format

### Integration Tests:
- Full workflow: parse → expand → list → show with UUIDs
- Mixed scenarios don't break (can't mix formats in same phase)

### Manual Testing Steps:
1. Parse a PRD with `--id-format uuid`, verify task creation
2. Expand a complex task, verify subtask UUID generation
3. Use `scud list`, verify readable display
4. Test with Descartes integration (external verification)

## Performance Considerations

- UUID generation is fast (v4 UUIDs use random bytes)
- Sorting fallback to lexicographic is O(n log n), same as current
- No performance regression expected

## Migration Notes

- Existing projects continue using sequential IDs (no migration needed)
- New projects can opt into UUID format via `--id-format uuid`
- Cannot convert between formats (would require re-generating all IDs)

## References

- Original issue: Descartes expects UUID task IDs but SCUD generates numeric IDs
- Error: "expected length 32 for simple format, found 1" when Descartes parses "1"
- `uuid` crate: https://docs.rs/uuid/
