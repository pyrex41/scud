# SCUD Graph Format (.scg) Implementation Plan

## Overview

Replace the verbose JSON task storage format with a token-efficient, graph-native format called SCUD Graph (`.scg`). This format explicitly represents the task dependency graph structure while reducing token consumption by ~75% when tasks are read by LLMs.

## Current State Analysis

### JSON Format Issues
- **Token Heavy**: ~180 tokens per task due to repeated field names, quotes, colons
- **Graph Implicit**: Dependencies scattered across individual task objects
- **Redundant**: Many fields serialized even when empty/default
- **Not Graph-Native**: The DAG structure is buried in arrays

### Current File Locations
- Tasks: `.taskmaster/tasks/tasks.json`
- Workflow: `.taskmaster/workflow-state.json` (keeping as JSON per decision 1A)

### Files Requiring Changes
Based on codebase analysis, 21 code locations across 5 files need updates:

| File | Locations | Type |
|------|-----------|------|
| `storage/mod.rs` | 8 | Core serialization |
| `models/task.rs` | 4 | Model + tests |
| `models/epic.rs` | 2 | Model + tests |
| `llm/client.rs` | 2 | LLM response parsing (unchanged - different concern) |
| Storage tests | 5+ | Test data |

## Desired End State

After implementation:
1. Tasks stored in `.taskmaster/tasks/tasks.scg` using new graph format
2. `scud convert` command available for JSON <-> SCG conversion
3. All existing functionality preserved
4. ~75% token reduction when LLMs read task files
5. Human-readable and hand-editable format

### Verification
- All 102 existing tests pass
- New parser/serializer tests pass
- Round-trip conversion: JSON -> SCG -> JSON produces equivalent data
- `scud list`, `scud show`, `scud stats` work identically

## What We're NOT Doing

- NOT changing `workflow-state.json` format (decision 1A)
- NOT supporting automatic format detection (decision 3B - explicit migration)
- NOT changing LLM response parsing (stays JSON, different concern)
- NOT adding complex escaped strings in node rows (decision 4A - separate section)
- NOT storing full complexity_analysis text (simplified to integer score)

## Implementation Approach

The implementation follows a bottom-up strategy:
1. Define the SCG format specification
2. Create parser and serializer in new `formats` module
3. Update Storage to use new format
4. Add `convert` command for migration
5. Update tests
6. Simplify Task model (remove complexity_analysis text)

---

## SCG Format Specification

### File Structure

```
# SCUD Graph v1
# Epic: <epic-tag>

@meta {
  name <epic-tag>
  created <iso8601>
  updated <iso8601>
}

@nodes
# id | title | status | complexity | priority
<id> | <title> | <status> | <complexity> | <priority>
...

@edges
# dependent -> dependency
<id> -> <id>
...

@parents
# parent: subtask1, subtask2, ...
<parent_id>: <subtask_id>, <subtask_id>
...

@assignments
# id | assigned_to | locked_by | locked_at
<id> | <assignee> | <locker> | <iso8601>
...

@details
# id | field | content (multiline via continuation)
<id> | description |
  <multiline content>
  <indented continuation>
<id> | test_strategy |
  <content>
...
```

### Status Codes (Single Character)
| Code | Status |
|------|--------|
| P | Pending |
| I | In-Progress |
| D | Done |
| R | Review |
| B | Blocked |
| F | Deferred |
| C | Cancelled |
| X | Expanded |

### Priority Codes
| Code | Priority |
|------|----------|
| H | High |
| M | Medium |
| L | Low |

### Example

Current JSON (~2400 tokens for 10 tasks):
```json
{
  "auth": {
    "name": "auth",
    "tasks": [
      {
        "id": "1",
        "title": "Design authentication API",
        "description": "Design REST endpoints for user auth",
        "status": "done",
        "complexity": 3,
        "priority": "high",
        "dependencies": [],
        "parent_id": null,
        "subtasks": ["1.1", "1.2"],
        "created_at": "2025-01-01T00:00:00Z",
        "updated_at": "2025-01-01T00:00:00Z"
      },
      ...
    ]
  }
}
```

New SCG format (~600 tokens for same 10 tasks):
```
# SCUD Graph v1
# Epic: auth

@meta {
  name auth
  created 2025-01-01T00:00:00Z
  updated 2025-01-01T00:00:00Z
}

@nodes
# id | title | status | complexity | priority
1 | Design authentication API | X | 3 | H
2 | Implement JWT tokens | P | 5 | M
3 | Add OAuth providers | P | 8 | M
1.1 | Define REST endpoints | D | 0 | H
1.2 | Write OpenAPI spec | D | 0 | H

@edges
2 -> 1
3 -> 2
1.2 -> 1.1

@parents
1: 1.1, 1.2

@details
1 | description |
  Design REST endpoints for user auth
  including login, logout, refresh tokens
2 | description |
  Implement JWT token generation and validation
```

