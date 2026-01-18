pub struct Prompts;

impl Prompts {
    pub fn parse_prd(phase_content: &str, num_tasks: u32, guidance: Option<&str>) -> String {
        let guidance_section = guidance
            .filter(|g| !g.is_empty())
            .map(|g| {
                format!(
                    r#"
## Project Guidance

The following project-specific guidance should inform your task breakdown:

{}

"#,
                    g
                )
            })
            .unwrap_or_default();

        format!(
            r#"You are a Scrum Master parsing a phase into actionable development tasks.
{}
Phase Content:
{}

Parse this phase into approximately {} discrete, actionable tasks. Return a JSON array of tasks with the following structure:

[
  {{
    "title": "Task name (concise, action-oriented)",
    "description": "What needs to be done (2-3 sentences)",
    "priority": "high|medium|low",
    "complexity": <1|2|3|5|8|13|21>,
    "dependencies": [],  // Use 1-indexed task references, e.g., ["1", "2"]. NEVER use "0" - indices start at 1.
    "agent_type": "fast-builder|builder|reviewer|planner|tester"  // Which type of agent is best suited for this task
  }}
]

Guidelines:
- Generate approximately {} tasks (can vary by 1-2 if needed for logical breakdown)
- Each task should be atomic and independently testable
- Use Fibonacci complexity scale:
  * 1 = Trivial (~30 min, e.g., update config value)
  * 2 = Simple (30m-1h, e.g., add basic validation)
  * 3 = Moderate (1-2h, e.g., create new API endpoint)
  * 5 = Complex (2-4h, e.g., integrate third-party service)
  * 8 = Very Complex (4-8h, e.g., build feature with multiple components)
  * 13 = Extremely Complex (1 day, SHOULD BE SPLIT)
  * 21 = Too Large (MUST BE SPLIT - only use if absolutely necessary)
- Dependencies use 1-indexed task references (first task is "1", NOT "0")
- NEVER reference task "0" - it does not exist
- Identify dependencies where tasks must be done in specific order (use task indices, e.g., ["1", "2"])
- Order tasks logically (foundational work first)
- Each task should have clear success criteria
- Assign agent_type based on BOTH task nature AND complexity:
  * "fast-builder" = Simple implementation tasks with complexity 1-2 (quick code changes, config updates, simple features)
  * "builder" = Complex implementation tasks with complexity 3+ (multi-file changes, new features, integrations)
  * "reviewer" = Code review, quality checks, refactoring tasks
  * "planner" = Design, architecture planning, research tasks
  * "tester" = Test writing, test automation, validation tasks
- IMPORTANT: Use "fast-builder" for complexity 1-2 implementation tasks, "builder" for complexity 3+ implementation tasks

Return ONLY the JSON array, no additional explanation."#,
            guidance_section, phase_content, num_tasks, num_tasks
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
        guidance: Option<&str>,
    ) -> String {
        let context = existing_details
            .map(|d| format!("\nExisting Technical Details:\n{}\n", d))
            .unwrap_or_default();

        let guidance_section = guidance
            .filter(|g| !g.is_empty())
            .map(|g| {
                format!(
                    r#"
## Project Guidance

The following project-specific guidance should inform your subtask breakdown:

{}

"#,
                    g
                )
            })
            .unwrap_or_default();

        format!(
            r#"You are breaking down a development task into smaller, manageable subtasks.
{}
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
    "dependencies": []  // 1-indexed subtask refs: ["1", "2"]. NEVER use "0". External deps: ["TASK-123"]
  }}
]

Guidelines:
- Start with foundational work (models, schemas)
- Then build core logic
- Then add UI/API layers
- Finally add tests and documentation
- Each subtask should be independently completable
- Use 1-indexed dependencies (e.g., ["1"] = first subtask). "0" is INVALID.
- Dependency values MUST be strings, not numbers
- Aim for {} subtasks total (can vary by 1-2 if needed for logical breakdown)
- DO NOT include "complexity" field - subtasks are all assumed to be small and manageable

Return ONLY the JSON array, no additional explanation."#,
            guidance_section,
            complexity,
            task_title,
            task_description,
            context,
            recommended_subtasks,
            recommended_subtasks
        )
    }

    pub fn reanalyze_dependencies(task_context: &str, phases: &[String]) -> String {
        format!(
            r#"You are analyzing a software project's task dependencies across multiple phases.

## Current Task State

{task_context}

## Your Task

Review the tasks above and suggest dependency changes that would improve execution order. Consider:

1. **Logical ordering**: Tasks that produce artifacts another task needs
2. **Current completion state**: Don't add deps on PENDING tasks for DONE tasks
3. **Cross-phase dependencies**: Tasks in one phase that should wait for tasks in another
4. **Remove redundant deps**: If A depends on B, and B depends on C, A doesn't also need C
5. **Missing dependencies**: If a task clearly requires output from another task, add the dependency

## Rules

- Use full task IDs with phase prefix (e.g., "auth:1", "api:3")
- Task IDs are 1-indexed. NEVER suggest dependencies on task "0" or any ID ending in ":0"
- Valid examples: "auth:1", "api:3", "main:10" - Invalid: "auth:0", "0"
- Only suggest changes for tasks that are PENDING or IN PROGRESS
- Don't modify DONE, EXPANDED, or SKIPPED tasks
- Consider that some tasks may intentionally have no dependencies
- Be conservative - only suggest changes that are clearly needed

## Response Format

Return a JSON array of suggestions:
```json
[
  {{
    "task_id": "api:3",
    "add_dependencies": ["auth:1", "core:2"],
    "remove_dependencies": [],
    "reasoning": "API endpoints need authentication service and core models"
  }}
]
```

Return empty array [] if no changes are needed.

Phases to analyze: {phases:?}
"#,
            task_context = task_context,
            phases = phases
        )
    }

    pub fn validate_tasks_against_prd(prd_content: &str, tasks_json: &str) -> String {
        format!(
            r#"You are a QA engineer validating that extracted tasks accurately reflect the original PRD.

## Original PRD/Requirements Document

{prd_content}

## Current Tasks (JSON)

{tasks_json}

## Your Task

Compare the tasks against the PRD and identify:

1. **Missing Requirements**: Features or requirements in the PRD that have NO corresponding task
2. **Incomplete Coverage**: Requirements that are only partially covered by existing tasks
3. **Misaligned Tasks**: Tasks that don't accurately reflect what the PRD specifies
4. **Extra Tasks**: Tasks that go beyond what the PRD requires (not necessarily bad, but note them)
5. **Dependency Issues**: Tasks that should logically depend on others based on PRD context
6. **Agent Type Issues**: Tasks with missing or incorrect agent_type assignments

## Analysis Guidelines

- Be thorough - check every requirement in the PRD has corresponding task(s)
- Consider implicit requirements (e.g., if PRD mentions "user authentication", tasks should cover login, logout, session management, etc.)
- Check that task descriptions accurately capture the PRD's intent
- Verify task priorities align with PRD emphasis
- Look for edge cases mentioned in PRD but missing from tasks
- Verify every task has an appropriate agent_type assigned based on task nature AND complexity:
  * "fast-builder" = Simple implementation tasks with complexity 0-2 (quick code changes, config updates, simple features)
  * "builder" = Complex implementation tasks with complexity 3+ (multi-file changes, new features, integrations)
  * "reviewer" = Code review, quality checks, refactoring tasks
  * "planner" = Design, architecture planning, research tasks
  * "tester" = Test writing, test automation, validation tasks
- IMPORTANT: Check that complexity matches agent_type - complexity 0-2 should use fast-builder, complexity 3+ should use builder

## Response Format

Return a JSON object:
```json
{{
  "coverage_score": <0-100>,
  "missing_requirements": [
    {{
      "requirement": "Description of missing requirement from PRD",
      "prd_section": "Where in PRD this appears",
      "suggested_task": "Brief description of task that should be added"
    }}
  ],
  "incomplete_coverage": [
    {{
      "requirement": "Description of partially covered requirement",
      "existing_tasks": ["task_id1", "task_id2"],
      "gap": "What aspect is missing"
    }}
  ],
  "misaligned_tasks": [
    {{
      "task_id": "ID of misaligned task",
      "issue": "How the task doesn't match PRD",
      "suggestion": "How to fix"
    }}
  ],
  "extra_tasks": [
    {{
      "task_id": "ID of extra task",
      "note": "Why this may be beyond PRD scope"
    }}
  ],
  "dependency_suggestions": [
    {{
      "task_id": "ID of task",
      "should_depend_on": ["task_id1"],
      "reasoning": "Why based on PRD context"
    }}
  ],
  "agent_type_issues": [
    {{
      "task_id": "ID of task with wrong or missing agent_type",
      "current_agent_type": null,
      "suggested_agent_type": "fast-builder|builder|reviewer|planner|tester",
      "reasoning": "Why this agent type is more appropriate (consider complexity: 0-2 = fast-builder, 3+ = builder)"
    }}
  ],
  "summary": "Brief overall assessment"
}}
```

If tasks perfectly cover the PRD, return:
```json
{{
  "coverage_score": 100,
  "missing_requirements": [],
  "incomplete_coverage": [],
  "misaligned_tasks": [],
  "extra_tasks": [],
  "dependency_suggestions": [],
  "agent_type_issues": [],
  "summary": "Tasks fully cover all PRD requirements"
}}
```

Return ONLY the JSON object, no additional explanation."#,
            prd_content = prd_content,
            tasks_json = tasks_json
        )
    }

    pub fn fix_prd_issues(
        prd_content: &str,
        tasks_json: &str,
        validation: &crate::commands::check_deps::PrdValidationResult,
    ) -> String {
        let validation_json = serde_json::to_string_pretty(validation).unwrap_or_default();

        format!(
            r#"You are a task management expert fixing tasks to better align with the PRD.

## Original PRD/Requirements Document

{prd_content}

## Current Tasks (JSON)

{tasks_json}

## Validation Results (Issues Found)

{validation_json}

## Your Task

Generate fixes for the issues identified in the validation. Focus on:

1. **Misaligned Tasks**: Update task titles and descriptions to match PRD intent
2. **Dependency Issues**: Add or remove dependencies based on logical ordering
3. **Incomplete Coverage**: Update task descriptions to cover gaps
4. **Agent Type Issues**: Assign or correct agent_type for each task

## Rules

- Only fix issues that can be resolved by updating existing tasks
- Do NOT suggest adding new tasks (that's a separate operation)
- Be precise - update only what needs changing
- Use full task IDs with phase prefix (e.g., "swfix:1", "auth:3")
- Keep task titles concise and action-oriented
- Keep descriptions to 2-3 sentences

## Response Format

Return a JSON array of fixes:
```json
[
  {{
    "action": "update_task",
    "task_id": "swfix:1",
    "new_title": "Updated title matching PRD",
    "new_description": "Updated description that better reflects PRD requirements",
    "reasoning": "Why this change aligns with PRD"
  }},
  {{
    "action": "update_dependency",
    "task_id": "swfix:3",
    "add_dependencies": ["swfix:1"],
    "remove_dependencies": [],
    "reasoning": "Task 3 needs output from task 1 per PRD"
  }},
  {{
    "action": "update_agent_type",
    "task_id": "swfix:2",
    "new_agent_type": "builder",
    "reasoning": "Task involves implementation work, builder agent is most appropriate"
  }}
]
```

## Agent Type Guidelines

When assigning agent_type, consider BOTH task nature AND complexity:
- "fast-builder" = Simple implementation tasks with complexity 0-2 (quick code changes, config updates, simple features)
- "builder" = Complex implementation tasks with complexity 3+ (multi-file changes, new features, integrations)
- "reviewer" = Code review, quality checks, refactoring tasks
- "planner" = Design, architecture planning, research tasks
- "tester" = Test writing, test automation, validation tasks

CRITICAL RULES:
1. Every task MUST have an agent_type assigned. If a task has no agent_type (null), generate an update_agent_type fix.
2. For implementation tasks, use complexity to choose between fast-builder and builder:
   - Complexity 0, 1, or 2 → "fast-builder"
   - Complexity 3, 5, 8, 13, or 21 → "builder"
3. Subtasks (tasks with parent_id) typically have complexity 0 and should use "fast-builder" unless they are test or review tasks.

Return empty array [] if no automatic fixes are possible.

Return ONLY the JSON array, no additional explanation."#,
            prd_content = prd_content,
            tasks_json = tasks_json,
            validation_json = validation_json
        )
    }
}
