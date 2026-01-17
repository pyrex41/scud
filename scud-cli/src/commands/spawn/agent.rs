//! Agent prompt generation for Claude Code sessions
//!
//! Creates prompts that provide task context and instructions for Claude Code agents.

use crate::commands::swarm::session::WaveSummary;
use crate::models::task::Task;

/// Generate a prompt for Claude Code with task context
pub fn generate_prompt(task: &Task, tag: &str) -> String {
    let mut prompt = format!(
        r#"You are working on SCUD task {id}: {title}

Tag: {tag}
Complexity: {complexity}
Priority: {priority:?}

Description:
{description}
"#,
        id = task.id,
        title = task.title,
        tag = tag,
        complexity = task.complexity,
        priority = task.priority,
        description = task.description,
    );

    // Add details if present
    if let Some(ref details) = task.details {
        prompt.push_str(&format!(
            r#"
Technical Details:
{}
"#,
            details
        ));
    }

    // Add test strategy if present
    if let Some(ref test_strategy) = task.test_strategy {
        prompt.push_str(&format!(
            r#"
Test Strategy:
{}
"#,
            test_strategy
        ));
    }

    // Add dependencies info if any
    if !task.dependencies.is_empty() {
        prompt.push_str(&format!(
            r#"
Dependencies (should be done):
{}
"#,
            task.dependencies.join(", ")
        ));
    }

    // Add instructions
    prompt.push_str(&format!(
        r#"
Instructions:
1. First, explore the codebase to understand the context for this task
2. Implement the task following project conventions and patterns
3. Write tests if applicable based on the test strategy
4. When complete, run: scud set-status {} done
5. If blocked by issues, run: scud set-status {} blocked

Begin by understanding what needs to be done and exploring relevant code.
"#,
        task.id, task.id
    ));

    prompt
}

/// Generate a shorter prompt for tasks with less context
pub fn generate_minimal_prompt(task: &Task, tag: &str) -> String {
    format!(
        r#"SCUD Task {}: {}

Tag: {}
Description: {}

When done: scud set-status {} done
If blocked: scud set-status {} blocked
"#,
        task.id, task.title, tag, task.description, task.id, task.id
    )
}

/// Generate a prompt using a custom template
///
/// Template placeholders:
/// - {task.id} - Task ID
/// - {task.title} - Task title
/// - {task.description} - Task description
/// - {task.complexity} - Complexity score
/// - {task.priority} - Priority level
/// - {task.details} - Technical details (empty if none)
/// - {task.test_strategy} - Test strategy (empty if none)
/// - {task.dependencies} - Comma-separated dependencies
/// - {tag} - Phase/tag name
pub fn generate_prompt_with_template(task: &Task, tag: &str, template: &str) -> String {
    let mut result = template.to_string();

    result = result.replace("{task.id}", &task.id);
    result = result.replace("{task.title}", &task.title);
    result = result.replace("{task.description}", &task.description);
    result = result.replace("{task.complexity}", &task.complexity.to_string());
    result = result.replace("{task.priority}", &format!("{:?}", task.priority));
    result = result.replace("{task.details}", task.details.as_deref().unwrap_or(""));
    result = result.replace("{task.test_strategy}", task.test_strategy.as_deref().unwrap_or(""));
    result = result.replace("{task.dependencies}", &task.dependencies.join(", "));
    result = result.replace("{tag}", tag);

    result
}

