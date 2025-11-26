pub struct Prompts;

impl Prompts {
    pub fn parse_prd(phase_content: &str) -> String {
        format!(
            r#"You are a Scrum Master parsing a phase into actionable development tasks.

Phase Content:
{}

Parse this phase into discrete, actionable tasks. Return a JSON array of tasks with the following structure:

[
  {{
    "title": "Task name (concise, action-oriented)",
    "description": "What needs to be done (2-3 sentences)",
    "priority": "high|medium|low",
    "complexity": <1|2|3|5|8|13|21>,
    "dependencies": []
  }}
]

Guidelines:
- Each task should be atomic and independently testable
- Use Fibonacci complexity scale:
  * 1 = Trivial (~30 min, e.g., update config value)
  * 2 = Simple (30m-1h, e.g., add basic validation)
  * 3 = Moderate (1-2h, e.g., create new API endpoint)
  * 5 = Complex (2-4h, e.g., integrate third-party service)
  * 8 = Very Complex (4-8h, e.g., build feature with multiple components)
  * 13 = Extremely Complex (1 day, SHOULD BE SPLIT)
  * 21 = Too Large (MUST BE SPLIT - only use if absolutely necessary)
- Identify dependencies where tasks must be done in specific order (use task indices, e.g., ["1", "2"])
- Order tasks logically (foundational work first)
- Each task should have clear success criteria

Return ONLY the JSON array, no additional explanation."#,
            phase_content
        )
    }

    pub fn analyze_complexity(
        task_title: &str,
        task_description: &str,
        existing_details: Option<&str>,
    ) -> String {
        let context = existing_details
            .map(|d| format!("\nExisting Technical Details:\n{}\n", d))
            .unwrap_or_default();

        format!(
            r#"You are analyzing the complexity of a development task.

Task: {}
Description: {}{}

Analyze this task and provide:
1. A complexity score (1, 2, 3, 5, 8, 13, or 21) using Fibonacci scale
2. A brief reasoning explaining the score

Consider:
- Technical difficulty and unknowns
- Number of components/files affected
- Testing requirements
- Integration points and dependencies
- Research needed
- Edge cases to handle

Complexity Scale:
- 1 = Trivial (~30 min)
- 2 = Simple (30m-1h)
- 3 = Moderate (1-2h)
- 5 = Complex (2-4h)
- 8 = Very Complex (4-8h)
- 13 = Extremely Complex (1 day) - Should be split
- 21 = Too Large - Must be split

Return a JSON object:
{{
  "complexity": <number>,
  "reasoning": "explanation of the score"
}}

Return ONLY the JSON object, no additional explanation."#,
            task_title, task_description, context
        )
    }

    pub fn expand_task(
        task_title: &str,
        task_description: &str,
        complexity: u32,
        existing_details: Option<&str>,
        recommended_subtasks: usize,
    ) -> String {
        let context = existing_details
            .map(|d| format!("\nExisting Technical Details:\n{}\n", d))
            .unwrap_or_default();

        format!(
            r#"You are breaking down a development task into smaller, manageable subtasks.

Original Task (Complexity {}): {}
Description: {}{}

Break this task down into approximately {} subtasks based on its complexity.

Create subtasks that:
- Are small, focused, and independently completable
- Are independently testable
- Have clear dependencies between them
- Cover all aspects of the original task
- Maintain logical order

Return a JSON array of subtasks:
[
  {{
    "title": "Subtask name",
    "description": "What needs to be done",
    "priority": "high|medium|low",
    "dependencies": []  // Array of strings: ["1", "2", "3"] for subtask dependencies, or ["TASK-123"] for external dependencies
  }}
]

Guidelines:
- Start with foundational work (models, schemas)
- Then build core logic
- Then add UI/API layers
- Finally add tests and documentation
- Each subtask should be independently completable
- Use dependencies to enforce correct order (e.g., ["1"] means depends on first subtask)
- Dependency values MUST be strings, not numbers
- Aim for {} subtasks total (can vary by 1-2 if needed for logical breakdown)
- DO NOT include "complexity" field - subtasks are all assumed to be small and manageable

Return ONLY the JSON array, no additional explanation."#,
            complexity,
            task_title,
            task_description,
            context,
            recommended_subtasks,
            recommended_subtasks
        )
    }
}