### Escaping Rules
- Pipe `|` in content: `\|`
- Backslash: `\\`
- Newlines in single-line fields: `\n`
- Continuation lines start with 2+ spaces

### Grammar (EBNF)

```ebnf
graph       = header , { section } ;
header      = "# SCUD Graph v1" , NEWLINE , "# Epic:" , IDENT , NEWLINE ;

section     = meta_section | nodes_section | edges_section
            | parents_section | assignments_section | details_section ;

meta_section = "@meta" , "{" , NEWLINE , { meta_pair } , "}" , NEWLINE ;
meta_pair    = IDENT , value , NEWLINE ;

nodes_section = "@nodes" , NEWLINE , { comment } , { node_row } ;
node_row      = ID , "|" , TEXT , "|" , STATUS , "|" , INT , "|" , PRIORITY , NEWLINE ;

edges_section = "@edges" , NEWLINE , { comment } , { edge_row } ;
edge_row      = ID , "->" , ID , NEWLINE ;

parents_section = "@parents" , NEWLINE , { comment } , { parent_row } ;
parent_row      = ID , ":" , ID , { "," , ID } , NEWLINE ;

assignments_section = "@assignments" , NEWLINE , { comment } , { assign_row } ;
assign_row          = ID , "|" , TEXT , "|" , TEXT , "|" , TIMESTAMP , NEWLINE ;

details_section = "@details" , NEWLINE , { comment } , { detail_block } ;
detail_block    = ID , "|" , FIELD_NAME , "|" , NEWLINE , { continuation } ;
continuation    = INDENT , TEXT , NEWLINE ;

comment     = "#" , { any } , NEWLINE ;

STATUS      = "P" | "I" | "D" | "R" | "B" | "F" | "C" | "X" ;
PRIORITY    = "H" | "M" | "L" ;
FIELD_NAME  = "description" | "details" | "test_strategy" ;
ID          = ? alphanumeric, hyphen, underscore, colon, dot ? ;
IDENT       = ? alphanumeric, hyphen, underscore ? ;
TEXT        = ? any chars except unescaped pipe and newline ? ;
TIMESTAMP   = ? ISO8601 datetime ? ;
INT         = ? non-negative integer ? ;
INDENT      = "  " ;  (* 2+ spaces *)
```

---

## Phase 1: Create Format Module

### Overview
Create the `formats` module with SCG parser and serializer.

### Changes Required:

#### 1.1 New File: `src/formats/mod.rs`

**File**: `scud-cli/src/formats/mod.rs`

```rust
//! Task graph serialization formats
//!
//! This module provides parsers and serializers for different
//! task storage formats.

mod scg;

pub use scg::{parse_scg, serialize_scg};

/// Supported file formats
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    /// Legacy JSON format
    Json,
    /// SCUD Graph format (.scg)
    Scg,
}

impl Format {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "json" => Some(Format::Json),
            "scg" => Some(Format::Scg),
            _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Scg => "scg",
        }
    }
}
```

#### 1.2 New File: `src/formats/scg.rs`

**File**: `scud-cli/src/formats/scg.rs`

