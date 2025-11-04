---

### 📘 `INTEGRATION-GUIDE.md`

```markdown
# Integration Guide: BMAD Agents + Task-Master

This guide explains how to use a hybrid system combining the intelligence of BMAD-style agents with the structured state management of Task-Master.

### Core Philosophy
*   **Task-Master is the Single Source of Truth:** All task statuses, dependencies, and details live in `.taskmaster/tasks/tasks.json`. There are no other state files.
*   **Agents are Stateless Workers:** Agents (like PM, Architect, Dev) are invoked to perform a specific job. They read from Task-Master and/or write back to it, but they don't maintain their own state between runs.
*   **Workflows are Orchestrators:** Workflows are sequences of commands that call agents and Task-Master in the correct order.

---

### Components

#### 1. Agents (`.claude/agents/`)
*   `/tm-pm` (Product Manager): Creates PRDs and parses them into Task-Master epics.
*   `/tm-architect` (Architect): Designs the technical solution and updates the Task-Master plan with new tasks and details.
*   `/tm-sm` (Scrum Master): Writes detailed story files for complex tasks.
*   `/tm-dev` (Developer): Implements code for a single task.

#### 2. Workflows (`.claude/workflows/` or as `.md` guides)
*   **`workflow-plan-and-build.md`**: The main, end-to-end process for taking an epic from idea to completion. This is your primary playbook.
*   **`workflow-retrospective.md`**: A post-project analysis workflow that uses data from Task-Master to generate insights.

#### 3. State (`.taskmaster/`)
*   **`tasks/tasks.json`**: The heart of the system. This single file contains all epics, tasks, dependencies, and metadata.

---

### Quick Start Guide

**Step 1: Setup**
1.  Install Task-Master.
2.  Create a `.claude/agents/` directory in your project.
3.  Save `tm-pm.md`, `tm-architect.md`, `tm-sm.md`, and `tm-dev.md` into that directory.

**Step 2: Run the Main Workflow**
Follow the steps outlined in `workflow-plan-and-build.md`.

1.  **Define (`/tm-pm`):**
    > `/tm-pm` -> "Help me build a new feature..."
    ```bash
    task-master parse-prd epic-1-new-feature.md --tag=epic-1-new-feature
    ```

2.  **Architect (`/tm-architect`):**
    > `/tm-architect` -> "Design the system for `epic-1-new-feature`."
    *(This agent will run Task-Master commands for you)*

3.  **Create Stories (`/tm-sm`):**
    ```bash
    task-master analyze-complexity --tag=epic-1-new-feature
    ```
    > `/tm-sm` -> "Create stories for complex tasks in `epic-1-new-feature`."

4.  **Build (`dev-loop.sh`):**
    Save the script from `workflow-plan-and-build.md` and run it.
    ```bash
    bash dev-loop.sh epic-1-new-feature
    ```

**Step 3: Track Progress**
At any point, check the status of your epic:
```bash
task-master progress --tag=epic-1-new-feature
