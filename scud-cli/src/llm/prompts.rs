pub struct Prompts;

impl Prompts {
    pub fn parse_prd(epic_content: &str) -> String {
        format!(
            r#"You are a Scrum Master parsing an epic into actionable development tasks.

Epic Content:
{}

Parse this epic into discrete, actionable tasks. Return a JSON array of tasks with the following structure:

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
            epic_content
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
    ) -> String {
        let context = existing_details
            .map(|d| format!("\nExisting Technical Details:\n{}\n", d))
            .unwrap_or_default();

        format!(
            r#"You are breaking down a complex task into smaller, manageable subtasks.

Original Task (Complexity {}): {}
Description: {}{}

This task is too complex (>13 points) and needs to be broken down into smaller subtasks.

Create subtasks that:
- Each have complexity ≤ 8 (ideally ≤ 5)
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
    "complexity": <1-8>,
    "dependencies": []  // IDs of other subtasks this depends on
  }}
]

Guidelines:
- Start with foundational work (models, schemas)
- Then build core logic
- Then add UI/API layers
- Finally add tests and documentation
- Each subtask should take at most 8 hours
- Use dependencies to enforce correct order

Return ONLY the JSON array, no additional explanation."#,
            complexity, task_title, task_description, context
        )
    }

    pub fn research_topic(query: &str) -> String {
        format!(
            r#"You are a technical research assistant helping a developer.

Research Query: {}

Provide a comprehensive but concise response covering:
1. Key concepts and best practices
2. Common pitfalls to avoid
3. Recommended approaches
4. Code examples if relevant
5. Links to documentation or resources (if you're aware of them)

Focus on practical, actionable information that helps with implementation.
Be concise but thorough - aim for 200-400 words.

Format your response in markdown."#,
            query
        )
    }
}
