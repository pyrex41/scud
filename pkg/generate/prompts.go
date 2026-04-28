package generate

import "fmt"

// ParsePRDPrompt returns the prompt for converting a PRD to tasks.
func ParsePRDPrompt(content string, numTasks int, guidance string) string {
	guidanceSection := ""
	if guidance != "" {
		guidanceSection = fmt.Sprintf(`

## Project Guidance

%s`, guidance)
	}

	return fmt.Sprintf(`You are a senior technical project manager breaking down a Product Requirements Document (PRD) into actionable development tasks.

## Instructions

Analyze the following PRD and create approximately %d development tasks. Each task should be:
- Atomic and independently implementable
- Clearly scoped with a specific deliverable
- Properly ordered by dependencies (foundational work first)
- Assigned appropriate complexity using the Fibonacci scale

## Complexity Scale (Fibonacci)
- 1: Trivial change (config update, typo fix)
- 2: Simple change (single file, straightforward logic)
- 3: Moderate (few files, some design decisions)
- 5: Complex (multiple files, integration work)
- 8: Very complex (cross-cutting concerns, significant design)
- 13: Major feature (many files, architectural decisions)
- 21: Epic-level (should probably be broken down further)

## Agent Types
- "fast-builder": For complexity 1-2 tasks (quick changes, config updates, simple implementations)
- "builder": For complexity 3+ tasks (multi-file changes, integrations, complex logic)
- "reviewer": For code review and refactoring tasks
- "planner": For design, architecture, and research tasks
- "tester": For test automation and validation tasks

## Output Format

Return a JSON array of task objects:

`+"```json"+`
[
  {
    "title": "Action-oriented task title",
    "description": "2-3 sentences describing what needs to be done",
    "priority": "high",
    "complexity": 5,
    "dependencies": ["1", "2"],
    "agent_type": "builder"
  }
]
`+"```"+`

## Rules
- Dependencies are 1-indexed (first task = "1", NEVER "0")
- Dependencies must reference task indices as strings
- Start with foundational tasks (models, schemas, config)
- Then core logic and business rules
- Then UI/API layers
- Finally tests and documentation
- Keep task titles concise and action-oriented (start with a verb)
- Cross-phase dependencies use format "phase:id" (e.g., "auth:3")%s

## PRD Content

%s`, numTasks, guidanceSection, content)
}

// ValidateTasksAgainstPRD returns the prompt for comparing PRD requirements against tasks.
func ValidateTasksAgainstPRD(prdContent, tasksJSON string) string {
	return fmt.Sprintf(`You are a senior technical reviewer validating that a set of development tasks fully covers a Product Requirements Document (PRD).

## Instructions

Compare the PRD requirements against the existing tasks and identify:
1. Missing requirements not covered by any task
2. Requirements only partially covered
3. Tasks that don't align with any PRD requirement

## Output Format

Return a JSON object:

`+"```json"+`
{
  "coverage_score": 85,
  "missing_requirements": ["Requirement X is not addressed by any task"],
  "incomplete_coverage": ["Requirement Y is partially covered but missing Z aspect"],
  "misaligned_tasks": ["Task N does not map to any PRD requirement"],
  "summary": "Overall assessment of coverage"
}
`+"```"+`

## PRD Content

%s

## Current Tasks (JSON)

%s`, prdContent, tasksJSON)
}

// FixPRDIssuesPrompt returns the prompt for fixing PRD coverage issues.
func FixPRDIssuesPrompt(prdContent, tasksJSON, issuesJSON string) string {
	return fmt.Sprintf(`You are a senior technical project manager fixing coverage gaps between a PRD and development tasks.

## Instructions

Given the PRD, current tasks, and identified issues, provide fixes:
1. Update existing task titles/descriptions to better cover requirements
2. Add new tasks for missing requirements

## Output Format

Return a JSON object:

`+"```json"+`
{
  "update_tasks": [
    {"id": "3", "title": "Updated title", "description": "Updated description"}
  ],
  "add_tasks": [
    {
      "title": "New task title",
      "description": "What needs to be done",
      "priority": "high",
      "complexity": 5,
      "dependencies": ["1", "2"],
      "agent_type": "builder"
    }
  ]
}
`+"```"+`

## Rules
- Only update tasks that need changes
- New task dependencies must reference existing task IDs
- Keep changes minimal and targeted

## PRD Content

%s

## Current Tasks (JSON)

%s

## Identified Issues

%s`, prdContent, tasksJSON, issuesJSON)
}