```rust
//! SCUD Graph (.scg) format parser and serializer
//!
//! A token-efficient, graph-native format for task storage.

use anyhow::{Context, Result};
use std::collections::HashMap;

use crate::models::{Epic, Priority, Task, TaskStatus};

const FORMAT_VERSION: &str = "v1";
const HEADER_PREFIX: &str = "# SCUD Graph";

/// Status code mapping
fn status_to_code(status: &TaskStatus) -> char {
    match status {
        TaskStatus::Pending => 'P',
        TaskStatus::InProgress => 'I',
        TaskStatus::Done => 'D',
        TaskStatus::Review => 'R',
        TaskStatus::Blocked => 'B',
        TaskStatus::Deferred => 'F',
        TaskStatus::Cancelled => 'C',
        TaskStatus::Expanded => 'X',
    }
}

fn code_to_status(code: char) -> Option<TaskStatus> {
    match code {
        'P' => Some(TaskStatus::Pending),
        'I' => Some(TaskStatus::InProgress),
        'D' => Some(TaskStatus::Done),
        'R' => Some(TaskStatus::Review),
        'B' => Some(TaskStatus::Blocked),
        'F' => Some(TaskStatus::Deferred),
        'C' => Some(TaskStatus::Cancelled),
        'X' => Some(TaskStatus::Expanded),
        _ => None,
    }
}

fn priority_to_code(priority: &Priority) -> char {
    match priority {
        Priority::High => 'H',
        Priority::Medium => 'M',
        Priority::Low => 'L',
    }
}

fn code_to_priority(code: char) -> Option<Priority> {
    match code {
        'H' => Some(Priority::High),
        'M' => Some(Priority::Medium),
        'L' => Some(Priority::Low),
        _ => None,
    }
}

/// Escape special characters in text fields
fn escape_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', "\\n")
}

/// Unescape special characters
fn unescape_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => result.push('\\'),
                Some('|') => result.push('|'),
                Some('n') => result.push('\n'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Parse SCG format into Epic
pub fn parse_scg(content: &str) -> Result<Epic> {
    let mut lines = content.lines().peekable();

    // Parse header
    let first_line = lines.next().context("Empty file")?;
    if !first_line.starts_with(HEADER_PREFIX) {
        anyhow::bail!("Invalid SCG header: expected '{}', got '{}'", HEADER_PREFIX, first_line);
    }

    let epic_line = lines.next().context("Missing epic tag line")?;
    let epic_tag = epic_line
        .strip_prefix("# Epic:")
        .map(|s| s.trim())
        .context("Invalid epic line format")?;

    let mut epic = Epic::new(epic_tag.to_string());
    let mut tasks: HashMap<String, Task> = HashMap::new();
    let mut edges: Vec<(String, String)> = Vec::new();
    let mut parents: HashMap<String, Vec<String>> = HashMap::new();
    let mut details: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut assignments: HashMap<String, (Option<String>, Option<String>, Option<String>)> = HashMap::new();

    // Track current section
    let mut current_section: Option<&str> = None;
    let mut current_detail_id: Option<String> = None;
    let mut current_detail_field: Option<String> = None;
    let mut current_detail_content: Vec<String> = Vec::new();

    // Helper to flush current detail
    let flush_detail = |id: &Option<String>, field: &Option<String>, content: &mut Vec<String>, details: &mut HashMap<String, HashMap<String, String>>| {
        if let (Some(id), Some(field)) = (id, field) {
            let text = content.join("\n");
            details.entry(id.clone()).or_default().insert(field.clone(), text);
            content.clear();
        }
    };

    for line in lines {
        let trimmed = line.trim();

        // Skip empty lines and comments (except section headers)
        if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.starts_with("# ")) {
            continue;
        }

        // Check for section headers
        if trimmed.starts_with('@') {
            // Flush any pending detail
            flush_detail(&current_detail_id, &current_detail_field, &mut current_detail_content, &mut details);
            current_detail_id = None;
            current_detail_field = None;

            current_section = Some(match trimmed {
                "@meta {" | "@meta" => "meta",
                "@nodes" => "nodes",
                "@edges" => "edges",
                "@parents" => "parents",
                "@assignments" => "assignments",
                "@details" => "details",
                _ => continue,
            });
            continue;
        }

        // Handle continuation lines in details
        if current_section == Some("details") && line.starts_with("  ") && current_detail_id.is_some() {
            current_detail_content.push(line[2..].to_string());
            continue;
        }

        // Skip meta closing brace and comment lines
        if trimmed == "}" || trimmed.starts_with('#') {
            continue;
        }

        match current_section {
            Some("meta") => {
                // Parse "key value" pairs
                if let Some((key, value)) = trimmed.split_once(char::is_whitespace) {
                    let value = value.trim();
                    // Meta fields are informational, epic name is already set
                    if key == "name" && epic.name != value {
                        epic = Epic::new(value.to_string());
                    }
                }
            }
            Some("nodes") => {
                // Parse "id | title | status | complexity | priority"
                let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
                if parts.len() >= 5 {
                    let id = parts[0].to_string();
                    let title = unescape_text(parts[1]);
                    let status = code_to_status(parts[2].chars().next().unwrap_or('P'))
                        .unwrap_or_default();
                    let complexity: u32 = parts[3].parse().unwrap_or(0);
                    let priority = code_to_priority(parts[4].chars().next().unwrap_or('M'))
                        .unwrap_or_default();

                    let mut task = Task::new(id.clone(), title, String::new());
                    task.status = status;
                    task.complexity = complexity;
                    task.priority = priority;
                    tasks.insert(id, task);
                }
            }
            Some("edges") => {
                // Parse "dependent -> dependency"
                if let Some((dependent, dependency)) = trimmed.split_once("->") {
                    edges.push((dependent.trim().to_string(), dependency.trim().to_string()));
                }
            }
            Some("parents") => {
                // Parse "parent: child1, child2, ..."
                if let Some((parent, children)) = trimmed.split_once(':') {
                    let child_ids: Vec<String> = children
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    parents.insert(parent.trim().to_string(), child_ids);
                }
            }
            Some("assignments") => {
                // Parse "id | assigned_to | locked_by | locked_at"
                let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
                if parts.len() >= 4 {
                    let id = parts[0].to_string();
                    let assigned = if parts[1].is_empty() { None } else { Some(parts[1].to_string()) };
                    let locked_by = if parts[2].is_empty() { None } else { Some(parts[2].to_string()) };
                    let locked_at = if parts[3].is_empty() { None } else { Some(parts[3].to_string()) };
                    assignments.insert(id, (assigned, locked_by, locked_at));
                }
            }
            Some("details") => {
                // Flush previous detail if starting new one
                flush_detail(&current_detail_id, &current_detail_field, &mut current_detail_content, &mut details);

                // Parse "id | field |"
                let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
                if parts.len() >= 2 {
                    current_detail_id = Some(parts[0].to_string());
                    current_detail_field = Some(parts[1].to_string());
                    current_detail_content.clear();
                }
            }
            _ => {}
        }
    }

    // Flush any remaining detail
    flush_detail(&current_detail_id, &current_detail_field, &mut current_detail_content, &mut details);

    // Apply edges (dependencies)
    for (dependent, dependency) in edges {
        if let Some(task) = tasks.get_mut(&dependent) {
            task.dependencies.push(dependency);
        }
    }

    // Apply parent-child relationships
    for (parent_id, child_ids) in parents {
        if let Some(parent) = tasks.get_mut(&parent_id) {
            parent.subtasks = child_ids.clone();
        }
        for child_id in child_ids {
            if let Some(child) = tasks.get_mut(&child_id) {
                child.parent_id = Some(parent_id.clone());
            }
        }
    }

    // Apply details
    for (id, fields) in details {
        if let Some(task) = tasks.get_mut(&id) {
            if let Some(desc) = fields.get("description") {
                task.description = desc.clone();
            }
            if let Some(det) = fields.get("details") {
                task.details = Some(det.clone());
            }
            if let Some(ts) = fields.get("test_strategy") {
                task.test_strategy = Some(ts.clone());
            }
        }
    }

    // Apply assignments
    for (id, (assigned, locked_by, locked_at)) in assignments {
        if let Some(task) = tasks.get_mut(&id) {
            task.assigned_to = assigned;
            task.locked_by = locked_by;
            task.locked_at = locked_at;
        }
    }

    // Add all tasks to epic
    epic.tasks = tasks.into_values().collect();

    // Sort tasks by ID for consistent ordering
    epic.tasks.sort_by(|a, b| {
        // Natural sort: "1" < "2" < "10", "1.1" < "1.2" < "1.10"
        let a_parts: Vec<&str> = a.id.split('.').collect();
        let b_parts: Vec<&str> = b.id.split('.').collect();

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
    });

    Ok(epic)
}

/// Serialize Epic to SCG format
pub fn serialize_scg(epic: &Epic) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!("{} {}\n", HEADER_PREFIX, FORMAT_VERSION));
    output.push_str(&format!("# Epic: {}\n\n", epic.name));

    // Meta section
    let now = chrono::Utc::now().to_rfc3339();
    output.push_str("@meta {\n");
    output.push_str(&format!("  name {}\n", epic.name));
    output.push_str(&format!("  updated {}\n", now));
    output.push_str("}\n\n");

    // Sort tasks for consistent output
    let mut sorted_tasks = epic.tasks.clone();
    sorted_tasks.sort_by(|a, b| {
        let a_parts: Vec<&str> = a.id.split('.').collect();
        let b_parts: Vec<&str> = b.id.split('.').collect();
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
    });

    // Nodes section
    output.push_str("@nodes\n");
    output.push_str("# id | title | status | complexity | priority\n");
    for task in &sorted_tasks {
        output.push_str(&format!(
            "{} | {} | {} | {} | {}\n",
            task.id,
            escape_text(&task.title),
            status_to_code(&task.status),
            task.complexity,
            priority_to_code(&task.priority)
        ));
    }
    output.push('\n');

    // Edges section (dependencies)
    let edges: Vec<_> = sorted_tasks
        .iter()
        .flat_map(|t| t.dependencies.iter().map(move |dep| (&t.id, dep)))
        .collect();

    if !edges.is_empty() {
        output.push_str("@edges\n");
        output.push_str("# dependent -> dependency\n");
        for (dependent, dependency) in edges {
            output.push_str(&format!("{} -> {}\n", dependent, dependency));
        }
        output.push('\n');
    }

    // Parents section
    let parents: Vec<_> = sorted_tasks
        .iter()
        .filter(|t| !t.subtasks.is_empty())
        .collect();

    if !parents.is_empty() {
        output.push_str("@parents\n");
        output.push_str("# parent: subtasks...\n");
        for task in parents {
            output.push_str(&format!("{}: {}\n", task.id, task.subtasks.join(", ")));
        }
        output.push('\n');
    }

    // Assignments section
    let assignments: Vec<_> = sorted_tasks
        .iter()
        .filter(|t| t.assigned_to.is_some() || t.locked_by.is_some())
        .collect();

    if !assignments.is_empty() {
        output.push_str("@assignments\n");
        output.push_str("# id | assigned_to | locked_by | locked_at\n");
        for task in assignments {
            output.push_str(&format!(
                "{} | {} | {} | {}\n",
                task.id,
                task.assigned_to.as_deref().unwrap_or(""),
                task.locked_by.as_deref().unwrap_or(""),
                task.locked_at.as_deref().unwrap_or("")
            ));
        }
        output.push('\n');
    }

    // Details section
    let tasks_with_details: Vec<_> = sorted_tasks
        .iter()
        .filter(|t| !t.description.is_empty() || t.details.is_some() || t.test_strategy.is_some())
        .collect();

    if !tasks_with_details.is_empty() {
        output.push_str("@details\n");
        for task in tasks_with_details {
            if !task.description.is_empty() {
                output.push_str(&format!("{} | description |\n", task.id));
                for line in task.description.lines() {
                    output.push_str(&format!("  {}\n", line));
                }
            }
            if let Some(ref details) = task.details {
                output.push_str(&format!("{} | details |\n", task.id));
                for line in details.lines() {
                    output.push_str(&format!("  {}\n", line));
                }
            }
            if let Some(ref test_strategy) = task.test_strategy {
                output.push_str(&format!("{} | test_strategy |\n", task.id));
                for line in test_strategy.lines() {
                    output.push_str(&format!("  {}\n", line));
                }
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_codes() {
        assert_eq!(status_to_code(&TaskStatus::Pending), 'P');
        assert_eq!(status_to_code(&TaskStatus::InProgress), 'I');
        assert_eq!(status_to_code(&TaskStatus::Done), 'D');
        assert_eq!(status_to_code(&TaskStatus::Expanded), 'X');

        assert_eq!(code_to_status('P'), Some(TaskStatus::Pending));
        assert_eq!(code_to_status('X'), Some(TaskStatus::Expanded));
        assert_eq!(code_to_status('Z'), None);
    }

    #[test]
    fn test_priority_codes() {
        assert_eq!(priority_to_code(&Priority::High), 'H');
        assert_eq!(priority_to_code(&Priority::Medium), 'M');
        assert_eq!(priority_to_code(&Priority::Low), 'L');

        assert_eq!(code_to_priority('H'), Some(Priority::High));
        assert_eq!(code_to_priority('X'), None);
    }

    #[test]
    fn test_escape_unescape() {
        assert_eq!(escape_text("hello|world"), "hello\\|world");
        assert_eq!(escape_text("line1\nline2"), "line1\\nline2");
        assert_eq!(unescape_text("hello\\|world"), "hello|world");
        assert_eq!(unescape_text("line1\\nline2"), "line1\nline2");
    }

    #[test]
    fn test_round_trip() {
        let mut epic = Epic::new("test-epic".to_string());

        let mut task1 = Task::new("1".to_string(), "First task".to_string(), "Description".to_string());
        task1.complexity = 5;
        task1.priority = Priority::High;
        task1.status = TaskStatus::Done;

        let mut task2 = Task::new("2".to_string(), "Second task".to_string(), "Another desc".to_string());
        task2.dependencies = vec!["1".to_string()];
        task2.complexity = 3;

        epic.add_task(task1);
        epic.add_task(task2);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.name, "test-epic");
        assert_eq!(parsed.tasks.len(), 2);

        let t1 = parsed.get_task("1").unwrap();
        assert_eq!(t1.title, "First task");
        assert_eq!(t1.complexity, 5);
        assert_eq!(t1.status, TaskStatus::Done);

        let t2 = parsed.get_task("2").unwrap();
        assert_eq!(t2.dependencies, vec!["1".to_string()]);
    }

    #[test]
    fn test_parent_child() {
        let mut epic = Epic::new("parent-test".to_string());

        let mut parent = Task::new("1".to_string(), "Parent".to_string(), "Parent task".to_string());
        parent.status = TaskStatus::Expanded;
        parent.subtasks = vec!["1.1".to_string(), "1.2".to_string()];

        let mut child1 = Task::new("1.1".to_string(), "Child 1".to_string(), "First child".to_string());
        child1.parent_id = Some("1".to_string());

        let mut child2 = Task::new("1.2".to_string(), "Child 2".to_string(), "Second child".to_string());
        child2.parent_id = Some("1".to_string());
        child2.dependencies = vec!["1.1".to_string()];

        epic.add_task(parent);
        epic.add_task(child1);
        epic.add_task(child2);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        let p = parsed.get_task("1").unwrap();
        assert_eq!(p.subtasks, vec!["1.1", "1.2"]);

        let c1 = parsed.get_task("1.1").unwrap();
        assert_eq!(c1.parent_id, Some("1".to_string()));

        let c2 = parsed.get_task("1.2").unwrap();
        assert_eq!(c2.parent_id, Some("1".to_string()));
        assert_eq!(c2.dependencies, vec!["1.1".to_string()]);
    }
}
```

