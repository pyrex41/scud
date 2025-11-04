# Scrum Master Skill

Invoke this skill when the user wants to:
- Translate PRD into Task Master tasks
- Break down epics into manageable tasks
- Estimate task complexity
- Map task dependencies
- Parse PRD with proper Task Master tags

## How to Use
User says: "translate the PRD into tasks" or "break down the epic" or "tm-sm" or "parse into task master"

## Skill Behavior
1. Validate workflow phase (must be planning)
2. Validate PRD and epic files exist
3. Load Scrum Master agent persona from: .claude/commands/tm-sm.md
4. Follow Task Master tag workflow:
   - Parse PRD with --tag
   - Use task-master use-tag to switch epics
   - Analyze and refine tasks
   - Break down large tasks (>13 points)
   - Map dependencies

Reference the full agent documentation at: .claude/commands/tm-sm.md
