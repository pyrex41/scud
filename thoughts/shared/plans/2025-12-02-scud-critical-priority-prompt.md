# Task: Add Critical Priority Level to SCUD

## Context

We're unifying SCUD and Descartes task management systems. Descartes has 4 priority levels (Low, Medium, High, Critical) while SCUD has 3 (Low, Medium, High). We need to add Critical to SCUD for full compatibility.

## Changes Required

### 1. Update Priority Enum

**File**: `scud-cli/src/models/task.rs`

Find the Priority enum (around line 19585-19592) and add Critical:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    #[default]
    Medium,
    High,
    Critical,  // NEW: Add this variant
}
```

### 2. Update SCG Priority Code Mapping

**File**: `scud-cli/src/formats/scg.rs`

Find `priority_to_code` function (around line 10149) and add Critical mapping:

```rust
fn priority_to_code(priority: &Priority) -> char {
    match priority {
        Priority::High => 'H',
        Priority::Medium => 'M',
        Priority::Low => 'L',
        Priority::Critical => 'C',  // NEW: Add this case
    }
}
```

Find `code_to_priority` function and add the reverse mapping:

```rust
fn code_to_priority(code: char) -> Priority {
    match code {
        'H' => Priority::High,
        'M' => Priority::Medium,
        'L' => Priority::Low,
        'C' => Priority::Critical,  // NEW: Add this case
        _ => Priority::Medium,  // Default
    }
}
```

### 3. Update MCP Tool Schema (if applicable)

**File**: `scud-mcp/src/tools/task.ts` (or similar)

If there's a priority enum in the MCP tool definitions, add "critical" to the allowed values:

```typescript
priority: {
  type: 'string',
  description: 'Task priority level',
  enum: ['low', 'medium', 'high', 'critical'],  // Add 'critical'
}
```

### 4. Update Tests

Add test cases for Critical priority:
- Serialization/deserialization round-trip
- SCG format parsing with 'C' code
- Priority comparison/ordering (Critical > High > Medium > Low)

Example test:

```rust
#[test]
fn test_critical_priority_scg_roundtrip() {
    let task = Task::new("1".to_string(), "Critical task".to_string(), "Desc".to_string());
    task.priority = Priority::Critical;

    let scg = serialize_scg_task(&task);
    assert!(scg.contains("| C"));  // Critical code

    let parsed = parse_scg_task(&scg).unwrap();
    assert_eq!(parsed.priority, Priority::Critical);
}
```

### 5. Update Documentation

- README.md - mention 4 priority levels
- Any help text in CLI that lists priority options
- Update `scud set-status --help` or similar if it shows valid priorities

## Verification

```bash
# Run tests
cargo test -p scud

# Manual verification - create task with critical priority
scud init
scud add "Urgent fix" --priority critical

# Check it shows correctly
scud list  # Should show priority as 'C' or 'critical'
scud show 1  # Should display "Priority: critical"

# Verify SCG file contains 'C' code
cat .scud/tasks/tasks.scg | grep "| C"
```

## Notes

- Critical should be the highest priority (above High)
- Default priority remains Medium
- SCG code 'C' is used (not 'X' which is for Expanded status)
- This is for compatibility with Descartes - no breaking changes to existing SCUD functionality
- Existing .scg files without Critical priority will continue to work (backward compatible)