#### 1.3 Update `src/lib.rs`

**File**: `scud-cli/src/lib.rs`
**Changes**: Add formats module export

```rust
pub mod formats;
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` compiles without errors
- [ ] `cargo test formats` - all format tests pass
- [ ] `cargo clippy` - no warnings

#### Manual Verification:
- [ ] Review generated SCG output for readability

---

## Phase 2: Update Storage Layer

### Overview
Modify Storage to read/write SCG format instead of JSON for tasks.

### Changes Required:

#### 2.1 Update `src/storage/mod.rs`

**File**: `scud-cli/src/storage/mod.rs`
**Changes**:
- Change `tasks_file()` to return `.scg` extension
- Replace `serde_json` calls with format module calls
- Update `load_tasks()` and `save_tasks()`
- Update `load_epic()` and `update_epic()`

Key changes:

```rust
// Line 113: Update file extension
pub fn tasks_file(&self) -> PathBuf {
    self.taskmaster_dir().join("tasks").join("tasks.scg")
}

// Line 202-213: Update load_tasks
pub fn load_tasks(&self) -> Result<HashMap<String, Epic>> {
    let path = self.tasks_file();
    if !path.exists() {
        anyhow::bail!("Tasks file not found: {}\nRun: scud init", path.display());
    }

    let content = self.read_with_lock(&path)?;

    // Parse multi-epic SCG format
    let mut epics = HashMap::new();

    // Split content by epic headers
    let mut current_epic_content = String::new();
    let mut current_epic_tag: Option<String> = None;

    for line in content.lines() {
        if line.starts_with("# Epic:") {
            // Save previous epic if exists
            if let Some(tag) = current_epic_tag.take() {
                if !current_epic_content.is_empty() {
                    let full_content = format!("# SCUD Graph v1\n# Epic: {}\n{}", tag, current_epic_content);
                    let epic = crate::formats::parse_scg(&full_content)
                        .with_context(|| format!("Failed to parse epic '{}'", tag))?;
                    epics.insert(tag, epic);
                }
            }
            current_epic_tag = line.strip_prefix("# Epic:").map(|s| s.trim().to_string());
            current_epic_content.clear();
        } else if line.starts_with("# SCUD Graph") {
            // Skip version header between epics
            continue;
        } else {
            current_epic_content.push_str(line);
            current_epic_content.push('\n');
        }
    }

    // Save last epic
    if let Some(tag) = current_epic_tag {
        if !current_epic_content.is_empty() {
            let full_content = format!("# SCUD Graph v1\n# Epic: {}\n{}", tag, current_epic_content);
            let epic = crate::formats::parse_scg(&full_content)
                .with_context(|| format!("Failed to parse epic '{}'", tag))?;
            epics.insert(tag, epic);
        }
    }

    Ok(epics)
}

// Line 215-221: Update save_tasks
pub fn save_tasks(&self, tasks: &HashMap<String, Epic>) -> Result<()> {
    let path = self.tasks_file();
    self.write_with_lock(&path, || {
        let mut output = String::new();

        // Sort epics by tag for consistent output
        let mut sorted_tags: Vec<_> = tasks.keys().collect();
        sorted_tags.sort();

        for (i, tag) in sorted_tags.iter().enumerate() {
            if i > 0 {
                output.push_str("\n---\n\n");  // Epic separator
            }
            let epic = tasks.get(*tag).unwrap();
            output.push_str(&crate::formats::serialize_scg(epic));
        }

        Ok(output)
    })
}
```

