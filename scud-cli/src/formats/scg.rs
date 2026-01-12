//! SCUD Graph (.scg) format parser and serializer
//!
//! A token-efficient, graph-native format for task storage.

use anyhow::{Context, Result};
use std::collections::HashMap;

use crate::models::{IdFormat, Phase, Priority, Task, TaskStatus};

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
        Priority::Critical => 'C',
        Priority::High => 'H',
        Priority::Medium => 'M',
        Priority::Low => 'L',
    }
}

fn code_to_priority(code: char) -> Option<Priority> {
    match code {
        'C' => Some(Priority::Critical),
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

/// Split a line by pipe character, respecting escaped pipes
fn split_by_pipe(line: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            // Check for escaped pipe or backslash
            if let Some(&next) = chars.peek() {
                if next == '|' || next == '\\' {
                    current.push(c);
                    current.push(chars.next().unwrap());
                    continue;
                }
            }
            current.push(c);
        } else if c == '|' {
            parts.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
    }
    parts.push(current.trim().to_string());
    parts
}

/// Parse SCG format into Phase
pub fn parse_scg(content: &str) -> Result<Phase> {
    let mut lines = content.lines().peekable();

    // Parse header
    let first_line = lines.next().context("Empty file")?;
    if !first_line.starts_with(HEADER_PREFIX) {
        anyhow::bail!(
            "Invalid SCG header: expected '{}', got '{}'",
            HEADER_PREFIX,
            first_line
        );
    }

    let phase_line = lines.next().context("Missing phase tag line")?;
    let phase_tag = phase_line
        .strip_prefix("# Phase:")
        .or_else(|| phase_line.strip_prefix("# Epic:")) // backwards compatibility
        .map(|s| s.trim())
        .context("Invalid phase line format")?;

    let mut phase = Phase::new(phase_tag.to_string());
    let mut tasks: HashMap<String, Task> = HashMap::new();
    let mut edges: Vec<(String, String)> = Vec::new();
    let mut parents: HashMap<String, Vec<String>> = HashMap::new();
    let mut details: HashMap<String, HashMap<String, String>> = HashMap::new();
    // Type: (assigned_to, locked_by, locked_at)
    type AssignmentInfo = (Option<String>, Option<String>, Option<String>);
    let mut assignments: HashMap<String, AssignmentInfo> = HashMap::new();

    // Track current section
    let mut current_section: Option<&str> = None;
    let mut current_detail_id: Option<String> = None;
    let mut current_detail_field: Option<String> = None;
    let mut current_detail_content: Vec<String> = Vec::new();

    for line in lines {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Check for section headers
        if trimmed.starts_with('@') {
            // Flush any pending detail
            flush_detail(
                &current_detail_id,
                &current_detail_field,
                &mut current_detail_content,
                &mut details,
            );
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
        if current_section == Some("details")
            && line.starts_with("  ")
            && current_detail_id.is_some()
        {
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
                    match key {
                        "name" => {
                            if phase.name != value {
                                phase = Phase::new(value.to_string());
                            }
                        }
                        "id_format" => {
                            phase.id_format = IdFormat::parse(value);
                        }
                        _ => {
                            // Ignore other meta fields (e.g., "updated")
                        }
                    }
                }
            }
            Some("nodes") => {
                // Parse "id | title | status | complexity | priority"
                let parts = split_by_pipe(trimmed);
                if parts.len() >= 5 {
                    let id = parts[0].clone();
                    let title = unescape_text(&parts[1]);
                    let status =
                        code_to_status(parts[2].chars().next().unwrap_or('P')).unwrap_or_default();
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
                // Parse "id | assigned_to" (new format) or "id | assigned_to | locked_by | locked_at" (legacy)
                let parts = split_by_pipe(trimmed);
                if parts.len() >= 2 {
                    let id = parts[0].clone();
                    let assigned = if parts[1].is_empty() {
                        None
                    } else {
                        Some(parts[1].clone())
                    };
                    // Legacy fields (locked_by, locked_at) are ignored if present
                    let locked_by: Option<String> = None;
                    let locked_at: Option<String> = None;
                    assignments.insert(id, (assigned, locked_by, locked_at));
                }
            }
            Some("details") => {
                // Flush previous detail if starting new one
                flush_detail(
                    &current_detail_id,
                    &current_detail_field,
                    &mut current_detail_content,
                    &mut details,
                );

                // Parse "id | field |"
                let parts = split_by_pipe(trimmed);
                if parts.len() >= 2 {
                    current_detail_id = Some(parts[0].clone());
                    current_detail_field = Some(parts[1].clone());
                    current_detail_content.clear();
                }
            }
            _ => {}
        }
    }

    // Flush any remaining detail
    flush_detail(
        &current_detail_id,
        &current_detail_field,
        &mut current_detail_content,
        &mut details,
    );

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

    // Apply assignments (informational only, no locking)
    for (id, (assigned, _locked_by, _locked_at)) in assignments {
        if let Some(task) = tasks.get_mut(&id) {
            task.assigned_to = assigned;
        }
    }

    // Add all tasks to phase
    phase.tasks = tasks.into_values().collect();

    // Sort tasks by ID for consistent ordering
    phase.tasks.sort_by(|a, b| natural_sort_ids(&a.id, &b.id));

    Ok(phase)
}

/// Natural sort for task IDs with UUID fallback
/// Numeric IDs: "1" < "2" < "10", "1.1" < "1.2" < "1.10"
/// UUIDs: Lexicographic comparison
pub fn natural_sort_ids(a: &str, b: &str) -> std::cmp::Ordering {
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

/// Helper to flush current detail
fn flush_detail(
    id: &Option<String>,
    field: &Option<String>,
    content: &mut Vec<String>,
    details: &mut HashMap<String, HashMap<String, String>>,
) {
    if let (Some(id), Some(field)) = (id, field) {
        let text = content.join("\n");
        details
            .entry(id.clone())
            .or_default()
            .insert(field.clone(), text);
        content.clear();
    }
}

/// Serialize Phase to SCG format
pub fn serialize_scg(phase: &Phase) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!("{} {}\n", HEADER_PREFIX, FORMAT_VERSION));
    output.push_str(&format!("# Phase: {}\n\n", phase.name));

    // Meta section
    let now = chrono::Utc::now().to_rfc3339();
    output.push_str("@meta {\n");
    output.push_str(&format!("  name {}\n", phase.name));
    output.push_str(&format!("  id_format {}\n", phase.id_format.as_str()));
    output.push_str(&format!("  updated {}\n", now));
    output.push_str("}\n\n");

    // Sort tasks for consistent output
    let mut sorted_tasks = phase.tasks.clone();
    sorted_tasks.sort_by(|a, b| natural_sort_ids(&a.id, &b.id));

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

    // Assignments section (informational only, no locking)
    let assignments: Vec<_> = sorted_tasks
        .iter()
        .filter(|t| t.assigned_to.is_some())
        .collect();

    if !assignments.is_empty() {
        output.push_str("@assignments\n");
        output.push_str("# id | assigned_to\n");
        for task in assignments {
            output.push_str(&format!(
                "{} | {}\n",
                task.id,
                task.assigned_to.as_deref().unwrap_or("")
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
        assert_eq!(priority_to_code(&Priority::Critical), 'C');
        assert_eq!(priority_to_code(&Priority::High), 'H');
        assert_eq!(priority_to_code(&Priority::Medium), 'M');
        assert_eq!(priority_to_code(&Priority::Low), 'L');

        assert_eq!(code_to_priority('C'), Some(Priority::Critical));
        assert_eq!(code_to_priority('H'), Some(Priority::High));
        assert_eq!(code_to_priority('M'), Some(Priority::Medium));
        assert_eq!(code_to_priority('L'), Some(Priority::Low));
        assert_eq!(code_to_priority('Z'), None);
    }

    #[test]
    fn test_escape_unescape() {
        assert_eq!(escape_text("hello|world"), "hello\\|world");
        assert_eq!(escape_text("line1\nline2"), "line1\\nline2");
        assert_eq!(unescape_text("hello\\|world"), "hello|world");
        assert_eq!(unescape_text("line1\\nline2"), "line1\nline2");
    }

    #[test]
    fn test_id_format_round_trip() {
        use crate::models::IdFormat;

        // Test UUID format persists through SCG round-trip
        let mut phase = Phase::new("uuid-phase".to_string());
        phase.id_format = IdFormat::Uuid;

        let task = Task::new(
            "a1b2c3d4e5f6789012345678901234ab".to_string(),
            "UUID Task".to_string(),
            "Description".to_string(),
        );
        phase.add_task(task);

        let scg = serialize_scg(&phase);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.id_format, IdFormat::Uuid);
        assert_eq!(parsed.name, "uuid-phase");
    }

    #[test]
    fn test_id_format_default_sequential() {
        // Test that phases without id_format default to Sequential
        let content = r#"# SCUD Graph v1
# Phase: old-phase

@meta {
  name old-phase
  updated 2025-01-01T00:00:00Z
}

@nodes
# id | title | status | complexity | priority
1 | Task | P | 0 | M
"#;
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.id_format, IdFormat::Sequential);
    }

    #[test]
    fn test_round_trip() {
        let mut epic = Phase::new("test-epic".to_string());

        let mut task1 = Task::new(
            "1".to_string(),
            "First task".to_string(),
            "Description".to_string(),
        );
        task1.complexity = 5;
        task1.priority = Priority::High;
        task1.status = TaskStatus::Done;

        let mut task2 = Task::new(
            "2".to_string(),
            "Second task".to_string(),
            "Another desc".to_string(),
        );
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
        let mut epic = Phase::new("parent-test".to_string());

        let mut parent = Task::new(
            "1".to_string(),
            "Parent".to_string(),
            "Parent task".to_string(),
        );
        parent.status = TaskStatus::Expanded;
        parent.subtasks = vec!["1.1".to_string(), "1.2".to_string()];

        let mut child1 = Task::new(
            "1.1".to_string(),
            "Child 1".to_string(),
            "First child".to_string(),
        );
        child1.parent_id = Some("1".to_string());

        let mut child2 = Task::new(
            "1.2".to_string(),
            "Child 2".to_string(),
            "Second child".to_string(),
        );
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

    #[test]
    fn test_malformed_header() {
        let result = parse_scg("not a valid scg file");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_phase() {
        let content = "# SCUD Graph v1\n# Phase: empty\n\n@nodes\n# id | title | status | complexity | priority\n";
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.name, "empty");
        assert!(phase.tasks.is_empty());
    }

    #[test]
    fn test_special_characters_in_title() {
        let mut epic = Phase::new("test".to_string());
        let task = Task::new(
            "1".to_string(),
            "Task with | pipe".to_string(),
            "Desc".to_string(),
        );
        epic.add_task(task);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.get_task("1").unwrap().title, "Task with | pipe");
    }

    #[test]
    fn test_multiline_description() {
        let mut epic = Phase::new("test".to_string());
        let task = Task::new(
            "1".to_string(),
            "Task".to_string(),
            "Line 1\nLine 2\nLine 3".to_string(),
        );
        epic.add_task(task);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        let t = parsed.get_task("1").unwrap();
        assert_eq!(t.description, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_assignments() {
        let mut epic = Phase::new("test".to_string());
        let mut task = Task::new("1".to_string(), "Task".to_string(), "Desc".to_string());
        task.assigned_to = Some("alice".to_string());
        epic.add_task(task);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        let t = parsed.get_task("1").unwrap();
        assert_eq!(t.assigned_to, Some("alice".to_string()));
    }

    #[test]
    fn test_natural_sort_order() {
        let mut epic = Phase::new("test".to_string());

        // Add tasks in random order
        for id in ["10", "2", "1", "1.10", "1.2", "1.1"] {
            let task = Task::new(id.to_string(), format!("Task {}", id), String::new());
            epic.add_task(task);
        }

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        let ids: Vec<&str> = parsed.tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["1", "1.1", "1.2", "1.10", "2", "10"]);
    }

    #[test]
    fn test_natural_sort_uuids() {
        // Test UUID sorting falls back to lexicographic
        use super::natural_sort_ids;

        // UUIDs should sort lexicographically
        let uuid_a = "a1b2c3d4e5f6789012345678901234ab";
        let uuid_b = "b1b2c3d4e5f6789012345678901234ab";
        let uuid_c = "c1b2c3d4e5f6789012345678901234ab";

        assert_eq!(natural_sort_ids(uuid_a, uuid_b), std::cmp::Ordering::Less);
        assert_eq!(natural_sort_ids(uuid_b, uuid_c), std::cmp::Ordering::Less);
        assert_eq!(natural_sort_ids(uuid_a, uuid_a), std::cmp::Ordering::Equal);

        // Mixed UUIDs should also sort lexicographically
        let mut ids = vec![uuid_c, uuid_a, uuid_b];
        ids.sort_by(|a, b| natural_sort_ids(a, b));
        assert_eq!(ids, vec![uuid_a, uuid_b, uuid_c]);
    }

    #[test]
    fn test_natural_sort_mixed_numeric_uuid() {
        // When mixing numeric and UUID IDs, fall back to lexicographic
        use super::natural_sort_ids;

        let numeric = "123";
        let uuid = "a1b2c3d4e5f6789012345678901234ab";

        // "123" < "a1b2..." lexicographically
        assert_eq!(natural_sort_ids(numeric, uuid), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_all_statuses() {
        let mut epic = Phase::new("test".to_string());

        let statuses = [
            ("1", TaskStatus::Pending),
            ("2", TaskStatus::InProgress),
            ("3", TaskStatus::Done),
            ("4", TaskStatus::Review),
            ("5", TaskStatus::Blocked),
            ("6", TaskStatus::Deferred),
            ("7", TaskStatus::Cancelled),
            ("8", TaskStatus::Expanded),
        ];

        for (id, status) in &statuses {
            let mut task = Task::new(id.to_string(), format!("Task {}", id), String::new());
            task.status = status.clone();
            epic.add_task(task);
        }

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        for (id, expected_status) in statuses {
            let task = parsed.get_task(id).unwrap();
            assert_eq!(
                task.status, expected_status,
                "Status mismatch for task {}",
                id
            );
        }
    }

    #[test]
    fn test_all_priorities() {
        let mut epic = Phase::new("test".to_string());

        let priorities = [
            ("1", Priority::Critical),
            ("2", Priority::High),
            ("3", Priority::Medium),
            ("4", Priority::Low),
        ];

        for (id, priority) in &priorities {
            let mut task = Task::new(id.to_string(), format!("Task {}", id), String::new());
            task.priority = priority.clone();
            epic.add_task(task);
        }

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        for (id, expected_priority) in priorities {
            let task = parsed.get_task(id).unwrap();
            assert_eq!(
                task.priority, expected_priority,
                "Priority mismatch for task {}",
                id
            );
        }
    }

    #[test]
    fn test_details_and_test_strategy() {
        let mut epic = Phase::new("test".to_string());
        let mut task = Task::new(
            "1".to_string(),
            "Task".to_string(),
            "Description".to_string(),
        );
        task.details = Some("Detailed implementation notes".to_string());
        task.test_strategy = Some("Unit tests and integration tests".to_string());
        epic.add_task(task);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        let t = parsed.get_task("1").unwrap();
        assert_eq!(t.description, "Description");
        assert_eq!(t.details, Some("Detailed implementation notes".to_string()));
        assert_eq!(
            t.test_strategy,
            Some("Unit tests and integration tests".to_string())
        );
    }

    // Additional edge case tests

    #[test]
    fn test_backslash_escape() {
        let mut epic = Phase::new("test".to_string());
        let task = Task::new(
            "1".to_string(),
            "Task with \\ backslash".to_string(),
            "Desc".to_string(),
        );
        epic.add_task(task);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(
            parsed.get_task("1").unwrap().title,
            "Task with \\ backslash"
        );
    }

    #[test]
    fn test_multiple_special_chars() {
        let mut epic = Phase::new("test".to_string());
        let task = Task::new(
            "1".to_string(),
            "Task with | pipe and \\ backslash".to_string(),
            "Line 1\nLine 2 with | and \\".to_string(),
        );
        epic.add_task(task);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        let t = parsed.get_task("1").unwrap();
        assert_eq!(t.title, "Task with | pipe and \\ backslash");
        assert_eq!(t.description, "Line 1\nLine 2 with | and \\");
    }

    #[test]
    fn test_unicode_content() {
        let mut epic = Phase::new("unicode-test".to_string());
        let task = Task::new(
            "1".to_string(),
            "日本語タイトル 🚀 Émojis".to_string(),
            "描述 with émojis 😀".to_string(),
        );
        epic.add_task(task);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        let t = parsed.get_task("1").unwrap();
        assert_eq!(t.title, "日本語タイトル 🚀 Émojis");
        assert_eq!(t.description, "描述 with émojis 😀");
    }

    #[test]
    fn test_empty_dependencies() {
        let mut epic = Phase::new("test".to_string());
        let task = Task::new("1".to_string(), "Task".to_string(), "Desc".to_string());
        epic.add_task(task);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        assert!(parsed.get_task("1").unwrap().dependencies.is_empty());
    }

    #[test]
    fn test_multiple_dependencies() {
        let mut epic = Phase::new("test".to_string());

        let task1 = Task::new("1".to_string(), "Task 1".to_string(), String::new());
        let task2 = Task::new("2".to_string(), "Task 2".to_string(), String::new());
        let mut task3 = Task::new("3".to_string(), "Task 3".to_string(), String::new());
        task3.dependencies = vec!["1".to_string(), "2".to_string()];

        epic.add_task(task1);
        epic.add_task(task2);
        epic.add_task(task3);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        let t3 = parsed.get_task("3").unwrap();
        assert_eq!(t3.dependencies.len(), 2);
        assert!(t3.dependencies.contains(&"1".to_string()));
        assert!(t3.dependencies.contains(&"2".to_string()));
    }

    #[test]
    fn test_complexity_boundary_values() {
        let mut epic = Phase::new("test".to_string());

        // Test all Fibonacci complexity values
        let complexities: Vec<u32> = vec![0, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89];
        for (i, c) in complexities.iter().enumerate() {
            let mut task = Task::new(
                format!("{}", i + 1),
                format!("Task {}", i + 1),
                String::new(),
            );
            task.complexity = *c;
            epic.add_task(task);
        }

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        for (i, expected) in complexities.iter().enumerate() {
            let task = parsed.get_task(&format!("{}", i + 1)).unwrap();
            assert_eq!(
                task.complexity,
                *expected,
                "Complexity mismatch for task {}",
                i + 1
            );
        }
    }

    #[test]
    fn test_long_description() {
        let mut epic = Phase::new("test".to_string());
        let long_desc = "A".repeat(5000); // Max description length
        let task = Task::new("1".to_string(), "Task".to_string(), long_desc.clone());
        epic.add_task(task);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.get_task("1").unwrap().description, long_desc);
    }

    #[test]
    fn test_empty_description() {
        let mut epic = Phase::new("test".to_string());
        let task = Task::new("1".to_string(), "Task".to_string(), String::new());
        epic.add_task(task);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.get_task("1").unwrap().description, "");
    }

    #[test]
    fn test_whitespace_handling() {
        // Ensure whitespace in titles is preserved
        let mut epic = Phase::new("test".to_string());
        let task = Task::new(
            "1".to_string(),
            "  Task with   spaces  ".to_string(),
            "Desc".to_string(),
        );
        epic.add_task(task);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        // After round-trip, leading/trailing spaces in title should be trimmed
        // (this is expected behavior from split/trim)
        let t = parsed.get_task("1").unwrap();
        assert_eq!(t.title, "Task with   spaces");
    }

    #[test]
    fn test_nested_subtasks() {
        let mut epic = Phase::new("test".to_string());

        // Create hierarchy: 1 -> 1.1, 1.2 -> 1.2.1
        let mut parent = Task::new("1".to_string(), "Parent".to_string(), String::new());
        parent.status = TaskStatus::Expanded;
        parent.subtasks = vec!["1.1".to_string(), "1.2".to_string()];

        let mut child1 = Task::new("1.1".to_string(), "Child 1".to_string(), String::new());
        child1.parent_id = Some("1".to_string());

        let mut child2 = Task::new("1.2".to_string(), "Child 2".to_string(), String::new());
        child2.parent_id = Some("1".to_string());
        child2.status = TaskStatus::Expanded;
        child2.subtasks = vec!["1.2.1".to_string()];

        let mut grandchild =
            Task::new("1.2.1".to_string(), "Grandchild".to_string(), String::new());
        grandchild.parent_id = Some("1.2".to_string());

        epic.add_task(parent);
        epic.add_task(child1);
        epic.add_task(child2);
        epic.add_task(grandchild);

        let scg = serialize_scg(&epic);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.tasks.len(), 4);

        let gc = parsed.get_task("1.2.1").unwrap();
        assert_eq!(gc.parent_id, Some("1.2".to_string()));

        let c2 = parsed.get_task("1.2").unwrap();
        assert!(c2.subtasks.contains(&"1.2.1".to_string()));
    }

    #[test]
    fn test_section_comment_lines_ignored() {
        // Manually create SCG with extra comment lines - they should be ignored
        let content = r#"# SCUD Graph v1
# Epic: test

@meta {
  name test
  # this is a comment
  updated 2025-01-01T00:00:00Z
}

@nodes
# id | title | status | complexity | priority
# another comment
1 | Task | P | 0 | M
"#;
        let epic = parse_scg(content).unwrap();
        assert_eq!(epic.tasks.len(), 1);
        assert_eq!(epic.get_task("1").unwrap().title, "Task");
    }
}
