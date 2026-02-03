//! Agent prompt generation for Claude Code sessions
//!
//! Creates prompts that provide task context and instructions for Claude Code agents.

use crate::agents::AgentDef;
use crate::commands::spawn::terminal::Harness;
use crate::commands::swarm::session::WaveSummary;
use crate::models::task::Task;
use std::path::Path;

/// Resolved configuration for spawning an agent
#[derive(Debug, Clone)]
pub struct ResolvedAgentConfig {
    /// The harness to use (claude or opencode)
    pub harness: Harness,
    /// The model to use (if specified)
    pub model: Option<String>,
    /// The prompt to send to the agent
    pub prompt: String,
    /// Whether this config came from an agent definition
    pub from_agent_def: bool,
    /// The agent type name (if from agent def)
    pub agent_type: Option<String>,
}

/// Resolve the agent configuration for a task.
///
/// If the task has an `agent_type`, attempts to load the agent definition
/// from `.scud/agents/<agent_type>.toml` and uses its harness, model, and
/// prompt template. Falls back to defaults if the agent definition is not found.
///
/// # Arguments
/// * `task` - The task to resolve configuration for
/// * `tag` - The task's tag/phase name
/// * `default_harness` - Harness to use if no agent definition specifies one
/// * `default_model` - Model to use if no agent definition specifies one
/// * `working_dir` - Project root for loading agent definitions
pub fn resolve_agent_config(
    task: &Task,
    tag: &str,
    default_harness: Harness,
    default_model: Option<&str>,
    working_dir: &Path,
) -> ResolvedAgentConfig {
    if let Some(ref agent_type) = task.agent_type {
        // Try to load agent definition
        match AgentDef::try_load(agent_type, working_dir) {
            Some(agent_def) => {
                let harness = agent_def.harness().unwrap_or(default_harness);
                let model = agent_def
                    .model()
                    .map(String::from)
                    .or_else(|| default_model.map(String::from));

                // Use custom prompt template if available
                let prompt = match agent_def.prompt_template(working_dir) {
                    Some(template) => generate_prompt_with_template(task, tag, &template),
                    None => generate_prompt(task, tag),
                };

                ResolvedAgentConfig {
                    harness,
                    model,
                    prompt,
                    from_agent_def: true,
                    agent_type: Some(agent_type.clone()),
                }
            }
            None => {
                // Agent type specified but no definition found - use defaults
                ResolvedAgentConfig {
                    harness: default_harness,
                    model: default_model.map(String::from),
                    prompt: generate_prompt(task, tag),
                    from_agent_def: false,
                    agent_type: Some(agent_type.clone()),
                }
            }
        }
    } else {
        // No agent type - use defaults
        ResolvedAgentConfig {
            harness: default_harness,
            model: default_model.map(String::from),
            prompt: generate_prompt(task, tag),
            from_agent_def: false,
            agent_type: None,
        }
    }
}

impl ResolvedAgentConfig {
    /// Get a display string for logging (e.g., "opencode:grok-code-fast-1@fast-builder")
    pub fn display_info(&self) -> String {
        let model_str = self
            .model
            .as_deref()
            .map(|m| format!(":{}", m))
            .unwrap_or_default();

        if let Some(ref agent_type) = self.agent_type {
            format!("{}{}@{}", self.harness.name(), model_str, agent_type)
        } else {
            format!("{}{}", self.harness.name(), model_str)
        }
    }
}

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
1. Check for discoveries from other agents: scud log-all --limit 10
2. Explore the codebase to understand the context for this task
3. Implement the task following project conventions and patterns
4. Log important discoveries to share with other agents:
   scud log {id} "Found X in Y, useful for Z"
5. Write tests if applicable based on the test strategy
6. When complete, run: scud set-status {id} done
7. If blocked by issues, run: scud set-status {id} blocked

Discovery Logging:
- Log findings that other agents might benefit from (file locations, patterns, gotchas)
- Keep logs concise but informative (1-3 sentences)
- Example: scud log {id} "Auth helpers are in lib/auth.rs, not utils/"

Begin by checking recent logs and exploring relevant code.
"#,
        id = task.id
    ));

    prompt
}