#### 2.2 Update initialization

**File**: `scud-cli/src/storage/mod.rs`
**Changes**: Update `initialize()` to create empty SCG file

```rust
// Around line 151-156
// Initialize tasks.scg with empty content
let tasks_file = self.tasks_file();
if !tasks_file.exists() {
    let empty_tasks: HashMap<String, Epic> = HashMap::new();
    self.save_tasks(&empty_tasks)?;
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` compiles
- [ ] `cargo test` - all 102 tests pass (some may need updates)
- [ ] `cargo clippy` - no warnings

#### Manual Verification:
- [ ] Run `scud init` in test directory, verify `.scg` file created
- [ ] Run `scud parse-prd` and verify SCG format used

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 3: Add Convert Command

### Overview
Add `scud convert` command for explicit migration between formats.

### Changes Required:

#### 3.1 New File: `src/commands/convert.rs`

**File**: `scud-cli/src/commands/convert.rs`

```rust
//! Convert between task storage formats

use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::formats::Format;
use crate::models::Epic;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    from_format: &str,
    to_format: &str,
    backup: bool,
) -> Result<()> {
    let from = Format::from_extension(from_format)
        .ok_or_else(|| anyhow::anyhow!("Unknown format: {}", from_format))?;
    let to = Format::from_extension(to_format)
        .ok_or_else(|| anyhow::anyhow!("Unknown format: {}", to_format))?;

    if from == to {
        println!("{}", "Source and target formats are the same".yellow());
        return Ok(());
    }

    let storage = Storage::new(project_root.clone());
    let taskmaster_dir = storage.taskmaster_dir();
    let tasks_dir = taskmaster_dir.join("tasks");

    // Determine source file
    let source_file = tasks_dir.join(format!("tasks.{}", from.extension()));
    let target_file = tasks_dir.join(format!("tasks.{}", to.extension()));

    if !source_file.exists() {
        anyhow::bail!(
            "Source file not found: {}\nExpected format: {}",
            source_file.display(),
            from_format
        );
    }

    println!(
        "{} {} -> {}",
        "Converting".blue(),
        source_file.display(),
        target_file.display()
    );

    // Read source
    let content = fs::read_to_string(&source_file)
        .with_context(|| format!("Failed to read {}", source_file.display()))?;

    // Parse based on source format
    let epics: HashMap<String, Epic> = match from {
        Format::Json => {
            serde_json::from_str(&content)
                .with_context(|| "Failed to parse JSON")?
        }
        Format::Scg => {
            // Parse multi-epic SCG (same logic as storage)
            parse_multi_epic_scg(&content)?
        }
    };

    println!("  {} epic(s) found", epics.len());
    for (tag, epic) in &epics {
        println!("    {} {} tasks", tag.cyan(), epic.tasks.len());
    }

    // Serialize to target format
    let output = match to {
        Format::Json => {
            serde_json::to_string_pretty(&epics)
                .with_context(|| "Failed to serialize to JSON")?
        }
        Format::Scg => {
            let mut out = String::new();
            let mut sorted_tags: Vec<_> = epics.keys().collect();
            sorted_tags.sort();

            for (i, tag) in sorted_tags.iter().enumerate() {
                if i > 0 {
                    out.push_str("\n---\n\n");
                }
                let epic = epics.get(*tag).unwrap();
                out.push_str(&crate::formats::serialize_scg(epic));
            }
            out
        }
    };

    // Backup if requested
    if backup && source_file.exists() {
        let backup_file = tasks_dir.join(format!("tasks.{}.backup", from.extension()));
        fs::copy(&source_file, &backup_file)
            .with_context(|| format!("Failed to create backup at {}", backup_file.display()))?;
        println!("  {} Backup created: {}", "✓".green(), backup_file.display());
    }

    // Write target
    fs::write(&target_file, &output)
        .with_context(|| format!("Failed to write {}", target_file.display()))?;

    // Remove source if different file
    if source_file != target_file {
        fs::remove_file(&source_file)
            .with_context(|| format!("Failed to remove old file {}", source_file.display()))?;
    }

    println!("\n{}", "✅ Conversion complete!".green().bold());
    println!();
    println!("{}", "Verify with:".blue());
    println!("  scud list");
    println!("  scud stats");

    Ok(())
}

fn parse_multi_epic_scg(content: &str) -> Result<HashMap<String, Epic>> {
    let mut epics = HashMap::new();
    let mut current_epic_content = String::new();
    let mut current_epic_tag: Option<String> = None;

    for line in content.lines() {
        if line.starts_with("# Epic:") {
            if let Some(tag) = current_epic_tag.take() {
                if !current_epic_content.is_empty() {
                    let full_content = format!("# SCUD Graph v1\n# Epic: {}\n{}", tag, current_epic_content);
                    let epic = crate::formats::parse_scg(&full_content)
                        .with_context(|| format!("Failed to parse epic '{}'", tag))?;
                    epics.insert(tag, epic);
                }
            }
            current_epic_tag = line.strip_prefix("# Epic:").map(|s| s.trim().to_string());
            current_epic_content.clear();
        } else if line.starts_with("# SCUD Graph") || line == "---" {
            continue;
        } else {
            current_epic_content.push_str(line);
            current_epic_content.push('\n');
        }
    }

    if let Some(tag) = current_epic_tag {
        if !current_epic_content.is_empty() {
            let full_content = format!("# SCUD Graph v1\n# Epic: {}\n{}", tag, current_epic_content);
            let epic = crate::formats::parse_scg(&full_content)
                .with_context(|| format!("Failed to parse epic '{}'", tag))?;
            epics.insert(tag, epic);
        }
    }

    Ok(epics)
}
```