/// Generate a prompt for wave review
pub fn generate_review_prompt(
    summary: &WaveSummary,
    tasks: &[(String, String)], // (task_id, title)
    review_all: bool,
) -> String {
    let tasks_str = if review_all {
        tasks
            .iter()
            .map(|(id, title)| format!("- {} | {}", id, title))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        // Sample: first task, last task, and one random middle task
        let sample: Vec<_> = if tasks.len() <= 3 {
            tasks.iter().collect()
        } else {
            vec![&tasks[0], &tasks[tasks.len() / 2], &tasks[tasks.len() - 1]]
        };
        sample
            .iter()
            .map(|(id, title)| format!("- {} | {}", id, title))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let files_str = if summary.files_changed.len() <= 10 {
        summary.files_changed.join("\n")
    } else {
        let mut s = summary.files_changed[..10].join("\n");
        s.push_str(&format!(
            "\n... and {} more files",
            summary.files_changed.len() - 10
        ));
        s
    };

    format!(
        r#"You are reviewing SCUD wave {wave_number}.

## Tasks to Review
{tasks}

## Files Changed
{files}

## Review Process
1. For each task, run: scud show <task_id>
2. Read the changed files relevant to each task
3. Check implementation quality and correctness

## Output Format
For each task:
  PASS: <task_id> - looks good
  IMPROVE: <task_id> - <specific issue>

When complete, create marker file:
  echo "REVIEW_COMPLETE: ALL_PASS" > .scud/review-complete-{wave_number}
Or if improvements needed:
  echo "REVIEW_COMPLETE: IMPROVEMENTS_NEEDED" > .scud/review-complete-{wave_number}
  echo "IMPROVE_TASKS: <comma-separated task IDs>" >> .scud/review-complete-{wave_number}
"#,
        wave_number = summary.wave_number,
        tasks = tasks_str,
        files = files_str,
    )
}

/// Generate a prompt for repair agent
pub fn generate_repair_prompt(
    task_id: &str,
    task_title: &str,
    failed_command: &str,
    error_output: &str,
    task_files: &[String],
    error_files: &[String],
) -> String {
    let task_files_str = task_files.join(", ");
    let error_files_str = error_files.join(", ");

    format!(
        r#"You are a repair agent fixing validation failures for SCUD task {task_id}: {task_title}

## Validation Failure
The following validation command failed:
{failed_command}

Error output:
{error_output}

## Attribution
This failure has been attributed to task {task_id} based on git blame analysis.
Files changed by this task: {task_files}

## Your Mission
1. Analyze the error output to understand what went wrong
2. Read the relevant files: {error_files}
3. Fix the issue while preserving the task's intended functionality
4. Run the validation command to verify the fix: {failed_command}

## Important
- Focus on fixing the specific error, don't refactor unrelated code
- If the fix requires changes to other tasks' code, note it but don't modify
- After fixing, commit with: scud commit -m "fix: {task_id} - <description>"

When the validation passes:
  scud set-status {task_id} done
  echo "REPAIR_COMPLETE: SUCCESS" > .scud/repair-complete-{task_id}

If you cannot fix it:
  scud set-status {task_id} blocked
  echo "REPAIR_COMPLETE: BLOCKED" > .scud/repair-complete-{task_id}
  echo "REASON: <explanation>" >> .scud/repair-complete-{task_id}
"#,
        task_id = task_id,
        task_title = task_title,
        failed_command = failed_command,
        error_output = error_output,
        task_files = task_files_str,
        error_files = error_files_str,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::task::Task;

    #[test]
    fn test_generate_prompt_basic() {
        let task = Task::new(
            "auth:1".to_string(),
            "Implement login".to_string(),
            "Add user authentication flow".to_string(),
        );

        let prompt = generate_prompt(&task, "auth");

        assert!(prompt.contains("auth:1"));
        assert!(prompt.contains("Implement login"));
        assert!(prompt.contains("Tag: auth"));
        assert!(prompt.contains("scud set-status auth:1 done"));
    }

    #[test]
    fn test_generate_prompt_with_details() {
        let mut task = Task::new(
            "api:2".to_string(),
            "Add endpoint".to_string(),
            "Create REST endpoint".to_string(),
        );
        task.details = Some("Use Express.js router pattern".to_string());
        task.test_strategy = Some("Unit test with Jest".to_string());

        let prompt = generate_prompt(&task, "api");

        assert!(prompt.contains("Technical Details:"));
        assert!(prompt.contains("Express.js router"));
        assert!(prompt.contains("Test Strategy:"));
        assert!(prompt.contains("Unit test with Jest"));
    }

    #[test]
    fn test_generate_minimal_prompt() {
        let task = Task::new(
            "fix:1".to_string(),
            "Quick fix".to_string(),
            "Fix typo".to_string(),
        );

        let prompt = generate_minimal_prompt(&task, "fix");

        assert!(prompt.contains("fix:1"));
        assert!(prompt.contains("Quick fix"));
        assert!(!prompt.contains("Technical Details"));
    }

    #[test]
    fn test_generate_prompt_with_template() {
        let mut task = Task::new(
            "auth:1".to_string(),
            "Login Feature".to_string(),
            "Implement login".to_string(),
        );
        task.complexity = 5;
        task.details = Some("Use OAuth".to_string());

        let template = "Task: {task.id} - {task.title}\nTag: {tag}\nDetails: {task.details}";
        let prompt = generate_prompt_with_template(&task, "auth", template);

        assert_eq!(prompt, "Task: auth:1 - Login Feature\nTag: auth\nDetails: Use OAuth");
    }

    #[test]
    fn test_generate_prompt_with_template_missing_fields() {
        let task = Task::new(
            "1".to_string(),
            "Title".to_string(),
            "Desc".to_string(),
        );

        let template = "Details: {task.details} | Strategy: {task.test_strategy}";
        let prompt = generate_prompt_with_template(&task, "test", template);

        assert_eq!(prompt, "Details:  | Strategy: ");
    }

    #[test]
    fn test_generate_review_prompt_all() {
        let summary = WaveSummary {
            wave_number: 1,
            tasks_completed: vec!["auth:1".to_string(), "auth:2".to_string()],
            files_changed: vec!["src/auth.rs".to_string(), "src/main.rs".to_string()],
        };

        let tasks = vec![
            ("auth:1".to_string(), "Add login".to_string()),
            ("auth:2".to_string(), "Add logout".to_string()),
        ];

        let prompt = generate_review_prompt(&summary, &tasks, true);

        assert!(prompt.contains("wave 1"));
        assert!(prompt.contains("auth:1 | Add login"));
        assert!(prompt.contains("auth:2 | Add logout"));
        assert!(prompt.contains("src/auth.rs"));
    }

    #[test]
    fn test_generate_review_prompt_sampled() {
        let summary = WaveSummary {
            wave_number: 2,
            tasks_completed: vec![
                "t:1".to_string(),
                "t:2".to_string(),
                "t:3".to_string(),
                "t:4".to_string(),
                "t:5".to_string(),
            ],
            files_changed: vec!["a.rs".to_string()],
        };

        let tasks: Vec<_> = (1..=5)
            .map(|i| (format!("t:{}", i), format!("Task {}", i)))
            .collect();

        let prompt = generate_review_prompt(&summary, &tasks, false);

        // Should only include first, middle, and last (3 tasks sampled)
        assert!(prompt.contains("t:1"));
        assert!(prompt.contains("t:3")); // middle
        assert!(prompt.contains("t:5")); // last
        // t:2 and t:4 should not be present
        assert!(!prompt.contains("t:2 | Task 2"));
        assert!(!prompt.contains("t:4 | Task 4"));
    }

    #[test]
    fn test_generate_repair_prompt() {
        let prompt = generate_repair_prompt(
            "auth:1",
            "Add login",
            "cargo build",
            "error: mismatched types at src/main.rs:42",
            &["src/auth.rs".to_string()],
            &["src/main.rs".to_string()],
        );

        assert!(prompt.contains("auth:1"));
        assert!(prompt.contains("Add login"));
        assert!(prompt.contains("cargo build"));
        assert!(prompt.contains("mismatched types"));
        assert!(prompt.contains("src/auth.rs"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("REPAIR_COMPLETE"));
    }
}