/// Generate a shorter prompt for tasks with less context
pub fn generate_minimal_prompt(task: &Task, tag: &str) -> String {
    format!(
        r#"SCUD Task {id}: {title}

Tag: {tag}
Description: {description}

First: scud log-all --limit 5 (check recent discoveries)
Log findings: scud log {id} "your discovery"
When done: scud set-status {id} done
If blocked: scud set-status {id} blocked
"#,
        id = task.id,
        title = task.title,
        tag = tag,
        description = task.description
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
    result = result.replace(
        "{task.test_strategy}",
        task.test_strategy.as_deref().unwrap_or(""),
    );
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
- Log what you fixed for other agents: scud log {task_id} "Fixed: <brief description>"

When the validation passes:
  scud log {task_id} "Repair successful: <what was fixed>"
  scud set-status {task_id} done
  echo "REPAIR_COMPLETE: SUCCESS" > .scud/repair-complete-{task_id}

If you cannot fix it:
  scud log {task_id} "Repair blocked: <reason>"
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

/// Generate a prompt for batch repair agent handling multiple tasks
pub fn generate_batch_repair_prompt(
    tasks: &[(String, String, Vec<String>)], // (task_id, title, files_changed)
    failed_command: &str,
    error_output: &str,
    error_locations: &[(String, Option<u32>)], // (file, line)
) -> String {
    let tasks_str = tasks
        .iter()
        .map(|(id, title, files)| format!("- {} | {}\n  Files: {}", id, title, files.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");

    let error_locations_str = error_locations
        .iter()
        .take(20) // Limit to avoid prompt explosion
        .map(|(file, line)| match line {
            Some(l) => format!("  {}:{}", file, l),
            None => format!("  {}", file),
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are a batch repair agent fixing validation failures for multiple SCUD tasks.

## Validation Failure
The following validation command failed:
{failed_command}

Error output:
{error_output}

## Error Locations
{error_locations}

## Responsible Tasks
Based on git blame analysis, these tasks may be responsible:
{tasks}

## Your Mission
1. Analyze the error output to understand ALL the issues
2. Read the relevant files and understand what each task was trying to do
3. Fix issues systematically - some errors may be related
4. Run the validation command after each fix to check progress: {failed_command}

## Process
For each issue:
1. Identify which task introduced it
2. Read the task details: scud show <task_id>
3. Fix the issue while preserving intended functionality
4. Commit: scud commit -m "fix: <task_id> - <description>"
5. Log: scud log <task_id> "Fixed: <brief description>"

## Important
- Fix ALL issues before signaling completion
- Some issues may cascade - fix root causes first
- If you cannot fix an issue, document why
- Iterate until validation passes or you're truly blocked

## Completion
When ALL validation passes:
  echo "BATCH_REPAIR_COMPLETE: SUCCESS" > .scud/batch-repair-complete
  echo "FIXED_TASKS: <comma-separated task IDs that were fixed>" >> .scud/batch-repair-complete

If blocked on some tasks:
  echo "BATCH_REPAIR_COMPLETE: PARTIAL" > .scud/batch-repair-complete
  echo "FIXED_TASKS: <task IDs fixed>" >> .scud/batch-repair-complete
  echo "BLOCKED_TASKS: <task IDs blocked>" >> .scud/batch-repair-complete
  echo "BLOCK_REASON: <explanation>" >> .scud/batch-repair-complete

If completely blocked:
  echo "BATCH_REPAIR_COMPLETE: BLOCKED" > .scud/batch-repair-complete
  echo "REASON: <explanation>" >> .scud/batch-repair-complete
"#,
        failed_command = failed_command,
        error_output = error_output,
        error_locations = error_locations_str,
        tasks = tasks_str,
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

        assert_eq!(
            prompt,
            "Task: auth:1 - Login Feature\nTag: auth\nDetails: Use OAuth"
        );
    }

    #[test]
    fn test_generate_prompt_with_template_missing_fields() {
        let task = Task::new("1".to_string(), "Title".to_string(), "Desc".to_string());

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

    // =========================================================================
    // Tests for resolve_agent_config
    // =========================================================================

    #[test]
    fn test_resolve_agent_config_no_agent_type() {
        let temp = tempfile::TempDir::new().unwrap();
        let task = Task::new("1".to_string(), "Test".to_string(), "Desc".to_string());

        let config = resolve_agent_config(&task, "test", Harness::Claude, None, temp.path());

        assert_eq!(config.harness, Harness::Claude);
        assert_eq!(config.model, None);
        assert!(!config.from_agent_def);
        assert!(config.agent_type.is_none());
    }

    #[test]
    fn test_resolve_agent_config_uses_default_model() {
        let temp = tempfile::TempDir::new().unwrap();
        let task = Task::new("1".to_string(), "Test".to_string(), "Desc".to_string());

        let config =
            resolve_agent_config(&task, "test", Harness::Claude, Some("opus"), temp.path());

        assert_eq!(config.harness, Harness::Claude);
        assert_eq!(config.model, Some("opus".to_string()));
        assert!(!config.from_agent_def);
    }

    #[test]
    fn test_resolve_agent_config_agent_type_not_found() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut task = Task::new("1".to_string(), "Test".to_string(), "Desc".to_string());
        task.agent_type = Some("nonexistent".to_string());

        let config =
            resolve_agent_config(&task, "test", Harness::Claude, Some("sonnet"), temp.path());

        // Should fall back to defaults when agent def not found
        assert_eq!(config.harness, Harness::Claude);
        assert_eq!(config.model, Some("sonnet".to_string()));
        assert!(!config.from_agent_def);
        assert_eq!(config.agent_type, Some("nonexistent".to_string()));
    }

    #[test]
    fn test_resolve_agent_config_from_agent_def() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents_dir = temp.path().join(".scud").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        // Create a fast-builder agent definition with opencode harness
        let agent_file = agents_dir.join("fast-builder.toml");
        std::fs::write(
            &agent_file,
            r#"
[agent]
name = "fast-builder"
description = "Fast builder"

[model]
harness = "opencode"
model = "xai/grok-code-fast-1"
"#,
        )
        .unwrap();

        let mut task = Task::new("1".to_string(), "Test".to_string(), "Desc".to_string());
        task.agent_type = Some("fast-builder".to_string());

        // Default harness is claude, but agent def should override to opencode
        let config =
            resolve_agent_config(&task, "test", Harness::Claude, Some("opus"), temp.path());

        assert_eq!(config.harness, Harness::OpenCode);
        assert_eq!(config.model, Some("xai/grok-code-fast-1".to_string()));
        assert!(config.from_agent_def);
        assert_eq!(config.agent_type, Some("fast-builder".to_string()));
    }

    #[test]
    fn test_resolve_agent_config_agent_def_without_model_uses_default() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents_dir = temp.path().join(".scud").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        // Agent def with harness but no model
        let agent_file = agents_dir.join("custom.toml");
        std::fs::write(
            &agent_file,
            r#"
[agent]
name = "custom"

[model]
harness = "opencode"
"#,
        )
        .unwrap();

        let mut task = Task::new("1".to_string(), "Test".to_string(), "Desc".to_string());
        task.agent_type = Some("custom".to_string());

        let config =
            resolve_agent_config(&task, "test", Harness::Claude, Some("opus"), temp.path());

        // Harness comes from agent def, model falls back to default
        assert_eq!(config.harness, Harness::OpenCode);
        assert_eq!(config.model, Some("opus".to_string()));
        assert!(config.from_agent_def);
    }

    #[test]
    fn test_resolve_agent_config_uses_custom_prompt_template() {
        let temp = tempfile::TempDir::new().unwrap();
        let agents_dir = temp.path().join(".scud").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let agent_file = agents_dir.join("templated.toml");
        std::fs::write(
            &agent_file,
            r#"
[agent]
name = "templated"

[model]
harness = "claude"

[prompt]
template = "Custom: {task.title} in {tag}"
"#,
        )
        .unwrap();

        let mut task = Task::new("1".to_string(), "My Task".to_string(), "Desc".to_string());
        task.agent_type = Some("templated".to_string());

        let config = resolve_agent_config(&task, "my-tag", Harness::Claude, None, temp.path());

        assert_eq!(config.prompt, "Custom: My Task in my-tag");
        assert!(config.from_agent_def);
    }

    #[test]
    fn test_resolved_agent_config_display_info() {
        let config = ResolvedAgentConfig {
            harness: Harness::OpenCode,
            model: Some("xai/grok-code-fast-1".to_string()),
            prompt: "test".to_string(),
            from_agent_def: true,
            agent_type: Some("fast-builder".to_string()),
        };

        assert_eq!(
            config.display_info(),
            "opencode:xai/grok-code-fast-1@fast-builder"
        );

        let config_no_model = ResolvedAgentConfig {
            harness: Harness::Claude,
            model: None,
            prompt: "test".to_string(),
            from_agent_def: false,
            agent_type: None,
        };

        assert_eq!(config_no_model.display_info(), "claude");
    }
}