#### 3.2 Update `src/commands/mod.rs`

**File**: `scud-cli/src/commands/mod.rs`
**Changes**: Add convert module export

```rust
pub mod convert;
```

#### 3.3 Update `src/main.rs`

**File**: `scud-cli/src/main.rs`
**Changes**: Add Convert command variant and handler

```rust
// In Commands enum
/// Convert task storage format
Convert {
    /// Source format (json, scg)
    #[arg(long)]
    from: String,

    /// Target format (json, scg)
    #[arg(long)]
    to: String,

    /// Create backup of source file
    #[arg(long, default_value = "true")]
    backup: bool,
},

// In match handler
Commands::Convert { from, to, backup } => {
    commands::convert::run(project, &from, &to, backup)?;
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` compiles
- [ ] `cargo test` passes
- [ ] `scud convert --help` shows options

#### Manual Verification:
- [ ] Create test JSON file, run `scud convert --from json --to scg`
- [ ] Verify SCG output is valid and readable
- [ ] Convert back: `scud convert --from scg --to json`
- [ ] Verify round-trip produces equivalent data

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 4: Simplify Task Model

### Overview
Remove `complexity_analysis` text field (per decision to keep it simple as integer).

### Changes Required:

#### 4.1 Update `src/models/task.rs`

**File**: `scud-cli/src/models/task.rs`
**Changes**: Remove `complexity_analysis` field

