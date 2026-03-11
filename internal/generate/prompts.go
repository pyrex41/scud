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

` + "```json" + `
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
` + "```" + `

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
