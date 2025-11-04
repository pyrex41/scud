# Workflow: Plan and Build an Epic with Task-Master

This document outlines the end-to-end workflow for taking a product idea from conception to completion using Task-Master as the central state management system, orchestrated by Claude agents.

**Goal:** Complete all tasks for a single epic.
**Source of Truth:** `.taskmaster/tasks/tasks.json`

---

### Phase 1: Product Definition & Planning

**Agent:** `/tm-pm` (Product Manager)
**Goal:** Create a PRD and populate Task-Master with an initial set of tasks for a new epic.

**Step 1.1: Create the PRD**
Interact with the PM agent to define the product. The agent will produce `prd.md` and break it down into epic files like `epic-1-authentication.md`.

**Step 1.2: Parse Epic into Task-Master**
Execute the command provided by the PM to create the epic tag and its tasks.
```bash
# Example for a single epic
task-master parse-prd epic-1-authentication.md --tag=epic-1-authentication