// AnalyzeComplexityPrompt returns the prompt for analyzing task complexity.
func AnalyzeComplexityPrompt(title, description, details string) string {
	detailsSection := ""
	if details != "" {
		detailsSection = fmt.Sprintf(`

## Additional Details
%s`, details)
	}

	return fmt.Sprintf(`You are a senior developer analyzing the complexity of a development task.

## Task
- Title: %s
- Description: %s%s

## Complexity Scale (Fibonacci)
- 1: Trivial change (config update, typo fix)
- 2: Simple change (single file, straightforward logic)
- 3: Moderate (few files, some design decisions)
- 5: Complex (multiple files, integration work)
- 8: Very complex (cross-cutting concerns, significant design)
- 13: Major feature (many files, architectural decisions)
- 21: Epic-level (should probably be broken down further)

## Output Format

Return a JSON object:

`+"```json"+`
{
  "complexity": 5,
  "reasoning": "Brief explanation of why this complexity was chosen"
}
`+"```"+``, title, description, detailsSection)
}

// ReanalyzeDependenciesPrompt returns the prompt for cross-phase dependency analysis.
func ReanalyzeDependenciesPrompt(tasksJSON string) string {
	return fmt.Sprintf(`You are a senior technical architect analyzing task dependencies for correctness and completeness.

## Instructions

Review all tasks and their dependencies. Identify:
1. Missing dependencies (task B should depend on task A but doesn't)
2. Unnecessary dependencies (task B depends on A but doesn't actually need it)
3. Cross-phase dependencies that should be added

## Output Format

Return a JSON array of dependency suggestions:

`+"```json"+`
[
  {
    "task_id": "5",
    "add_deps": ["3", "4"],
    "remove_deps": ["1"],
    "reason": "Task 5 uses output from tasks 3 and 4, but not task 1"
  }
]
`+"```"+`

## Rules
- Only suggest changes that improve correctness
- Do not suggest circular dependencies
- Keep the DAG valid

## Current Tasks (JSON)

%s`, tasksJSON)
}

// GeneratePipelinePrompt returns the prompt for generating an attractor pipeline from a PRD.
func GeneratePipelinePrompt(prdContent string) string {
	return fmt.Sprintf(`You are a workflow architect converting a PRD into an attractor pipeline in DOT format.

## Instructions

Analyze the PRD and create a directed graph (DOT format) representing the execution pipeline.
Each node is a step in the pipeline. Edges represent execution order.

## Node Types (set via shape attribute)
- "box": Standard execution step (runs an AI agent)
- "diamond": Conditional/decision step
- "ellipse": Start/end node
- "parallelogram": Human review gate

## Output Format

Return a JSON object with a "dot" field containing the DOT graph:

`+"```json"+`
{
  "dot": "digraph pipeline {\n  rankdir=LR;\n\n  start [shape=ellipse label=\"Start\"];\n  analyze [shape=box label=\"Analyze Requirements\"];\n  implement [shape=box label=\"Implement Core\"];\n  review [shape=parallelogram label=\"Human Review\"];\n  test [shape=box label=\"Run Tests\"];\n  done [shape=ellipse label=\"Done\"];\n\n  start -> analyze;\n  analyze -> implement;\n  implement -> review;\n  review -> test;\n  test -> done;\n}"
}
`+"```"+`

## Rules
- Always start with a "start" node and end with a "done" node
- Use descriptive labels for each step
- Keep the pipeline linear unless parallelism is genuinely needed
- Include review gates for important milestones

## PRD Content

%s`, prdContent)
}

// ExpandTaskPrompt returns the prompt for expanding a complex task into subtasks.
func ExpandTaskPrompt(title, description string, complexity, recommendedSubs int, details, guidance string) string {
	detailsSection := ""
	if details != "" {
		detailsSection = fmt.Sprintf(`

## Existing Details
%s`, details)
	}

	guidanceSection := ""
	if guidance != "" {
		guidanceSection = fmt.Sprintf(`

## Project Guidance
%s`, guidance)
	}

	return fmt.Sprintf(`You are breaking down a complex development task into smaller, implementable subtasks.

## Parent Task
- Title: %s
- Description: %s
- Complexity: %d%s%s

## Instructions

Break this task into approximately %d subtasks that together fully implement the parent task.

## Output Format

Return a JSON array of subtask objects:

`+"```json"+`
[
  {
    "title": "Subtask title",
    "description": "What needs to be done",
    "priority": "high",
    "dependencies": []
  }
]
`+"```"+`

## Rules
- Dependencies are 1-indexed (first subtask = "1", NEVER "0")
- Dependencies must be strings, not numbers
- Start with foundational work (models, schemas)
- Then core logic
- Then API/UI layers
- Finally tests and documentation
- Each subtask should be independently implementable
- Do NOT include a complexity field (subtasks inherit from parent)`, title, description, complexity, detailsSection, guidanceSection, recommendedSubs)
}
