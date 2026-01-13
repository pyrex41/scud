//! Smart model review for Ralph Wiggum mode
//!
//! After each wave completes, the smart model reviews:
//! 1. Task prompts (what was asked)
//! 2. Git diffs (what was changed)
//! 3. Agent summaries (if available)
//!
//! The smart model then:
//! - Identifies issues and potential bugs
//! - Applies fixes if needed
//! - Generates recommendations for next wave

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use crate::llm::LLMClient;
use crate::models::phase::Phase;
use crate::models::task::Task;

use super::session::WaveState;

/// Result of reviewing a wave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    /// Wave number reviewed
    pub wave_number: usize,
    /// Number of tasks reviewed
    pub tasks_reviewed: usize,
    /// Issues found during review
    pub issues_found: Vec<String>,
    /// Fixes that were applied
    pub fixes_applied: Vec<String>,
    /// Recommendations for next wave
    pub recommendations: Vec<String>,
    /// Per-task review details
    pub task_reviews: Vec<TaskReview>,
    /// Overall summary
    pub summary: String,
    /// Review timestamp
    pub reviewed_at: String,
}

impl Default for ReviewResult {
    fn default() -> Self {
        Self {
            wave_number: 0,
            tasks_reviewed: 0,
            issues_found: Vec::new(),
            fixes_applied: Vec::new(),
            recommendations: Vec::new(),
            task_reviews: Vec::new(),
            summary: String::new(),
            reviewed_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Review details for a single task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskReview {
    /// Task ID
    pub task_id: String,
    /// Task title
    pub task_title: String,
    /// Files changed
    pub files_changed: Vec<String>,
    /// Lines added
    pub lines_added: usize,
    /// Lines removed
    pub lines_removed: usize,
    /// Issues found in this task
    pub issues: Vec<String>,
    /// Whether the task passed review
    pub passed: bool,
    /// Review notes
    pub notes: String,
}

/// Context for reviewing a task
struct TaskContext {
    task_id: String,
    task_title: String,
    task_description: String,
    diff: String,
    files_changed: Vec<String>,
    lines_added: usize,
    lines_removed: usize,
}

/// Review a completed wave using the smart model
pub async fn review_wave(
    project_root: Option<PathBuf>,
    wave: &WaveState,
    all_phases: &HashMap<String, Phase>,
    _phase_tag: &str,
    model_override: Option<&str>,
) -> Result<ReviewResult> {
    let task_tags = wave.task_tags();

    if task_tags.is_empty() {
        return Ok(ReviewResult {
            wave_number: wave.wave_number,
            summary: "No tasks to review".to_string(),
            ..Default::default()
        });
    }

    // Collect context for each task
    let mut task_contexts: Vec<TaskContext> = Vec::new();

    for (task_id, tag) in &task_tags {
        if let Some(phase) = all_phases.get(tag) {
            if let Some(task) = phase.get_task(task_id) {
                let context = collect_task_context(project_root.as_ref(), task, task_id)?;
                task_contexts.push(context);
            }
        }
    }

    // Skip review if no changes detected
    let total_changes: usize = task_contexts.iter()
        .map(|c| c.lines_added + c.lines_removed)
        .sum();

    if total_changes == 0 {
        return Ok(ReviewResult {
            wave_number: wave.wave_number,
            tasks_reviewed: task_contexts.len(),
            summary: "No code changes detected in this wave".to_string(),
            ..Default::default()
        });
    }

    // Build review prompt
    let prompt = build_review_prompt(&task_contexts);

    // Call smart model
    let llm = LLMClient::new_with_project_root(
        project_root.unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    )?;

    let response = llm.complete_smart(&prompt, model_override).await?;

    // Parse review response
    let parsed = parse_review_response(&response, &task_contexts)?;

    Ok(ReviewResult {
        wave_number: wave.wave_number,
        tasks_reviewed: task_contexts.len(),
        issues_found: parsed.issues,
        fixes_applied: Vec::new(), // Fixes applied separately
        recommendations: parsed.recommendations,
        task_reviews: parsed.task_reviews,
        summary: parsed.summary,
        reviewed_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Collect context for a task (diff, files changed, etc.)
fn collect_task_context(
    project_root: Option<&PathBuf>,
    task: &Task,
    task_id: &str,
) -> Result<TaskContext> {
    let working_dir = project_root
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Get recent commits that might be related to this task
    // Look for commits with task ID in message, or recent commits
    let diff = get_task_diff(&working_dir, task_id)?;
    let (files_changed, lines_added, lines_removed) = parse_diff_stats(&diff);

    Ok(TaskContext {
        task_id: task_id.to_string(),
        task_title: task.title.clone(),
        task_description: task.description.clone(),
        diff,
        files_changed,
        lines_added,
        lines_removed,
    })
}

/// Get git diff for a task
/// Looks for commits with task ID, or uses recent changes
fn get_task_diff(working_dir: &std::path::Path, task_id: &str) -> Result<String> {
    // First, try to find commits with this task ID
    let log_output = Command::new("git")
        .current_dir(working_dir)
        .args(["log", "--oneline", "-n", "20", "--grep", task_id])
        .output()
        .context("Failed to run git log")?;

    let log_str = String::from_utf8_lossy(&log_output.stdout);
    let commits: Vec<&str> = log_str.lines().collect();

    if !commits.is_empty() {
        // Get diff for these commits
        let first_commit = commits.last().unwrap().split_whitespace().next().unwrap_or("");
        let diff_output = Command::new("git")
            .current_dir(working_dir)
            .args(["diff", &format!("{}^..HEAD", first_commit), "--", "."])
            .output()
            .context("Failed to run git diff")?;

        let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();
        if !diff.is_empty() {
            return Ok(truncate_diff(&diff, 10000));
        }
    }

    // Fallback: get diff of recent changes (staged + unstaged)
    let diff_output = Command::new("git")
        .current_dir(working_dir)
        .args(["diff", "HEAD~5..HEAD", "--", "."])
        .output()
        .context("Failed to run git diff")?;

    let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();
    Ok(truncate_diff(&diff, 10000))
}

/// Truncate diff to max length
fn truncate_diff(diff: &str, max_len: usize) -> String {
    if diff.len() <= max_len {
        diff.to_string()
    } else {
        format!(
            "{}...\n\n[Diff truncated - {} more bytes]",
            &diff[..max_len],
            diff.len() - max_len
        )
    }
}

/// Parse diff statistics
fn parse_diff_stats(diff: &str) -> (Vec<String>, usize, usize) {
    let mut files: Vec<String> = Vec::new();
    let mut added = 0;
    let mut removed = 0;

    for line in diff.lines() {
        if line.starts_with("+++ b/") {
            files.push(line[6..].to_string());
        } else if line.starts_with('+') && !line.starts_with("+++") {
            added += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            removed += 1;
        }
    }

    (files, added, removed)
}

/// Build the review prompt
fn build_review_prompt(contexts: &[TaskContext]) -> String {
    let mut prompt = String::from(
r#"You are a senior software engineer reviewing code changes from an AI development session.

Review the following tasks and their associated code changes. For each task:
1. Check if the implementation matches the task requirements
2. Look for bugs, security issues, or code quality problems
3. Identify any incomplete work or missing tests
4. Note any patterns that could be improved

## Tasks and Changes

"#);

    for ctx in contexts {
        prompt.push_str(&format!(
            r#"### Task: {} - {}

**Description:** {}

**Files Changed:** {}
**Lines:** +{} / -{}

**Diff:**
```diff
{}
```

---

"#,
            ctx.task_id,
            ctx.task_title,
            ctx.task_description,
            ctx.files_changed.join(", "),
            ctx.lines_added,
            ctx.lines_removed,
            ctx.diff
        ));
    }

    prompt.push_str(
r#"## Response Format

Respond with a JSON object:
```json
{
  "summary": "Overall review summary (1-2 sentences)",
  "issues": ["List of issues found across all tasks"],
  "recommendations": ["Recommendations for next wave"],
  "task_reviews": [
    {
      "task_id": "task:id",
      "passed": true/false,
      "issues": ["Issues specific to this task"],
      "notes": "Brief notes about the implementation"
    }
  ]
}
```

Be concise but thorough. Focus on actionable feedback.
"#);

    prompt
}

/// Parsed review response
struct ParsedReview {
    summary: String,
    issues: Vec<String>,
    recommendations: Vec<String>,
    task_reviews: Vec<TaskReview>,
}

/// Parse the smart model's review response
fn parse_review_response(response: &str, contexts: &[TaskContext]) -> Result<ParsedReview> {
    // Try to parse as JSON
    #[derive(Deserialize)]
    struct ReviewJson {
        summary: Option<String>,
        issues: Option<Vec<String>>,
        recommendations: Option<Vec<String>>,
        task_reviews: Option<Vec<TaskReviewJson>>,
    }

    #[derive(Deserialize)]
    struct TaskReviewJson {
        task_id: String,
        passed: Option<bool>,
        issues: Option<Vec<String>>,
        notes: Option<String>,
    }

    // Extract JSON from response
    let json_str = extract_json(response);

    match serde_json::from_str::<ReviewJson>(json_str) {
        Ok(parsed) => {
            let task_reviews: Vec<TaskReview> = parsed.task_reviews
                .unwrap_or_default()
                .into_iter()
                .map(|tr| {
                    let ctx = contexts.iter().find(|c| c.task_id == tr.task_id);
                    TaskReview {
                        task_id: tr.task_id,
                        task_title: ctx.map(|c| c.task_title.clone()).unwrap_or_default(),
                        files_changed: ctx.map(|c| c.files_changed.clone()).unwrap_or_default(),
                        lines_added: ctx.map(|c| c.lines_added).unwrap_or(0),
                        lines_removed: ctx.map(|c| c.lines_removed).unwrap_or(0),
                        issues: tr.issues.unwrap_or_default(),
                        passed: tr.passed.unwrap_or(true),
                        notes: tr.notes.unwrap_or_default(),
                    }
                })
                .collect();

            Ok(ParsedReview {
                summary: parsed.summary.unwrap_or_else(|| "Review complete".to_string()),
                issues: parsed.issues.unwrap_or_default(),
                recommendations: parsed.recommendations.unwrap_or_default(),
                task_reviews,
            })
        }
        Err(_) => {
            // Fallback: create basic review from response text
            Ok(ParsedReview {
                summary: response.lines().next().unwrap_or("Review complete").to_string(),
                issues: Vec::new(),
                recommendations: Vec::new(),
                task_reviews: contexts.iter().map(|ctx| TaskReview {
                    task_id: ctx.task_id.clone(),
                    task_title: ctx.task_title.clone(),
                    files_changed: ctx.files_changed.clone(),
                    lines_added: ctx.lines_added,
                    lines_removed: ctx.lines_removed,
                    issues: Vec::new(),
                    passed: true,
                    notes: String::new(),
                }).collect(),
            })
        }
    }
}

/// Extract JSON from response text
fn extract_json(response: &str) -> &str {
    // Try markdown code block
    if let Some(start) = response.find("```json") {
        let content_start = start + 7;
        if let Some(end) = response[content_start..].find("```") {
            return response[content_start..content_start + end].trim();
        }
    }

    // Try plain code block
    if let Some(start) = response.find("```") {
        let content_start = start + 3;
        if let Some(end) = response[content_start..].find("```") {
            let block = response[content_start..content_start + end].trim();
            // Skip language identifier
            if let Some(newline) = block.find('\n') {
                return block[newline + 1..].trim();
            }
            return block;
        }
    }

    // Try to find JSON object
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            if end > start {
                return &response[start..=end];
            }
        }
    }

    response.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_diff_stats() {
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,5 @@
+// New comment
 fn main() {
-    println!("Hello");
+    println!("Hello, world!");
+    println!("Another line");
 }
"#;

        let (files, added, removed) = parse_diff_stats(diff);
        assert_eq!(files, vec!["src/main.rs"]);
        assert_eq!(added, 3);
        assert_eq!(removed, 1);
    }

    #[test]
    fn test_extract_json() {
        let response = r#"Here is my review:

```json
{
  "summary": "Good work",
  "issues": []
}
```

That's all!
"#;

        let json = extract_json(response);
        assert!(json.contains("summary"));
        assert!(json.contains("Good work"));
    }

    #[test]
    fn test_truncate_diff() {
        let short_diff = "short";
        assert_eq!(truncate_diff(short_diff, 100), "short");

        let long_diff = "a".repeat(200);
        let truncated = truncate_diff(&long_diff, 50);
        assert!(truncated.len() < 200);
        assert!(truncated.contains("truncated"));
    }
}