```rust
// Remove this field (around line 100-101):
// #[serde(skip_serializing_if = "Option::is_none")]
// pub complexity_analysis: Option<String>,

// Update Task::new() to remove initialization
// Remove: complexity_analysis: None,

// Remove any methods that reference complexity_analysis
```

#### 4.2 Update AI commands

**File**: `scud-cli/src/commands/ai/analyze_complexity.rs`
**Changes**: Remove complexity_analysis assignment

```rust
// Remove this line (around line 164):
// task.complexity_analysis = Some(analysis.reasoning);

// Just keep:
task.complexity = analysis.complexity;
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` compiles
- [ ] `cargo test` passes
- [ ] No references to `complexity_analysis` remain (except in migrations)

#### Manual Verification:
- [ ] Run `scud analyze-complexity` on test tasks
- [ ] Verify complexity scores are set correctly

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 5: Update Tests

### Overview
Update storage tests to work with SCG format.

### Changes Required:

#### 5.1 Update `src/storage/mod.rs` tests

**File**: `scud-cli/src/storage/mod.rs`
**Changes**: Update test assertions and test data

Key test updates:
- Round-trip tests should verify SCG format
- Error handling tests need SCG-style malformed data
- Remove JSON-specific assertions

#### 5.2 Add format conversion tests

