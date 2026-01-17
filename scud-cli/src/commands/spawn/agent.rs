//! Agent prompt generation for Claude Code sessions
//!
//! Creates prompts that provide task context and instructions for Claude Code agents.

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
}
