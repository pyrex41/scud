# SCUD View - Static HTML Viewer Implementation Plan

## Overview

Add a `scud view` command that generates a self-contained static HTML file with all task data embedded, then opens it in the default browser. No server required - just pure HTML/CSS/JS with mermaid.js from CDN.

## Current State Analysis

### Existing Infrastructure:
- `scud mermaid` command already generates mermaid flowchart diagrams (`scud-cli/src/commands/mermaid.rs`)
- Task data stored in `.scud/tasks/tasks.scg` with JSON mirror at `.scud/tasks/tasks.json`
- Node.js wrapper (`bin/scud.js`) handles some commands directly, delegates others to Rust CLI
- `open` package already available (used by tm-view, standard npm package)

### Key Data Structures:
- **Phase**: `{ name: string, tasks: Task[] }`
- **Task**: `{ id, title, description, status, complexity, priority, dependencies, parent_id, subtasks, details, test_strategy, assigned_to, locked_by, locked_at }`
- **Statuses**: Pending, InProgress, Done, Review, Blocked, Deferred, Cancelled, Expanded

## Desired End State

Running `scud view` will:
1. Read task data from `.scud/tasks/tasks.json`
2. Generate mermaid diagram via `scud mermaid` command
3. Generate a self-contained HTML file with:
   - All task data embedded as JSON
   - CSS for styling and view switching
   - Minimal JS for tab navigation and mermaid rendering
   - Views: Task List (hierarchical), Task Details, Mermaid Diagram, Stats
4. Write HTML to temp file (e.g., `/tmp/scud-view-{timestamp}.html`)
5. Open in default browser

### Verification:
- `scud view` opens browser with task viewer
- All phases/tags visible and selectable
- Tasks show hierarchy (parent → subtasks)
- Mermaid diagram renders correctly
- Stats show counts by status

## What We're NOT Doing

- No live updates / file watching
- No server process
- No interactivity (no status changes from UI)
- No separate npm package
- No Rust implementation (Node.js only for simplicity)

## Implementation Approach

Implement entirely in Node.js within `bin/scud.js` (or a new `bin/view.js` helper). Generate HTML string with embedded data, write to temp file, open browser.

---

## Phase 1: Create View Command Handler

### Overview
Add the `scud view` command to the Node.js wrapper that generates and opens the static HTML viewer.

### Changes Required:

#### 1.1 Add view command to bin/scud.js

**File**: `bin/scud.js`
**Changes**: Add 'view' to Node-handled commands, implement view generation

After line 196 (after the existing Node-only commands), add view command handler:

```javascript
// Around line 175, add 'view' to the switch or if-else chain for Node-only commands

case 'view':
  await runView();
  break;
```

Add the implementation function:

```javascript
import { execSync } from 'child_process';
import { tmpdir } from 'os';
import open from 'open';

async function runView() {
  const scudDir = path.join(process.cwd(), '.scud');
  const tasksJsonPath = path.join(scudDir, 'tasks', 'tasks.json');

  if (!fs.existsSync(scudDir)) {
    console.error(chalk.red('Error: No .scud directory found. Run: scud init'));
    process.exit(1);
  }

  if (!fs.existsSync(tasksJsonPath)) {
    console.error(chalk.red('Error: No tasks found. Run: scud parse-prd <prd-file>'));
    process.exit(1);
  }

  // Load task data
  const tasksData = JSON.parse(fs.readFileSync(tasksJsonPath, 'utf8'));

  // Generate mermaid diagram
  let mermaidDiagram = '';
  try {
    mermaidDiagram = execSync('scud mermaid --all-tags', {
      encoding: 'utf8',
      cwd: process.cwd()
    });
  } catch (e) {
    mermaidDiagram = '```mermaid\ngraph TD\n  A[No diagram available]\n```';
  }

  // Generate HTML
  const html = generateViewerHtml(tasksData, mermaidDiagram);

  // Write to temp file
  const tempFile = path.join(tmpdir(), `scud-view-${Date.now()}.html`);
  fs.writeFileSync(tempFile, html);

  console.log(chalk.green('Opening SCUD viewer...'));
  await open(tempFile);
}
```

#### 1.2 Create HTML generator function

