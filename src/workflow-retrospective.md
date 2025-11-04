---

### 🔬 `workflow-retrospective.md`

```markdown
# Workflow: Epic Retrospective with Task-Master

**Goal:** Use the final state of `.taskmaster/tasks/tasks.json` to analyze a completed epic, identify learnings, and generate a retrospective report.

**Prerequisite:** The epic must be 100% complete.

---

### Phase 1: Data Analysis with `jq`

Run these commands to extract key insights from the completed epic's data. Replace `epic-1-authentication` with your epic tag.

**Analysis 1: Complexity Hotspots**
Find the tasks with the highest complexity scores. These are areas where the most effort was concentrated.```bash
echo "🔥 Complexity Hotspots:"
jq '.["epic-1-authentication"].tasks | sort_by(-.complexity) | .[:5] | .[] | {id, title, complexity}' .taskmaster/tasks/tasks.json
