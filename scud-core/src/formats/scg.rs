//! SCUD Graph (.scg) format parser and serializer
//!
//! A token-efficient, graph-native format for task storage.

use anyhow::{Context, Result};
use std::collections::HashMap;

use crate::models::{IdFormat, Phase, Priority, Task, TaskStatus};
use crate::weave::bthread::{
    BThread, BThreadRule, NodeAnnotation, PartitionDef, PartitionStrategy, Role, TimeoutAction,
    WeaveEdge, WeaveEdgeType,
};
use crate::weave::coordinator::ActiveLock;
use crate::weave::event::{EventKind, EventPattern};
use crate::weave::matcher::GlobPattern;

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
        TaskStatus::Failed => '!',
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
        '!' => Some(TaskStatus::Failed),
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
    let mut agent_types: HashMap<String, String> = HashMap::new();

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
                "@agents" => "agents",
                "@details" => "details",
                "@weave" => "weave",
                "@roles" => "roles",
                "@partitions" => "partitions",
                "@locks" => "locks",
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
                // Parse "id | title | status | complexity | priority [| key=val key=val]"
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

                    // Extended node annotations: trailing key=value pairs after priority
                    if parts.len() > 5 {
                        let annotation_str = &parts[5];
                        if !annotation_str.is_empty() {
                            let mut annotation = NodeAnnotation::default();
                            for kv in annotation_str.split_whitespace() {
                                if let Some((k, v)) = kv.split_once('=') {
                                    match k {
                                        "role" => annotation.role = Some(v.to_string()),
                                        "scope" => annotation.scope = Some(v.to_string()),
                                        _ => {}
                                    }
                                }
                            }
                            if annotation.role.is_some() || annotation.scope.is_some() {
                                phase.node_annotations.insert(id.clone(), annotation);
                            }
                        }
                    }

                    tasks.insert(id, task);
                }
            }
            Some("edges") => {
                // Parse dependency edges "dependent -> dependency"
                // and behavioral edges: ~~ (conflict), >> (sequence), != (exclusion)
                if let Some((dependent, dependency)) = trimmed.split_once("->") {
                    edges.push((dependent.trim().to_string(), dependency.trim().to_string()));
                } else if let Some(edge) = parse_behavioral_edge(trimmed) {
                    phase.weave_edges.push(edge);
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
            Some("agents") => {
                // Parse "id | agent_type"
                let parts = split_by_pipe(trimmed);
                if parts.len() >= 2 && !parts[1].is_empty() {
                    agent_types.insert(parts[0].clone(), parts[1].clone());
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
            Some("weave") => {
                // Parse "id | name | priority | enabled | rule_type | rule_spec"
                let parts = split_by_pipe(trimmed);
                if parts.len() >= 6 {
                    let id = parts[0].clone();
                    let name = parts[1].clone();
                    let priority: u32 = parts[2].parse().unwrap_or(0);
                    let enabled = parts[3].trim() == "Y";
                    let rule_type = parts[4].trim();
                    let rule_spec = &parts[5];

                    if let Some(rule) = parse_bthread_rule(rule_type, rule_spec) {
                        let bthread = BThread {
                            id,
                            name,
                            priority,
                            enabled,
                            rules: vec![rule],
                        };
                        phase.weave_threads.push(bthread);
                    }
                }
            }
            Some("roles") => {
                // Parse "role_id | name | allow_pattern | deny_pattern"
                let parts = split_by_pipe(trimmed);
                if parts.len() >= 4 {
                    let id = parts[0].clone();
                    let name = parts[1].clone();
                    let allow_patterns = parse_glob_patterns(&parts[2]);
                    let deny_patterns = parse_glob_patterns(&parts[3]);
                    let role = Role {
                        id,
                        name,
                        allow_patterns,
                        deny_patterns,
                    };
                    phase.roles.push(role);
                }
            }
            Some("partitions") => {
                // Parse "partition_id | scope_pattern | strategy | agent_count"
                let parts = split_by_pipe(trimmed);
                if parts.len() >= 4 {
                    let id = parts[0].clone();
                    let scope_pattern = GlobPattern::new(parts[1].trim());
                    let strategy = match parts[2].trim() {
                        "hash" => PartitionStrategy::Hash,
                        "round-robin" => PartitionStrategy::RoundRobin,
                        "directory" => PartitionStrategy::Directory,
                        _ => PartitionStrategy::Hash,
                    };
                    let agent_count: u32 = parts[3].parse().unwrap_or(1);
                    let partition = PartitionDef {
                        id,
                        scope_pattern,
                        strategy,
                        agent_count,
                    };
                    phase.partitions.push(partition);
                }
            }
            Some("locks") => {
                // Parse "lock_key | holder_agent | task_id | acquired_at | ttl_secs"
                let parts = split_by_pipe(trimmed);
                if parts.len() >= 5 {
                    let lock_key = parts[0].clone();
                    let holder_agent = parts[1].clone();
                    let task_id_str = parts[2].trim();
                    let task_id = if task_id_str.is_empty() || task_id_str == "-" {
                        None
                    } else {
                        Some(task_id_str.to_string())
                    };
                    let acquired_at = parts[3].clone();
                    let ttl_secs: u64 = parts[4].parse().unwrap_or(3600);
                    let lock = ActiveLock {
                        lock_key,
                        holder_agent,
                        task_id,
                        acquired_at,
                        ttl_secs,
                    };
                    phase.locks.push(lock);
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

    // Apply agent types
    for (id, agent_type) in agent_types {
        if let Some(task) = tasks.get_mut(&id) {
            task.agent_type = Some(agent_type);
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

/// Parse a behavioral edge line (~~, >>, !=) with optional `| reason=...` suffix.
fn parse_behavioral_edge(line: &str) -> Option<WeaveEdge> {
    // Try each operator
    for op in &["~~", ">>", "!="] {
        if let Some(pos) = line.find(op) {
            let from = line[..pos].trim().to_string();
            let rest = line[pos + op.len()..].trim();

            // Check for optional `| reason=...` suffix
            let (to, reason) = if let Some(pipe_pos) = rest.find('|') {
                let to_part = rest[..pipe_pos].trim().to_string();
                let reason_part = rest[pipe_pos + 1..].trim();
                let reason = reason_part
                    .strip_prefix("reason=")
                    .map(|r| r.trim().to_string());
                (to_part, reason)
            } else {
                (rest.to_string(), None)
            };

            let edge_type = WeaveEdgeType::parse(op)?;
            return Some(WeaveEdge {
                from,
                to,
                edge_type,
                reason,
            });
        }
    }
    None
}

/// Parse a BThreadRule from rule_type string and space-separated key=value rule_spec.
fn parse_bthread_rule(rule_type: &str, rule_spec: &str) -> Option<BThreadRule> {
    let kv: HashMap<&str, &str> = rule_spec
        .split_whitespace()
        .filter_map(|pair| pair.split_once('='))
        .collect();

    match rule_type.trim() {
        "Mutex" => {
            let kind_str = kv.get("kind").unwrap_or(&"FileWrite");
            let key = kv.get("key").unwrap_or(&"").to_string();
            let ttl_secs = kv.get("ttl_secs").and_then(|v| v.parse().ok());
            Some(BThreadRule::Mutex {
                scope: EventPattern::kind(EventKind::parse(kind_str)),
                key,
                ttl_secs,
            })
        }
        "Require" => {
            let trigger = kv
                .get("trigger")
                .map(|k| EventPattern::kind(EventKind::parse(k)))?;
            let prereq = kv
                .get("prereq")
                .map(|k| EventPattern::kind(EventKind::parse(k)))?;
            let reset = kv
                .get("reset")
                .map(|k| EventPattern::kind(EventKind::parse(k)));
            Some(BThreadRule::Require {
                trigger,
                prerequisite: prereq,
                reset,
            })
        }
        "BlockUntil" => {
            let trigger = kv
                .get("trigger")
                .map(|k| EventPattern::kind(EventKind::parse(k)))?;
            let block_str = kv.get("block")?;
            let block = vec![EventPattern::kind(EventKind::parse(block_str))];
            let until = kv
                .get("until")
                .map(|k| EventPattern::kind(EventKind::parse(k)))?;
            let escalate = kv.get("escalate").map(|v| *v == "true").unwrap_or(false);
            Some(BThreadRule::BlockUntil {
                trigger,
                block,
                until,
                escalate,
                escalation_message: None,
            })
        }
        "BlockAlways" => {
            let kind_str = kv.get("kind").unwrap_or(&"DangerousCommand");
            Some(BThreadRule::BlockAlways {
                scope: EventPattern::kind(EventKind::parse(kind_str)),
            })
        }
        "RateLimit" => {
            let kind_str = kv.get("kind").unwrap_or(&"Commit");
            let max: u32 = kv.get("max").and_then(|v| v.parse().ok()).unwrap_or(1);
            let window_secs: u64 = kv.get("window").and_then(|v| v.parse().ok()).unwrap_or(60);
            Some(BThreadRule::RateLimit {
                scope: EventPattern::kind(EventKind::parse(kind_str)),
                max,
                window_secs,
            })
        }
        "Timeout" => {
            let kind_str = kv.get("kind").unwrap_or(&"TestRun");
            let max_secs: u64 = kv
                .get("max_secs")
                .and_then(|v| v.parse().ok())
                .unwrap_or(300);
            let action = match kv.get("action") {
                Some(&"warn") | Some(&"Warn") => TimeoutAction::Warn,
                _ => TimeoutAction::Kill,
            };
            Some(BThreadRule::Timeout {
                scope: EventPattern::kind(EventKind::parse(kind_str)),
                max_duration_secs: max_secs,
                action,
            })
        }
        "Partition" => {
            let kind_str = kv.get("kind").unwrap_or(&"FileWrite");
            let strategy = match kv.get("strategy") {
                Some(&"round-robin") => PartitionStrategy::RoundRobin,
                Some(&"directory") => PartitionStrategy::Directory,
                _ => PartitionStrategy::Hash,
            };
            let agent_count: u32 = kv.get("count").and_then(|v| v.parse().ok()).unwrap_or(1);
            Some(BThreadRule::Partition {
                scope: EventPattern::kind(EventKind::parse(kind_str)),
                strategy,
                agent_count,
            })
        }
        _ => None,
    }
}

/// Parse space-separated glob patterns. "-" means empty list.
fn parse_glob_patterns(s: &str) -> Vec<GlobPattern> {
    let trimmed = s.trim();
    if trimmed == "-" || trimmed.is_empty() {
        return Vec::new();
    }
    trimmed
        .split_whitespace()
        .map(GlobPattern::new)
        .collect()
}

/// Validate weave-related sections of a Phase.
///
/// Returns a list of validation errors found. An empty list means valid.
#[allow(dead_code)]
pub fn validate_weave(phase: &Phase) -> Vec<String> {
    let mut errors = Vec::new();

    // Validate @weave IDs match w:\d+
    for bt in &phase.weave_threads {
        if !is_valid_weave_id(&bt.id) {
            errors.push(format!(
                "@weave: ID '{}' must match pattern 'w:<number>'",
                bt.id
            ));
        }
    }

    // Validate @roles IDs match r:\w+
    for role in &phase.roles {
        if !is_valid_role_id(&role.id) {
            errors.push(format!(
                "@roles: ID '{}' must match pattern 'r:<word>'",
                role.id
            ));
        }
    }

    // Validate @partitions agent_count > 0
    for p in &phase.partitions {
        if p.agent_count == 0 {
            errors.push(format!(
                "@partitions: '{}' agent_count must be > 0",
                p.id
            ));
        }
    }

    // Validate @locks TTL > 0
    for lock in &phase.locks {
        if lock.ttl_secs == 0 {
            errors.push(format!(
                "@locks: '{}' ttl_secs must be > 0",
                lock.lock_key
            ));
        }
    }

    errors
}

/// Check if a weave ID matches `w:\d+` pattern.
fn is_valid_weave_id(id: &str) -> bool {
    if let Some(rest) = id.strip_prefix("w:") {
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    }
}

/// Check if a role ID matches `r:\w+` pattern.
fn is_valid_role_id(id: &str) -> bool {
    if let Some(rest) = id.strip_prefix("r:") {
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    } else {
        false
    }
}

/// Serialize a BThreadRule to (rule_type, rule_spec) strings for SCG format.
fn serialize_bthread_rule(rule: &BThreadRule) -> (&'static str, String) {
    match rule {
        BThreadRule::Mutex {
            scope,
            key,
            ttl_secs,
        } => {
            let kind = scope
                .kind
                .as_ref()
                .map(|k| k.as_str())
                .unwrap_or("FileWrite");
            let mut spec = format!("kind={} key={}", kind, key);
            if let Some(ttl) = ttl_secs {
                spec.push_str(&format!(" ttl_secs={}", ttl));
            }
            ("Mutex", spec)
        }
        BThreadRule::Require {
            trigger,
            prerequisite,
            reset,
        } => {
            let t = trigger
                .kind
                .as_ref()
                .map(|k| k.as_str())
                .unwrap_or("Commit");
            let p = prerequisite
                .kind
                .as_ref()
                .map(|k| k.as_str())
                .unwrap_or("TestPass");
            let mut spec = format!("trigger={} prereq={}", t, p);
            if let Some(ref r) = reset {
                let r_str = r.kind.as_ref().map(|k| k.as_str()).unwrap_or("FileWrite");
                spec.push_str(&format!(" reset={}", r_str));
            }
            ("Require", spec)
        }
        BThreadRule::BlockUntil {
            trigger,
            block,
            until,
            escalate,
            ..
        } => {
            let t = trigger
                .kind
                .as_ref()
                .map(|k| k.as_str())
                .unwrap_or("ApiChange");
            let b = block
                .first()
                .and_then(|bp| bp.kind.as_ref())
                .map(|k| k.as_str())
                .unwrap_or("Build");
            let u = until
                .kind
                .as_ref()
                .map(|k| k.as_str())
                .unwrap_or("ApiReviewApproved");
            let mut spec = format!("trigger={} block={} until={}", t, b, u);
            if *escalate {
                spec.push_str(" escalate=true");
            }
            ("BlockUntil", spec)
        }
        BThreadRule::BlockAlways { scope } => {
            let kind = scope
                .kind
                .as_ref()
                .map(|k| k.as_str())
                .unwrap_or("DangerousCommand");
            ("BlockAlways", format!("kind={}", kind))
        }
        BThreadRule::RateLimit {
            scope,
            max,
            window_secs,
        } => {
            let kind = scope
                .kind
                .as_ref()
                .map(|k| k.as_str())
                .unwrap_or("Commit");
            (
                "RateLimit",
                format!("kind={} max={} window={}", kind, max, window_secs),
            )
        }
        BThreadRule::Timeout {
            scope,
            max_duration_secs,
            action,
        } => {
            let kind = scope
                .kind
                .as_ref()
                .map(|k| k.as_str())
                .unwrap_or("TestRun");
            let action_str = match action {
                TimeoutAction::Kill => "kill",
                TimeoutAction::Warn => "warn",
            };
            (
                "Timeout",
                format!(
                    "kind={} max_secs={} action={}",
                    kind, max_duration_secs, action_str
                ),
            )
        }
        BThreadRule::Partition {
            scope,
            strategy,
            agent_count,
        } => {
            let kind = scope
                .kind
                .as_ref()
                .map(|k| k.as_str())
                .unwrap_or("FileWrite");
            let strat = match strategy {
                PartitionStrategy::Hash => "hash",
                PartitionStrategy::RoundRobin => "round-robin",
                PartitionStrategy::Directory => "directory",
            };
            (
                "Partition",
                format!("kind={} strategy={} count={}", kind, strat, agent_count),
            )
        }
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
        let base = format!(
            "{} | {} | {} | {} | {}",
            task.id,
            escape_text(&task.title),
            status_to_code(&task.status),
            task.complexity,
            priority_to_code(&task.priority)
        );
        // Append node annotations if present
        if let Some(ann) = phase.node_annotations.get(&task.id) {
            let mut kv_parts = Vec::new();
            if let Some(ref role) = ann.role {
                kv_parts.push(format!("role={}", role));
            }
            if let Some(ref scope) = ann.scope {
                kv_parts.push(format!("scope={}", scope));
            }
            if !kv_parts.is_empty() {
                output.push_str(&format!("{} | {}\n", base, kv_parts.join(" ")));
            } else {
                output.push_str(&format!("{}\n", base));
            }
        } else {
            output.push_str(&format!("{}\n", base));
        }
    }
    output.push('\n');

    // Edges section (dependencies + behavioral edges)
    let dep_edges: Vec<_> = sorted_tasks
        .iter()
        .flat_map(|t| t.dependencies.iter().map(move |dep| (&t.id, dep)))
        .collect();

    if !dep_edges.is_empty() || !phase.weave_edges.is_empty() {
        output.push_str("@edges\n");
        if !dep_edges.is_empty() {
            output.push_str("# dependent -> dependency\n");
            for (dependent, dependency) in dep_edges {
                output.push_str(&format!("{} -> {}\n", dependent, dependency));
            }
        }
        // Behavioral edges
        if !phase.weave_edges.is_empty() {
            output.push('\n');
            output.push_str("# Behavioral edges\n");
            for edge in &phase.weave_edges {
                let line = format!(
                    "{} {} {}",
                    edge.from,
                    edge.edge_type.operator(),
                    edge.to
                );
                if let Some(ref reason) = edge.reason {
                    output.push_str(&format!("{} | reason={}\n", line, reason));
                } else {
                    output.push_str(&format!("{}\n", line));
                }
            }
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

    // Agents section
    let agents: Vec<_> = sorted_tasks
        .iter()
        .filter(|t| t.agent_type.is_some())
        .collect();

    if !agents.is_empty() {
        output.push_str("@agents\n");
        output.push_str("# id | agent_type\n");
        for task in agents {
            output.push_str(&format!(
                "{} | {}\n",
                task.id,
                task.agent_type.as_deref().unwrap_or("")
            ));
        }
        output.push('\n');
    }

    // Roles section
    if !phase.roles.is_empty() {
        output.push_str("@roles\n");
        output.push_str("# role_id | name | allow_pattern | deny_pattern\n");
        for role in &phase.roles {
            let allow = if role.allow_patterns.is_empty() {
                "-".to_string()
            } else {
                role.allow_patterns
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let deny = if role.deny_patterns.is_empty() {
                "-".to_string()
            } else {
                role.deny_patterns
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            output.push_str(&format!("{} | {} | {} | {}\n", role.id, role.name, allow, deny));
        }
        output.push('\n');
    }

    // Weave section (b-threads)
    if !phase.weave_threads.is_empty() {
        output.push_str("@weave\n");
        output.push_str("# id | name | priority | enabled | rule_type | rule_spec\n");
        for bt in &phase.weave_threads {
            let enabled = if bt.enabled { "Y" } else { "N" };
            for rule in &bt.rules {
                let (rule_type, rule_spec) = serialize_bthread_rule(rule);
                output.push_str(&format!(
                    "{} | {} | {} | {} | {} | {}\n",
                    bt.id, bt.name, bt.priority, enabled, rule_type, rule_spec
                ));
            }
        }
        output.push('\n');
    }

    // Partitions section
    if !phase.partitions.is_empty() {
        output.push_str("@partitions\n");
        output.push_str("# partition_id | scope_pattern | strategy | agent_count\n");
        for p in &phase.partitions {
            let strategy = match p.strategy {
                PartitionStrategy::Hash => "hash",
                PartitionStrategy::RoundRobin => "round-robin",
                PartitionStrategy::Directory => "directory",
            };
            output.push_str(&format!(
                "{} | {} | {} | {}\n",
                p.id,
                p.scope_pattern.as_str(),
                strategy,
                p.agent_count
            ));
        }
        output.push('\n');
    }

    // Locks section
    if !phase.locks.is_empty() {
        output.push_str("@locks\n");
        output.push_str("# lock_key | holder_agent | task_id | acquired_at | ttl_secs\n");
        for lock in &phase.locks {
            let task_id = lock.task_id.as_deref().unwrap_or("-");
            output.push_str(&format!(
                "{} | {} | {} | {} | {}\n",
                lock.lock_key, lock.holder_agent, task_id, lock.acquired_at, lock.ttl_secs
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
        assert_eq!(status_to_code(&TaskStatus::Failed), '!');

        assert_eq!(code_to_status('P'), Some(TaskStatus::Pending));
        assert_eq!(code_to_status('X'), Some(TaskStatus::Expanded));
        assert_eq!(code_to_status('!'), Some(TaskStatus::Failed));
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
        let content =
            "# SCUD Graph v1\n# Phase: empty\n\n@nodes\n# id | title | status | complexity | priority\n";
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
    fn test_id_format_round_trip() {
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
    fn test_backward_compat_no_weave() {
        // SCG without weave sections should parse identically to before
        let content = "\
# SCUD Graph v1
# Phase: auth

@nodes
1 | Design auth | P | 5 | H
2 | Write tests | P | 3 | M

@edges
2 -> 1
";
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.name, "auth");
        assert_eq!(phase.tasks.len(), 2);
        assert!(phase.weave_threads.is_empty());
        assert!(phase.roles.is_empty());
        assert!(phase.partitions.is_empty());
        assert!(phase.locks.is_empty());
        assert!(phase.weave_edges.is_empty());
        assert!(phase.node_annotations.is_empty());
    }

    #[test]
    fn test_parse_weave_section() {
        let content = "\
# SCUD Graph v1
# Phase: test

@nodes
1 | Task one | P | 5 | H

@weave
w:1 | File mutex | 5 | Y | Mutex | kind=FileWrite key=file:{target}
w:2 | Test gate | 10 | Y | Require | trigger=Commit prereq=TestPass reset=FileWrite
w:3 | No kill | 1 | N | BlockAlways | kind=DangerousCommand
";
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.weave_threads.len(), 3);

        let bt1 = &phase.weave_threads[0];
        assert_eq!(bt1.id, "w:1");
        assert_eq!(bt1.name, "File mutex");
        assert_eq!(bt1.priority, 5);
        assert!(bt1.enabled);
        assert_eq!(bt1.rules.len(), 1);
        match &bt1.rules[0] {
            BThreadRule::Mutex { key, .. } => assert_eq!(key, "file:{target}"),
            _ => panic!("Expected Mutex rule"),
        }

        let bt2 = &phase.weave_threads[1];
        assert_eq!(bt2.id, "w:2");
        assert!(bt2.enabled);
        match &bt2.rules[0] {
            BThreadRule::Require {
                trigger,
                prerequisite,
                reset,
            } => {
                assert_eq!(trigger.kind, Some(EventKind::Commit));
                assert_eq!(prerequisite.kind, Some(EventKind::TestPass));
                assert!(reset.is_some());
            }
            _ => panic!("Expected Require rule"),
        }

        let bt3 = &phase.weave_threads[2];
        assert!(!bt3.enabled);
        match &bt3.rules[0] {
            BThreadRule::BlockAlways { scope } => {
                assert_eq!(scope.kind, Some(EventKind::DangerousCommand));
            }
            _ => panic!("Expected BlockAlways rule"),
        }
    }

    #[test]
    fn test_parse_roles_section() {
        let content = "\
# SCUD Graph v1
# Phase: test

@nodes
1 | Task one | P | 5 | H

@roles
r:impl | Implementer | src/** | docs/**
r:test | Test writer | tests/** src/test_*.rs | src/main.rs src/lib.rs
r:review | Reviewer | - | -
";
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.roles.len(), 3);

        let r1 = &phase.roles[0];
        assert_eq!(r1.id, "r:impl");
        assert_eq!(r1.name, "Implementer");
        assert_eq!(r1.allow_patterns.len(), 1);
        assert_eq!(r1.allow_patterns[0].as_str(), "src/**");
        assert_eq!(r1.deny_patterns.len(), 1);
        assert_eq!(r1.deny_patterns[0].as_str(), "docs/**");

        let r2 = &phase.roles[1];
        assert_eq!(r2.allow_patterns.len(), 2);
        assert_eq!(r2.deny_patterns.len(), 2);

        let r3 = &phase.roles[2];
        assert!(r3.allow_patterns.is_empty()); // "-" means empty
        assert!(r3.deny_patterns.is_empty());
    }

    #[test]
    fn test_parse_partitions_section() {
        let content = "\
# SCUD Graph v1
# Phase: test

@nodes
1 | Task one | P | 5 | H

@partitions
p:1 | src/codegen/** | hash | 4
p:2 | src/parser/** | round-robin | 3
p:3 | tests/** | directory | 4
";
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.partitions.len(), 3);

        assert_eq!(phase.partitions[0].id, "p:1");
        assert_eq!(phase.partitions[0].scope_pattern.as_str(), "src/codegen/**");
        assert_eq!(phase.partitions[0].strategy, PartitionStrategy::Hash);
        assert_eq!(phase.partitions[0].agent_count, 4);

        assert_eq!(phase.partitions[1].strategy, PartitionStrategy::RoundRobin);
        assert_eq!(phase.partitions[2].strategy, PartitionStrategy::Directory);
    }

    #[test]
    fn test_parse_locks_section() {
        let content = "\
# SCUD Graph v1
# Phase: test

@nodes
1 | Task one | P | 5 | H

@locks
file:src/auth/jwt.rs | agent-1 | auth:1.1 | 2026-02-06T10:05:00Z | 3600
schema-global | agent-1 | - | 2026-02-06T10:05:00Z | 7200
";
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.locks.len(), 2);

        let l1 = &phase.locks[0];
        assert_eq!(l1.lock_key, "file:src/auth/jwt.rs");
        assert_eq!(l1.holder_agent, "agent-1");
        assert_eq!(l1.task_id, Some("auth:1.1".to_string()));
        assert_eq!(l1.ttl_secs, 3600);

        let l2 = &phase.locks[1];
        assert_eq!(l2.lock_key, "schema-global");
        assert!(l2.task_id.is_none());
        assert_eq!(l2.ttl_secs, 7200);
    }

    #[test]
    fn test_parse_extended_nodes() {
        let content = "\
# SCUD Graph v1
# Phase: test

@nodes
1 | Task one | I | 5 | H | role=r:impl scope=src/auth/**
2 | Task two | P | 3 | M
";
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.tasks.len(), 2);

        // Node 1 should have annotations
        let ann = phase.node_annotations.get("1").expect("annotation for node 1");
        assert_eq!(ann.role, Some("r:impl".to_string()));
        assert_eq!(ann.scope, Some("src/auth/**".to_string()));

        // Node 2 should not
        assert!(!phase.node_annotations.contains_key("2"));
    }

    #[test]
    fn test_parse_behavioral_edges() {
        let content = "\
# SCUD Graph v1
# Phase: test

@nodes
1 | Task one | P | 5 | H
2 | Task two | P | 3 | M
3 | Task three | P | 3 | M

@edges
2 -> 1
1 ~~ 2 | reason=shared-config
1 >> 3 | reason=tests-need-impl
2 != 3
";
        let phase = parse_scg(content).unwrap();

        // Regular dependency edges
        let t2 = phase.get_task("2").unwrap();
        assert_eq!(t2.dependencies, vec!["1".to_string()]);

        // Behavioral edges
        assert_eq!(phase.weave_edges.len(), 3);

        let e1 = &phase.weave_edges[0];
        assert_eq!(e1.from, "1");
        assert_eq!(e1.to, "2");
        assert_eq!(e1.edge_type, WeaveEdgeType::Conflict);
        assert_eq!(e1.reason, Some("shared-config".to_string()));

        let e2 = &phase.weave_edges[1];
        assert_eq!(e2.edge_type, WeaveEdgeType::Sequence);
        assert_eq!(e2.reason, Some("tests-need-impl".to_string()));

        let e3 = &phase.weave_edges[2];
        assert_eq!(e3.edge_type, WeaveEdgeType::Exclusion);
        assert!(e3.reason.is_none());
    }

    #[test]
    fn test_weave_round_trip() {
        let mut phase = Phase::new("weave-test".to_string());

        let task = Task::new("1".to_string(), "Test task".to_string(), String::new());
        phase.add_task(task);

        // Add weave threads
        phase.weave_threads.push(BThread {
            id: "w:1".to_string(),
            name: "File mutex".to_string(),
            priority: 5,
            enabled: true,
            rules: vec![BThreadRule::Mutex {
                scope: EventPattern::kind(EventKind::FileWrite),
                key: "file:{target}".to_string(),
                ttl_secs: None,
            }],
        });
        phase.weave_threads.push(BThread {
            id: "w:2".to_string(),
            name: "Test gate".to_string(),
            priority: 10,
            enabled: false,
            rules: vec![BThreadRule::Require {
                trigger: EventPattern::kind(EventKind::Commit),
                prerequisite: EventPattern::kind(EventKind::TestPass),
                reset: Some(EventPattern::kind(EventKind::FileWrite)),
            }],
        });

        // Add roles
        phase.roles.push(Role {
            id: "r:impl".to_string(),
            name: "Implementer".to_string(),
            allow_patterns: vec![GlobPattern::new("src/**")],
            deny_patterns: vec![GlobPattern::new("docs/**")],
        });

        // Add partitions
        phase.partitions.push(PartitionDef {
            id: "p:1".to_string(),
            scope_pattern: GlobPattern::new("src/**"),
            strategy: PartitionStrategy::Hash,
            agent_count: 4,
        });

        // Add locks
        phase.locks.push(ActiveLock {
            lock_key: "file:src/main.rs".to_string(),
            holder_agent: "agent-1".to_string(),
            task_id: Some("1".to_string()),
            acquired_at: "2026-02-06T10:00:00Z".to_string(),
            ttl_secs: 3600,
        });

        // Add behavioral edges
        phase.weave_edges.push(WeaveEdge {
            from: "1".to_string(),
            to: "2".to_string(),
            edge_type: WeaveEdgeType::Conflict,
            reason: Some("shared-code".to_string()),
        });

        // Add node annotations
        phase.node_annotations.insert(
            "1".to_string(),
            NodeAnnotation {
                role: Some("r:impl".to_string()),
                scope: Some("src/auth/**".to_string()),
            },
        );

        // Serialize and re-parse
        let scg = serialize_scg(&phase);
        let parsed = parse_scg(&scg).unwrap();

        // Verify weave threads
        assert_eq!(parsed.weave_threads.len(), 2);
        assert_eq!(parsed.weave_threads[0].id, "w:1");
        assert!(parsed.weave_threads[0].enabled);
        assert_eq!(parsed.weave_threads[1].id, "w:2");
        assert!(!parsed.weave_threads[1].enabled);

        // Verify roles
        assert_eq!(parsed.roles.len(), 1);
        assert_eq!(parsed.roles[0].id, "r:impl");
        assert_eq!(parsed.roles[0].allow_patterns[0].as_str(), "src/**");

        // Verify partitions
        assert_eq!(parsed.partitions.len(), 1);
        assert_eq!(parsed.partitions[0].strategy, PartitionStrategy::Hash);
        assert_eq!(parsed.partitions[0].agent_count, 4);

        // Verify locks
        assert_eq!(parsed.locks.len(), 1);
        assert_eq!(parsed.locks[0].lock_key, "file:src/main.rs");
        assert_eq!(parsed.locks[0].ttl_secs, 3600);

        // Verify behavioral edges
        assert_eq!(parsed.weave_edges.len(), 1);
        assert_eq!(parsed.weave_edges[0].edge_type, WeaveEdgeType::Conflict);
        assert_eq!(parsed.weave_edges[0].reason, Some("shared-code".to_string()));

        // Verify node annotations
        let ann = parsed.node_annotations.get("1").unwrap();
        assert_eq!(ann.role, Some("r:impl".to_string()));
        assert_eq!(ann.scope, Some("src/auth/**".to_string()));
    }

    #[test]
    fn test_validate_weave_valid() {
        let mut phase = Phase::new("test".to_string());
        phase.weave_threads.push(BThread {
            id: "w:1".to_string(),
            name: "Test".to_string(),
            priority: 1,
            enabled: true,
            rules: vec![],
        });
        phase.roles.push(Role {
            id: "r:impl".to_string(),
            name: "Impl".to_string(),
            allow_patterns: vec![],
            deny_patterns: vec![],
        });
        phase.partitions.push(PartitionDef {
            id: "p:1".to_string(),
            scope_pattern: GlobPattern::new("src/**"),
            strategy: PartitionStrategy::Hash,
            agent_count: 2,
        });
        phase.locks.push(ActiveLock {
            lock_key: "test-key".to_string(),
            holder_agent: "agent-1".to_string(),
            task_id: None,
            acquired_at: "2026-02-06T10:00:00Z".to_string(),
            ttl_secs: 3600,
        });

        let errors = validate_weave(&phase);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_validate_weave_invalid() {
        let mut phase = Phase::new("test".to_string());

        // Invalid weave ID (missing w: prefix)
        phase.weave_threads.push(BThread {
            id: "bad-id".to_string(),
            name: "Test".to_string(),
            priority: 1,
            enabled: true,
            rules: vec![],
        });

        // Invalid role ID
        phase.roles.push(Role {
            id: "not-role".to_string(),
            name: "Bad".to_string(),
            allow_patterns: vec![],
            deny_patterns: vec![],
        });

        // Invalid partition (agent_count = 0)
        phase.partitions.push(PartitionDef {
            id: "p:1".to_string(),
            scope_pattern: GlobPattern::new("src/**"),
            strategy: PartitionStrategy::Hash,
            agent_count: 0,
        });

        // Invalid lock (ttl = 0)
        phase.locks.push(ActiveLock {
            lock_key: "test-key".to_string(),
            holder_agent: "agent-1".to_string(),
            task_id: None,
            acquired_at: "2026-02-06T10:00:00Z".to_string(),
            ttl_secs: 0,
        });

        let errors = validate_weave(&phase);
        assert_eq!(errors.len(), 4);
    }

    #[test]
    fn test_parse_weave_ratelimit() {
        let content = "\
# SCUD Graph v1
# Phase: test

@nodes
1 | Task | P | 5 | H

@weave
w:1 | Commit rate | 50 | Y | RateLimit | kind=Commit max=5 window=120
";
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.weave_threads.len(), 1);
        match &phase.weave_threads[0].rules[0] {
            BThreadRule::RateLimit {
                max,
                window_secs,
                scope,
            } => {
                assert_eq!(*max, 5);
                assert_eq!(*window_secs, 120);
                assert_eq!(scope.kind, Some(EventKind::Commit));
            }
            _ => panic!("Expected RateLimit rule"),
        }
    }

    #[test]
    fn test_parse_weave_timeout() {
        let content = "\
# SCUD Graph v1
# Phase: test

@nodes
1 | Task | P | 5 | H

@weave
w:1 | Time budget | 30 | Y | Timeout | kind=TestRun max_secs=300 action=warn
";
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.weave_threads.len(), 1);
        match &phase.weave_threads[0].rules[0] {
            BThreadRule::Timeout {
                max_duration_secs,
                action,
                ..
            } => {
                assert_eq!(*max_duration_secs, 300);
                assert_eq!(*action, TimeoutAction::Warn);
            }
            _ => panic!("Expected Timeout rule"),
        }
    }

    #[test]
    fn test_weave_all_rule_types_round_trip() {
        // Test that every rule type survives serialize -> parse
        let mut phase = Phase::new("all-rules".to_string());
        let task = Task::new("1".to_string(), "Task".to_string(), String::new());
        phase.add_task(task);

        phase.weave_threads.push(BThread {
            id: "w:1".to_string(),
            name: "Mutex rule".to_string(),
            priority: 5,
            enabled: true,
            rules: vec![BThreadRule::Mutex {
                scope: EventPattern::kind(EventKind::FileWrite),
                key: "file:{target}".to_string(),
                ttl_secs: Some(600),
            }],
        });
        phase.weave_threads.push(BThread {
            id: "w:2".to_string(),
            name: "Require rule".to_string(),
            priority: 10,
            enabled: true,
            rules: vec![BThreadRule::Require {
                trigger: EventPattern::kind(EventKind::Commit),
                prerequisite: EventPattern::kind(EventKind::TestPass),
                reset: Some(EventPattern::kind(EventKind::FileWrite)),
            }],
        });
        phase.weave_threads.push(BThread {
            id: "w:3".to_string(),
            name: "BlockAlways rule".to_string(),
            priority: 1,
            enabled: true,
            rules: vec![BThreadRule::BlockAlways {
                scope: EventPattern::kind(EventKind::DangerousCommand),
            }],
        });
        phase.weave_threads.push(BThread {
            id: "w:4".to_string(),
            name: "BlockUntil rule".to_string(),
            priority: 15,
            enabled: true,
            rules: vec![BThreadRule::BlockUntil {
                trigger: EventPattern::kind(EventKind::ApiChange),
                block: vec![EventPattern::kind(EventKind::Build)],
                until: EventPattern::kind(EventKind::Custom("ApiReviewApproved".to_string())),
                escalate: true,
                escalation_message: None,
            }],
        });
        phase.weave_threads.push(BThread {
            id: "w:5".to_string(),
            name: "RateLimit rule".to_string(),
            priority: 50,
            enabled: true,
            rules: vec![BThreadRule::RateLimit {
                scope: EventPattern::kind(EventKind::Commit),
                max: 5,
                window_secs: 120,
            }],
        });
        phase.weave_threads.push(BThread {
            id: "w:6".to_string(),
            name: "Timeout rule".to_string(),
            priority: 30,
            enabled: false,
            rules: vec![BThreadRule::Timeout {
                scope: EventPattern::kind(EventKind::TestRun),
                max_duration_secs: 300,
                action: TimeoutAction::Warn,
            }],
        });
        phase.weave_threads.push(BThread {
            id: "w:7".to_string(),
            name: "Partition rule".to_string(),
            priority: 20,
            enabled: true,
            rules: vec![BThreadRule::Partition {
                scope: EventPattern::kind(EventKind::FileWrite),
                strategy: PartitionStrategy::Directory,
                agent_count: 4,
            }],
        });

        let scg = serialize_scg(&phase);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.weave_threads.len(), 7);

        // Verify Mutex
        match &parsed.weave_threads[0].rules[0] {
            BThreadRule::Mutex { key, ttl_secs, scope } => {
                assert_eq!(key, "file:{target}");
                assert_eq!(*ttl_secs, Some(600));
                assert_eq!(scope.kind, Some(EventKind::FileWrite));
            }
            _ => panic!("Expected Mutex"),
        }

        // Verify Require
        match &parsed.weave_threads[1].rules[0] {
            BThreadRule::Require { trigger, prerequisite, reset } => {
                assert_eq!(trigger.kind, Some(EventKind::Commit));
                assert_eq!(prerequisite.kind, Some(EventKind::TestPass));
                assert!(reset.is_some());
            }
            _ => panic!("Expected Require"),
        }

        // Verify BlockAlways
        match &parsed.weave_threads[2].rules[0] {
            BThreadRule::BlockAlways { scope } => {
                assert_eq!(scope.kind, Some(EventKind::DangerousCommand));
            }
            _ => panic!("Expected BlockAlways"),
        }

        // Verify BlockUntil
        match &parsed.weave_threads[3].rules[0] {
            BThreadRule::BlockUntil { trigger, block, until, escalate, .. } => {
                assert_eq!(trigger.kind, Some(EventKind::ApiChange));
                assert_eq!(block[0].kind, Some(EventKind::Build));
                assert_eq!(until.kind, Some(EventKind::Custom("ApiReviewApproved".to_string())));
                assert!(*escalate);
            }
            _ => panic!("Expected BlockUntil"),
        }

        // Verify RateLimit
        match &parsed.weave_threads[4].rules[0] {
            BThreadRule::RateLimit { max, window_secs, .. } => {
                assert_eq!(*max, 5);
                assert_eq!(*window_secs, 120);
            }
            _ => panic!("Expected RateLimit"),
        }

        // Verify Timeout (disabled)
        assert!(!parsed.weave_threads[5].enabled);
        match &parsed.weave_threads[5].rules[0] {
            BThreadRule::Timeout { max_duration_secs, action, .. } => {
                assert_eq!(*max_duration_secs, 300);
                assert_eq!(*action, TimeoutAction::Warn);
            }
            _ => panic!("Expected Timeout"),
        }

        // Verify Partition
        match &parsed.weave_threads[6].rules[0] {
            BThreadRule::Partition { strategy, agent_count, .. } => {
                assert_eq!(*strategy, PartitionStrategy::Directory);
                assert_eq!(*agent_count, 4);
            }
            _ => panic!("Expected Partition"),
        }
    }

    #[test]
    fn test_roles_round_trip() {
        let mut phase = Phase::new("roles-rt".to_string());
        let task = Task::new("1".to_string(), "T".to_string(), String::new());
        phase.add_task(task);

        phase.roles.push(Role {
            id: "r:impl".to_string(),
            name: "Implementer".to_string(),
            allow_patterns: vec![GlobPattern::new("src/**"), GlobPattern::new("lib/**")],
            deny_patterns: vec![GlobPattern::new("docs/**")],
        });
        phase.roles.push(Role {
            id: "r:review".to_string(),
            name: "Reviewer".to_string(),
            allow_patterns: vec![],
            deny_patterns: vec![],
        });

        let scg = serialize_scg(&phase);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.roles.len(), 2);
        assert_eq!(parsed.roles[0].allow_patterns.len(), 2);
        assert_eq!(parsed.roles[0].deny_patterns.len(), 1);
        assert_eq!(parsed.roles[0].allow_patterns[0].as_str(), "src/**");
        assert_eq!(parsed.roles[0].allow_patterns[1].as_str(), "lib/**");

        // Empty role (-) should parse back to empty
        assert!(parsed.roles[1].allow_patterns.is_empty());
        assert!(parsed.roles[1].deny_patterns.is_empty());
    }

    #[test]
    fn test_partitions_round_trip() {
        let mut phase = Phase::new("parts-rt".to_string());
        let task = Task::new("1".to_string(), "T".to_string(), String::new());
        phase.add_task(task);

        phase.partitions.push(PartitionDef {
            id: "p:1".to_string(),
            scope_pattern: GlobPattern::new("src/**"),
            strategy: PartitionStrategy::Hash,
            agent_count: 4,
        });
        phase.partitions.push(PartitionDef {
            id: "p:2".to_string(),
            scope_pattern: GlobPattern::new("tests/**"),
            strategy: PartitionStrategy::RoundRobin,
            agent_count: 3,
        });
        phase.partitions.push(PartitionDef {
            id: "p:3".to_string(),
            scope_pattern: GlobPattern::new("docs/**"),
            strategy: PartitionStrategy::Directory,
            agent_count: 2,
        });

        let scg = serialize_scg(&phase);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.partitions.len(), 3);
        assert_eq!(parsed.partitions[0].strategy, PartitionStrategy::Hash);
        assert_eq!(parsed.partitions[0].agent_count, 4);
        assert_eq!(parsed.partitions[1].strategy, PartitionStrategy::RoundRobin);
        assert_eq!(parsed.partitions[2].strategy, PartitionStrategy::Directory);
        assert_eq!(parsed.partitions[2].scope_pattern.as_str(), "docs/**");
    }

    #[test]
    fn test_locks_round_trip() {
        let mut phase = Phase::new("locks-rt".to_string());
        let task = Task::new("1".to_string(), "T".to_string(), String::new());
        phase.add_task(task);

        phase.locks.push(ActiveLock {
            lock_key: "file:src/main.rs".to_string(),
            holder_agent: "agent-1".to_string(),
            task_id: Some("auth:1.1".to_string()),
            acquired_at: "2026-02-06T10:00:00Z".to_string(),
            ttl_secs: 3600,
        });
        phase.locks.push(ActiveLock {
            lock_key: "schema-global".to_string(),
            holder_agent: "agent-2".to_string(),
            task_id: None,
            acquired_at: "2026-02-06T10:05:00Z".to_string(),
            ttl_secs: 7200,
        });

        let scg = serialize_scg(&phase);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.locks.len(), 2);
        assert_eq!(parsed.locks[0].lock_key, "file:src/main.rs");
        assert_eq!(parsed.locks[0].task_id, Some("auth:1.1".to_string()));
        assert_eq!(parsed.locks[0].ttl_secs, 3600);
        assert_eq!(parsed.locks[1].lock_key, "schema-global");
        assert!(parsed.locks[1].task_id.is_none());
        assert_eq!(parsed.locks[1].ttl_secs, 7200);
    }

    #[test]
    fn test_behavioral_edges_round_trip() {
        let mut phase = Phase::new("edges-rt".to_string());
        let task1 = Task::new("1".to_string(), "T1".to_string(), String::new());
        let task2 = Task::new("2".to_string(), "T2".to_string(), String::new());
        let task3 = Task::new("3".to_string(), "T3".to_string(), String::new());
        phase.add_task(task1);
        phase.add_task(task2);
        phase.add_task(task3);

        phase.weave_edges.push(WeaveEdge {
            from: "1".to_string(),
            to: "2".to_string(),
            edge_type: WeaveEdgeType::Conflict,
            reason: Some("shared-config".to_string()),
        });
        phase.weave_edges.push(WeaveEdge {
            from: "1".to_string(),
            to: "3".to_string(),
            edge_type: WeaveEdgeType::Sequence,
            reason: None,
        });
        phase.weave_edges.push(WeaveEdge {
            from: "2".to_string(),
            to: "3".to_string(),
            edge_type: WeaveEdgeType::Exclusion,
            reason: Some("different-agents".to_string()),
        });

        let scg = serialize_scg(&phase);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.weave_edges.len(), 3);
        assert_eq!(parsed.weave_edges[0].edge_type, WeaveEdgeType::Conflict);
        assert_eq!(parsed.weave_edges[0].reason, Some("shared-config".to_string()));
        assert_eq!(parsed.weave_edges[1].edge_type, WeaveEdgeType::Sequence);
        assert!(parsed.weave_edges[1].reason.is_none());
        assert_eq!(parsed.weave_edges[2].edge_type, WeaveEdgeType::Exclusion);
    }

    #[test]
    fn test_node_annotations_round_trip() {
        let mut phase = Phase::new("ann-rt".to_string());
        let task1 = Task::new("1".to_string(), "T1".to_string(), String::new());
        let task2 = Task::new("2".to_string(), "T2".to_string(), String::new());
        phase.add_task(task1);
        phase.add_task(task2);

        phase.node_annotations.insert(
            "1".to_string(),
            NodeAnnotation {
                role: Some("r:impl".to_string()),
                scope: Some("src/auth/**".to_string()),
            },
        );
        // Node 2 has annotation with only role, no scope
        phase.node_annotations.insert(
            "2".to_string(),
            NodeAnnotation {
                role: Some("r:test".to_string()),
                scope: None,
            },
        );

        let scg = serialize_scg(&phase);
        let parsed = parse_scg(&scg).unwrap();

        assert_eq!(parsed.node_annotations.len(), 2);
        let ann1 = parsed.node_annotations.get("1").unwrap();
        assert_eq!(ann1.role, Some("r:impl".to_string()));
        assert_eq!(ann1.scope, Some("src/auth/**".to_string()));

        let ann2 = parsed.node_annotations.get("2").unwrap();
        assert_eq!(ann2.role, Some("r:test".to_string()));
        assert!(ann2.scope.is_none());
    }

    #[test]
    fn test_parse_scg_to_coordinator_evaluate() {
        // Integration test: parse SCG -> create Coordinator -> evaluate event
        use crate::weave::coordinator::Coordinator;
        use crate::weave::event::Event;

        let content = "\
# SCUD Graph v1
# Phase: integration

@nodes
1 | Task one | I | 5 | H

@weave
w:1 | File mutex | 5 | Y | Mutex | kind=FileWrite key=file:{target}
w:2 | No danger | 1 | Y | BlockAlways | kind=DangerousCommand
";
        let phase = parse_scg(content).unwrap();

        // Build coordinator from phase
        let mut coord = Coordinator::new();
        coord.threads = phase.weave_threads;
        coord.roles = phase.roles;
        coord.partitions = phase.partitions;
        for lock in &phase.locks {
            coord.active_locks.insert(lock.lock_key.clone(), lock.clone());
        }

        // DangerousCommand should be blocked
        let event = Event {
            kind: EventKind::DangerousCommand,
            agent: Some("agent-1".to_string()),
            target: Some("rm -rf /".to_string()),
            task_id: None,
            metadata: std::collections::HashMap::new(),
        };
        assert!(coord.evaluate(&event).is_blocked());

        // Normal FileWrite should proceed (no locks held)
        let event = Event {
            kind: EventKind::FileWrite,
            agent: Some("agent-1".to_string()),
            target: Some("src/main.rs".to_string()),
            task_id: None,
            metadata: std::collections::HashMap::new(),
        };
        assert!(coord.evaluate(&event).is_proceed());
    }

    #[test]
    fn test_parse_scg_with_locks_blocks_correctly() {
        // Integration: parse SCG with active locks -> evaluate -> verify blocking
        use crate::weave::coordinator::Coordinator;
        use crate::weave::event::Event;

        let content = "\
# SCUD Graph v1
# Phase: lock-test

@nodes
1 | Task | I | 5 | H

@weave
w:1 | File mutex | 5 | Y | Mutex | kind=FileWrite key=file:{target}

@locks
file:src/main.rs | agent-1 | 1 | 2026-02-06T10:05:00Z | 99999
";
        let phase = parse_scg(content).unwrap();

        let mut coord = Coordinator::new();
        coord.threads = phase.weave_threads;
        for lock in &phase.locks {
            coord.active_locks.insert(lock.lock_key.clone(), lock.clone());
        }

        // agent-2 tries to write locked file -> BLOCKED
        let event = Event {
            kind: EventKind::FileWrite,
            agent: Some("agent-2".to_string()),
            target: Some("src/main.rs".to_string()),
            task_id: None,
            metadata: std::collections::HashMap::new(),
        };
        assert!(coord.evaluate(&event).is_blocked());

        // agent-1 (lock holder) can still write -> PROCEED
        let event = Event {
            kind: EventKind::FileWrite,
            agent: Some("agent-1".to_string()),
            target: Some("src/main.rs".to_string()),
            task_id: None,
            metadata: std::collections::HashMap::new(),
        };
        assert!(coord.evaluate(&event).is_proceed());
    }

    #[test]
    fn test_parse_weave_partition() {
        let content = "\
# SCUD Graph v1
# Phase: test

@nodes
1 | Task | P | 5 | H

@weave
w:1 | Partition rule | 20 | Y | Partition | kind=FileWrite strategy=directory count=4
";
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.weave_threads.len(), 1);
        match &phase.weave_threads[0].rules[0] {
            BThreadRule::Partition { strategy, agent_count, .. } => {
                assert_eq!(*strategy, PartitionStrategy::Directory);
                assert_eq!(*agent_count, 4);
            }
            _ => panic!("Expected Partition rule"),
        }
    }

    #[test]
    fn test_parse_weave_block_until() {
        let content = "\
# SCUD Graph v1
# Phase: test

@nodes
1 | Task | P | 5 | H

@weave
w:1 | API gate | 15 | Y | BlockUntil | trigger=ApiChange block=Build until=ApiReviewApproved escalate=true
";
        let phase = parse_scg(content).unwrap();
        assert_eq!(phase.weave_threads.len(), 1);
        match &phase.weave_threads[0].rules[0] {
            BThreadRule::BlockUntil { trigger, block, until, escalate, .. } => {
                assert_eq!(trigger.kind, Some(EventKind::ApiChange));
                assert_eq!(block[0].kind, Some(EventKind::Build));
                assert_eq!(until.kind, Some(EventKind::Custom("ApiReviewApproved".to_string())));
                assert!(*escalate);
            }
            _ => panic!("Expected BlockUntil rule"),
        }
    }

    #[test]
    fn test_empty_weave_sections_not_serialized() {
        // A phase with no weave data should not emit @weave, @roles, etc. sections
        let mut phase = Phase::new("clean".to_string());
        let task = Task::new("1".to_string(), "Basic task".to_string(), String::new());
        phase.add_task(task);

        let scg = serialize_scg(&phase);
        assert!(!scg.contains("@weave"));
        assert!(!scg.contains("@roles"));
        assert!(!scg.contains("@partitions"));
        assert!(!scg.contains("@locks"));
    }

    #[test]
    fn test_full_scg_example_from_prd() {
        // Test the full example from Part III of the PRD
        let content = "\
# SCUD Graph v1
# Phase: auth

@nodes
auth:1   | Design auth system      | X | 13 | H
auth:1.1 | Implement JWT tokens    | I | 5  | H | role=r:impl scope=src/auth/**
auth:1.2 | Add rate limiting       | P | 8  | M | role=r:impl scope=src/middleware/**
auth:1.3 | Write auth tests        | P | 5  | M | role=r:test scope=tests/auth/**

@edges
auth:1.1 -> auth:1
auth:1.2 -> auth:1
auth:1.3 -> auth:1
auth:1.1 >> auth:1.3 | reason=tests-need-impl
auth:1.1 ~~ auth:1.2 | reason=shared-config-module
auth:1.1 != auth:1.3 | reason=different-specialization

@parents
auth:1: auth:1.1, auth:1.2, auth:1.3

@roles
r:impl | Implementer | src/** | docs/**
r:test | Test writer | tests/** | src/lib.rs

@weave
w:1 | File mutex   | 5  | Y | Mutex      | kind=FileWrite key=file:{target}
w:2 | Test gate    | 10 | Y | Require    | trigger=Commit prereq=TestPass reset=FileWrite

@locks
file:src/auth/jwt.rs | agent-1 | auth:1.1 | 2026-02-06T10:05:00Z | 3600
";
        let phase = parse_scg(content).unwrap();

        // Basic structure
        assert_eq!(phase.name, "auth");
        assert_eq!(phase.tasks.len(), 4);

        // Dependencies
        let t11 = phase.get_task("auth:1.1").unwrap();
        assert_eq!(t11.dependencies, vec!["auth:1"]);

        // Behavioral edges
        assert_eq!(phase.weave_edges.len(), 3);
        assert_eq!(phase.weave_edges[0].edge_type, WeaveEdgeType::Sequence);
        assert_eq!(phase.weave_edges[1].edge_type, WeaveEdgeType::Conflict);
        assert_eq!(phase.weave_edges[2].edge_type, WeaveEdgeType::Exclusion);

        // Node annotations
        assert_eq!(phase.node_annotations.len(), 3);
        let ann = phase.node_annotations.get("auth:1.1").unwrap();
        assert_eq!(ann.role, Some("r:impl".to_string()));
        assert_eq!(ann.scope, Some("src/auth/**".to_string()));

        // Roles
        assert_eq!(phase.roles.len(), 2);
        assert_eq!(phase.roles[0].id, "r:impl");

        // Weave threads
        assert_eq!(phase.weave_threads.len(), 2);
        assert_eq!(phase.weave_threads[0].id, "w:1");

        // Locks
        assert_eq!(phase.locks.len(), 1);
        assert_eq!(phase.locks[0].lock_key, "file:src/auth/jwt.rs");
        assert_eq!(phase.locks[0].holder_agent, "agent-1");
    }
}