**File**: `bin/scud.js` (or separate `bin/view-template.js`)
**Changes**: Add function to generate complete HTML with embedded data

```javascript
function generateViewerHtml(tasksData, mermaidDiagram) {
  // Extract mermaid content (remove ```mermaid and ``` markers)
  const mermaidContent = mermaidDiagram
    .replace(/```mermaid\n?/, '')
    .replace(/\n?```$/, '')
    .trim();

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>SCUD Task Viewer</title>
  <script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script>
  <style>
${getViewerStyles()}
  </style>
</head>
<body>
  <header>
    <h1>SCUD Task Viewer</h1>
    <nav>
      <button class="tab-btn active" data-tab="tasks">Tasks</button>
      <button class="tab-btn" data-tab="diagram">Diagram</button>
      <button class="tab-btn" data-tab="stats">Stats</button>
    </nav>
  </header>

  <main>
    <section id="tasks" class="tab-content active">
      <div class="phase-selector">
        <label>Phase: </label>
        <select id="phase-select"></select>
      </div>
      <div id="task-list"></div>
    </section>

    <section id="diagram" class="tab-content">
      <div class="mermaid">
${mermaidContent}
      </div>
    </section>

    <section id="stats" class="tab-content">
      <div id="stats-content"></div>
    </section>
  </main>

  <script>
    const TASKS_DATA = ${JSON.stringify(tasksData, null, 2)};
${getViewerScript()}
  </script>
</body>
</html>`;
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `scud view` command runs without error
- [ ] HTML file is created in temp directory
- [ ] HTML file contains valid mermaid syntax
- [ ] No Node.js runtime errors

#### Manual Verification:
- [ ] Browser opens automatically with the viewer
- [ ] Page renders without console errors

---

## Phase 2: Implement Viewer Styles

### Overview
Create CSS for the viewer with a clean, dark theme matching SCUD's terminal aesthetic.

### Changes Required:

#### 2.1 CSS styles function

**File**: `bin/scud.js`
**Changes**: Add getViewerStyles() function

```javascript
function getViewerStyles() {
  return `
    * { box-sizing: border-box; margin: 0; padding: 0; }

    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, monospace;
      background: #1a1a2e;
      color: #eee;
      line-height: 1.6;
    }

    header {
      background: #16213e;
      padding: 1rem 2rem;
      border-bottom: 1px solid #0f3460;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }

    h1 { font-size: 1.5rem; color: #e94560; }

    nav { display: flex; gap: 0.5rem; }

    .tab-btn {
      background: transparent;
      border: 1px solid #0f3460;
      color: #eee;
      padding: 0.5rem 1rem;
      cursor: pointer;
      border-radius: 4px;
      transition: all 0.2s;
    }

    .tab-btn:hover { background: #0f3460; }
    .tab-btn.active { background: #e94560; border-color: #e94560; }

    main { padding: 2rem; max-width: 1400px; margin: 0 auto; }

    .tab-content { display: none; }
    .tab-content.active { display: block; }

    .phase-selector {
      margin-bottom: 1.5rem;
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }

    .phase-selector select {
      background: #16213e;
      color: #eee;
      border: 1px solid #0f3460;
      padding: 0.5rem;
      border-radius: 4px;
      font-size: 1rem;
    }

    /* Task list styles */
    .task-item {
      background: #16213e;
      border: 1px solid #0f3460;
      border-radius: 8px;
      padding: 1rem;
      margin-bottom: 0.75rem;
      transition: border-color 0.2s;
    }

    .task-item:hover { border-color: #e94560; }

    .task-header {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      margin-bottom: 0.5rem;
    }

    .task-id {
      font-family: monospace;
      color: #00d9ff;
      font-weight: bold;
    }

    .task-title { font-weight: 500; margin-left: 0.75rem; flex: 1; }

    .task-meta {
      display: flex;
      gap: 0.5rem;
      font-size: 0.85rem;
    }

    .badge {
      padding: 0.2rem 0.5rem;
      border-radius: 4px;
      font-size: 0.75rem;
      text-transform: uppercase;
    }

    .status-pending { background: #374151; color: #9ca3af; }
    .status-inprogress, .status-in-progress { background: #1e40af; color: #93c5fd; }
    .status-done { background: #166534; color: #86efac; }
    .status-blocked { background: #991b1b; color: #fca5a5; }
    .status-review { background: #92400e; color: #fcd34d; }
    .status-expanded { background: #6b21a8; color: #d8b4fe; }
    .status-deferred { background: #374151; color: #9ca3af; }
    .status-cancelled { background: #1f2937; color: #6b7280; text-decoration: line-through; }

    .priority-high { border-left: 3px solid #ef4444; }
    .priority-medium { border-left: 3px solid #f59e0b; }
    .priority-low { border-left: 3px solid #6b7280; }

    .complexity {
      background: #0f3460;
      color: #00d9ff;
      padding: 0.2rem 0.5rem;
      border-radius: 4px;
      font-family: monospace;
    }

    .task-description {
      color: #9ca3af;
      font-size: 0.9rem;
      margin-top: 0.5rem;
    }

    .task-details {
      margin-top: 0.75rem;
      padding-top: 0.75rem;
      border-top: 1px solid #0f3460;
      font-size: 0.85rem;
      color: #9ca3af;
    }

    .subtasks {
      margin-left: 1.5rem;
      margin-top: 0.5rem;
      border-left: 2px solid #0f3460;
      padding-left: 1rem;
    }

    .subtask-item {
      background: #1a1a2e;
      border: 1px solid #0f3460;
      border-radius: 4px;
      padding: 0.75rem;
      margin-bottom: 0.5rem;
    }

    /* Dependencies */
    .dependencies {
      font-size: 0.8rem;
      color: #6b7280;
      margin-top: 0.5rem;
    }

    .dependencies span {
      background: #0f3460;
      padding: 0.1rem 0.4rem;
      border-radius: 3px;
      margin-right: 0.25rem;
    }

    /* Mermaid diagram */
    .mermaid {
      background: #16213e;
      padding: 2rem;
      border-radius: 8px;
      overflow-x: auto;
    }

    /* Stats */
    .stats-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 1rem;
    }

    .stat-card {
      background: #16213e;
      border: 1px solid #0f3460;
      border-radius: 8px;
      padding: 1.5rem;
      text-align: center;
    }

    .stat-value {
      font-size: 2.5rem;
      font-weight: bold;
      color: #e94560;
    }

    .stat-label {
      color: #9ca3af;
      font-size: 0.9rem;
      margin-top: 0.5rem;
    }

    .stat-breakdown {
      margin-top: 1rem;
      text-align: left;
    }

    .stat-row {
      display: flex;
      justify-content: space-between;
      padding: 0.25rem 0;
      border-bottom: 1px solid #0f3460;
    }
  `;
}
```

### Success Criteria:

#### Automated Verification:
- [ ] CSS is valid (no syntax errors)
- [ ] HTML renders without style errors in browser

#### Manual Verification:
- [ ] Dark theme displays correctly
- [ ] Task cards are visually distinct
- [ ] Status badges have appropriate colors
- [ ] Responsive on different screen sizes

---

## Phase 3: Implement Viewer JavaScript

### Overview
Add JavaScript for tab navigation, task rendering, and stats calculation.

### Changes Required:

#### 3.1 JavaScript function

**File**: `bin/scud.js`
**Changes**: Add getViewerScript() function

```javascript
function getViewerScript() {
  return `
    // Initialize mermaid
    mermaid.initialize({
      startOnLoad: true,
      theme: 'dark',
      themeVariables: {
        primaryColor: '#e94560',
        primaryTextColor: '#eee',
        primaryBorderColor: '#0f3460',
        lineColor: '#0f3460',
        secondaryColor: '#16213e',
        tertiaryColor: '#1a1a2e'
      }
    });

    // Tab navigation
    document.querySelectorAll('.tab-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
        document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
        btn.classList.add('active');
        document.getElementById(btn.dataset.tab).classList.add('active');
      });
    });

    // Get phases from data
    function getPhases() {
      if (Array.isArray(TASKS_DATA)) {
        // Single phase array format
        return { 'default': TASKS_DATA };
      }
      return TASKS_DATA;
    }

    // Populate phase selector
    const phases = getPhases();
    const phaseSelect = document.getElementById('phase-select');
    Object.keys(phases).forEach(phase => {
      const option = document.createElement('option');
      option.value = phase;
      option.textContent = phase;
      phaseSelect.appendChild(option);
    });

    // Render tasks for selected phase
    function renderTasks(phaseName) {
      const tasks = phases[phaseName]?.tasks || phases[phaseName] || [];
      const container = document.getElementById('task-list');
      container.innerHTML = '';

      // Build task map for subtask lookup
      const taskMap = new Map();
      tasks.forEach(t => taskMap.set(t.id, t));

      // Get root tasks (no parent_id)
      const rootTasks = tasks.filter(t => !t.parent_id);

      rootTasks.forEach(task => {
        container.appendChild(createTaskElement(task, taskMap));
      });
    }

    function createTaskElement(task, taskMap) {
      const div = document.createElement('div');
      const priorityClass = 'priority-' + (task.priority || 'medium').toLowerCase();
      div.className = 'task-item ' + priorityClass;

      const statusClass = 'status-' + (task.status || 'pending').toLowerCase().replace(/[^a-z]/g, '-');

      let html = '<div class="task-header">';
      html += '<div><span class="task-id">' + escapeHtml(task.id) + '</span>';
      html += '<span class="task-title">' + escapeHtml(task.title) + '</span></div>';
      html += '<div class="task-meta">';
      html += '<span class="badge ' + statusClass + '">' + escapeHtml(task.status || 'pending') + '</span>';
      if (task.complexity) {
        html += '<span class="complexity">C:' + task.complexity + '</span>';
      }
      html += '</div></div>';

      if (task.description) {
        html += '<div class="task-description">' + escapeHtml(task.description) + '</div>';
      }

      if (task.dependencies && task.dependencies.length > 0) {
        html += '<div class="dependencies">Depends on: ';
        task.dependencies.forEach(dep => {
          html += '<span>' + escapeHtml(dep) + '</span>';
        });
        html += '</div>';
      }

      if (task.assigned_to) {
        html += '<div class="task-details">Assigned: ' + escapeHtml(task.assigned_to) + '</div>';
      }

      div.innerHTML = html;

      // Add subtasks
      if (task.subtasks && task.subtasks.length > 0) {
        const subtasksDiv = document.createElement('div');
        subtasksDiv.className = 'subtasks';
        task.subtasks.forEach(subtaskId => {
          const subtask = taskMap.get(subtaskId);
          if (subtask) {
            const subtaskEl = createTaskElement(subtask, taskMap);
            subtaskEl.className = 'subtask-item ' + priorityClass;
            subtasksDiv.appendChild(subtaskEl);
          }
        });
        div.appendChild(subtasksDiv);
      }

      return div;
    }

    function escapeHtml(text) {
      if (!text) return '';
      const div = document.createElement('div');
      div.textContent = text;
      return div.innerHTML;
    }

    // Render stats
    function renderStats() {
      const container = document.getElementById('stats-content');
      const allTasks = [];

      Object.values(phases).forEach(phase => {
        const tasks = phase.tasks || phase;
        if (Array.isArray(tasks)) {
          allTasks.push(...tasks);
        }
      });

      const byStatus = {};
      const byPriority = {};
      let totalComplexity = 0;

      allTasks.forEach(task => {
        const status = (task.status || 'pending').toLowerCase();
        const priority = (task.priority || 'medium').toLowerCase();
        byStatus[status] = (byStatus[status] || 0) + 1;
        byPriority[priority] = (byPriority[priority] || 0) + 1;
        if (task.complexity && task.status !== 'expanded') {
          totalComplexity += task.complexity;
        }
      });

      let html = '<div class="stats-grid">';

      html += '<div class="stat-card">';
      html += '<div class="stat-value">' + allTasks.length + '</div>';
      html += '<div class="stat-label">Total Tasks</div>';
      html += '</div>';

      html += '<div class="stat-card">';
      html += '<div class="stat-value">' + totalComplexity + '</div>';
      html += '<div class="stat-label">Total Complexity</div>';
      html += '</div>';

      html += '<div class="stat-card">';
      html += '<div class="stat-value">' + (byStatus['done'] || 0) + '</div>';
      html += '<div class="stat-label">Completed</div>';
      html += '</div>';

      html += '<div class="stat-card">';
      html += '<div class="stat-value">' + (byStatus['inprogress'] || byStatus['in-progress'] || 0) + '</div>';
      html += '<div class="stat-label">In Progress</div>';
      html += '</div>';

      html += '</div>';

      html += '<h3 style="margin: 2rem 0 1rem; color: #e94560;">By Status</h3>';
      html += '<div class="stat-card"><div class="stat-breakdown">';
      Object.entries(byStatus).sort((a, b) => b[1] - a[1]).forEach(([status, count]) => {
        html += '<div class="stat-row"><span>' + status + '</span><span>' + count + '</span></div>';
      });
      html += '</div></div>';

      html += '<h3 style="margin: 2rem 0 1rem; color: #e94560;">By Priority</h3>';
      html += '<div class="stat-card"><div class="stat-breakdown">';
      Object.entries(byPriority).sort((a, b) => b[1] - a[1]).forEach(([priority, count]) => {
        html += '<div class="stat-row"><span>' + priority + '</span><span>' + count + '</span></div>';
      });
      html += '</div></div>';

      container.innerHTML = html;
    }

    // Phase selector change
    phaseSelect.addEventListener('change', (e) => {
      renderTasks(e.target.value);
    });

    // Initial render
    const firstPhase = Object.keys(phases)[0];
    if (firstPhase) {
      renderTasks(firstPhase);
    }
    renderStats();
  `;
}
```

### Success Criteria:

#### Automated Verification:
- [ ] JavaScript is valid (no syntax errors)
- [ ] No console errors when page loads

#### Manual Verification:
- [ ] Tab switching works correctly
- [ ] Phase selector populates and switching works
- [ ] Tasks render with correct hierarchy (subtasks nested under parents)
- [ ] Mermaid diagram renders
- [ ] Stats calculate correctly

---

## Phase 4: Wire Up Command and Test

### Overview
Complete the integration and ensure the command works end-to-end.

### Changes Required:

#### 4.1 Update bin/scud.js command handling

**File**: `bin/scud.js`
**Changes**: Ensure 'view' command is properly routed

The view command should be handled in Node.js, not delegated to Rust. Update the command routing around line 150-196:

```javascript
// Add to imports at top of file
import { tmpdir } from 'os';
import open from 'open';

// In the command handling section, add view before the rustCommands check:
const command = args[0];

if (command === 'view') {
  await runView();
  process.exit(0);
}

// ... rest of existing command handling
```

#### 4.2 Add open package dependency

**File**: `package.json`
**Changes**: Add 'open' to dependencies if not already present

```json
{
  "dependencies": {
    "open": "^10.0.3"
  }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `scud view` runs without error from project root
- [ ] `scud view` shows error message when run outside SCUD project
- [ ] Generated HTML file exists in temp directory
- [ ] HTML file is valid (can be parsed)

#### Manual Verification:
- [ ] Browser opens automatically
- [ ] All three tabs (Tasks, Diagram, Stats) work
- [ ] Task hierarchy displays correctly
- [ ] Mermaid diagram renders with correct colors
- [ ] Stats show accurate counts

---

## Testing Strategy

### Unit Tests:
- None needed for this simple implementation

### Integration Tests:
- Test `scud view` command in a project with tasks
- Test error handling when no `.scud` directory exists
- Test with multi-phase task data

### Manual Testing Steps:
1. Navigate to a SCUD-initialized project
2. Run `scud view`
3. Verify browser opens with viewer
4. Click through all tabs
5. Switch phases in selector
6. Verify mermaid diagram matches `scud mermaid --all-tags` output
7. Verify stats match `scud stats` output

## Performance Considerations

- HTML generation is fast (string concatenation)
- Mermaid.js loaded from CDN (fast, cached)
- No ongoing process (file opened and done)
- Temp files can accumulate; consider cleanup strategy in future

## References

- tm-view implementation: `/Users/reuben/bmad-tm/tm-view.xml`
- Existing mermaid command: `scud-cli/src/commands/mermaid.rs`
- SCUD CLI wrapper: `bin/scud.js`
- Task data format: `.scud/tasks/tasks.json`