**File**: `scud-cli/src/formats/scg.rs`
**Changes**: Add comprehensive parser tests

```rust
#[test]
fn test_malformed_header() {
    let result = parse_scg("not a valid scg file");
    assert!(result.is_err());
}

#[test]
fn test_empty_epic() {
    let content = "# SCUD Graph v1\n# Epic: empty\n\n@nodes\n";
    let epic = parse_scg(content).unwrap();
    assert_eq!(epic.name, "empty");
    assert!(epic.tasks.is_empty());
}

#[test]
fn test_special_characters_in_title() {
    let mut epic = Epic::new("test".to_string());
    let task = Task::new("1".to_string(), "Task with | pipe".to_string(), "Desc".to_string());
    epic.add_task(task);

    let scg = serialize_scg(&epic);
    let parsed = parse_scg(&scg).unwrap();

    assert_eq!(parsed.get_task("1").unwrap().title, "Task with | pipe");
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo test` - all tests pass (102+ tests)
- [ ] `cargo test formats` - format-specific tests pass
- [ ] `cargo clippy` - no warnings

#### Manual Verification:
- [ ] Review test coverage for edge cases

---

## Phase 6: Documentation and Cleanup

### Overview
Update documentation, remove dead code, final polish.

### Changes Required:

#### 6.1 Update version

**File**: `scud-cli/Cargo.toml`
**Changes**: Bump version to 1.5.0

```toml
version = "1.5.0"
```

#### 6.2 Update help text

**File**: `scud-cli/src/main.rs`
**Changes**: Update command descriptions to mention SCG format

#### 6.3 Remove deprecated code

- Remove any JSON-specific code paths that are no longer used
- Clean up unused imports

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build --release` succeeds
- [ ] `cargo test` all pass
- [ ] `cargo clippy` clean
- [ ] `cargo fmt -- --check` clean

#### Manual Verification:
- [ ] Full workflow test: init -> parse-prd -> analyze -> expand -> list
- [ ] Verify SCG file is readable and well-formatted
- [ ] Test convert command both directions

---

## Testing Strategy

### Unit Tests
- Parser: Valid SCG parsing, malformed input handling
- Serializer: Round-trip integrity, special character escaping
- Status/Priority codes: Mapping correctness

### Integration Tests
- Storage: Load/save with real files
- Convert: JSON <-> SCG round-trip
- Commands: Full workflow with SCG format

### Manual Testing Steps
1. Initialize new project: `scud init`
2. Parse a PRD: `scud parse-prd docs/test.md --tag test`
3. Verify SCG format: `cat .taskmaster/tasks/tasks.scg`
4. List tasks: `scud list`
5. Analyze complexity: `scud analyze-complexity`
6. Expand tasks: `scud expand --all`
7. Test conversion: `scud convert --from scg --to json`
8. Convert back: `scud convert --from json --to scg`
9. Verify data integrity after round-trip

## Performance Considerations

- SCG parsing is O(n) where n = file size
- No regex used in hot paths (simple string operations)
- Task sorting uses natural sort for consistent output
- File locking unchanged from JSON implementation

## Migration Notes

### For Existing Projects
1. Backup existing `tasks.json`
2. Run `scud convert --from json --to scg --backup`
3. Verify with `scud list` and `scud stats`
4. Delete backup after verification

### Rollback Procedure
1. If SCG file corrupted: `scud convert --from json --to scg` (from backup)
2. Or restore from `.taskmaster/tasks/tasks.json.backup`

## References

- JAMS format specification (inspiration)
- DOT graph language (edge syntax inspiration)
- Adjacency list representation (data structure)
