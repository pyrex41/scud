#!/usr/bin/env node

/**
 * SCUD CLI
 * Main entry point for scud commands
 */

const { execSync, spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const { tmpdir } = require('os');

const command = process.argv[2];
const args = process.argv.slice(3);

// Task management commands (use Rust CLI)
const taskCommands = ['tags', 'use-tag', 'list', 'show', 'set-status', 'next', 'stats', 'mermaid', 'waves', 'doctor', 'convert', 'assign', 'whois', 'migrate', 'hooks', 'warmup', 'commit', 'next-batch', 'who-is', 'reanalyze-deps', 'clean'];

// AI-powered commands (use Rust CLI)
const aiCommands = ['parse', 'parse-prd', 'analyze-complexity', 'expand', 'research'];

const versionCommands = ['--version', '-V'];

// All commands handled by Rust CLI
const rustCommands = [...taskCommands, ...aiCommands, ...versionCommands];

const commands = {
  init: 'Initialize SCUD in current project',
  status: 'Show current workflow status',
  install: 'Install slash commands for Claude Code',
  validate: 'Run workflow validation',
  help: 'Show this help message',
  view: 'Open interactive task viewer in browser',
  // Task commands
  tags: 'List all epic tags',
  'use-tag': 'Switch to epic',
  list: 'List tasks in active epic',
  show: 'Show task details',
  'set-status': 'Update task status',
  next: 'Find next available task',
  stats: 'Show task statistics'
};

function showHelp() {
  console.log(`
╭─────────────────────────────╮
│                             │
│   SCUD Task Manager         │
│                             │
╰─────────────────────────────╯

Usage: scud <command> [options]

Setup Commands:
  init          Initialize SCUD in current project
  install       Install slash commands for Claude Code
  status        Show current workflow status
  validate      Run workflow validation
  view          Open interactive task viewer in browser

Task Management (built-in, fast):
  tags                        List all epic tags
  use-tag <tag>              Switch to epic
  list [--status=<status>]   List tasks in active epic
  show <id>                  Show task details
  set-status <id> <status>   Update task status
  next                       Find next available task
  stats                      Show task statistics

AI-Powered (built-in, requires ANTHROPIC_API_KEY):
  parse-prd <file> --tag=<tag>    Parse PRD into tasks
  analyze-complexity [--task=<id>] Analyze task complexity
  expand [--task=<id>] [--all]    Expand tasks (default: all in current tag)
  research "<query>"              AI research

Examples:
  scud init                       # Initialize in current directory
  scud view                       # Open task viewer in browser
  scud tags                       # List all epics
  scud use-tag epic-1-auth        # Switch to epic
  scud next                       # Find next available task
  scud set-status 3 in-progress   # Start task 3

  scud parse-prd epic.md --tag epic-1   # Parse PRD (AI)
  scud analyze-complexity               # Analyze all tasks (AI)
  scud expand                           # Expand tasks in current tag (AI)
  scud expand --all                     # Expand tasks in ALL tags (AI)
  scud expand --task auth:1             # Expand specific task (AI)
  scud research "OAuth best practices"  # Research topic (AI)

For more information, visit:
https://github.com/yourusername/scud
`);
}

function init() {
  const installScript = path.join(__dirname, '..', 'bin', 'install.js');
  const result = spawnSync('node', [installScript, 'init'], { stdio: 'inherit' });
  if (result.status !== 0) {
    console.error('Installation failed');
    process.exit(1);
  }
}

function install() {
  const installScript = path.join(__dirname, '..', 'bin', 'install.js');
  const result = spawnSync('node', [installScript, ...args], { stdio: 'inherit' });
  if (result.status !== 0) {
    console.error('Installation failed');
    process.exit(1);
  }
}

function status() {
  const validator = path.join(__dirname, '..', 'src', 'validators', 'scud-validator.js');
  const result = spawnSync('node', [validator, 'get-command-availability'], { encoding: 'utf8' });

  if (result.status !== 0) {
    console.error('Status check failed:', result.stderr);
    process.exit(1);
  }

  try {
    const availability = JSON.parse(result.stdout);

    console.log('\n📊 SCUD Workflow Status\n');
    console.log('Available Commands:');

    for (const [cmd, info] of Object.entries(availability)) {
      const icon = info.available ? '✅' : '❌';
      console.log(`  ${icon} /${cmd}`);
      console.log(`     ${info.reason}`);
    }
    console.log('');
  } catch (error) {
    console.error('Status check failed:', error.message);
    process.exit(1);
  }
}

function validate() {
  const validator = path.join(__dirname, '..', 'src', 'validators', 'scud-validator.js');
  const result = spawnSync('node', [validator, 'validate-cli'], { stdio: 'inherit' });

  if (result.status === 0) {
    console.log('✅ Validation passed');
  } else {
    console.error('❌ Validation failed');
    process.exit(1);
  }
}

// Handle view command in Node.js (before Rust delegation)
if (command === 'view') {
  runView().catch(error => {
    console.error('Error running view:', error.message);
    process.exit(1);
  });
  return;
}

// Check if this is a command handled by Rust CLI
if (rustCommands.includes(command)) {
  // Find the Rust binary - prefer system-installed (cargo) over local builds
  const homedir = require('os').homedir();
  const cargoBinary = path.join(homedir, '.cargo', 'bin', 'scud');
  const localRelease = path.join(__dirname, '..', 'scud-cli', 'target', 'release', 'scud');
  const localDebug = path.join(__dirname, '..', 'scud-cli', 'target', 'debug', 'scud');

  // Priority: cargo-installed > local release > local debug
  let scudBinary = null;
  if (fs.existsSync(cargoBinary)) {
    scudBinary = cargoBinary;
  } else if (fs.existsSync(localRelease)) {
    scudBinary = localRelease;
  } else if (fs.existsSync(localDebug)) {
    scudBinary = localDebug;
  }

  if (!scudBinary) {
    console.error('❌ SCUD Rust CLI not found.');
    console.error('   Install with: cargo install scud-cli');
    console.error('   Or build locally: cd scud-cli && cargo build --release');
    process.exit(1);
  }

  // Use spawnSync with argument array to properly handle spaces and special chars
  const result = spawnSync(scudBinary, [command, ...args], { stdio: 'inherit' });
  process.exit(result.status || 0);
}

switch (command) {
  case 'init':
    init();
    break;
  case 'install':
    install();
    break;
  case 'status':
    status();
    break;
  case 'validate':
    validate();
    break;
  case 'help':
  case undefined:
    showHelp();
    break;
  default:
    console.error(`Unknown command: ${command}`);
    console.log('Run "scud help" for usage information');
    process.exit(1);
}

/**
 * Run the view command - generate and open static HTML viewer
 */
async function runView() {
  const scudDir = path.join(process.cwd(), '.scud');
  const tasksJsonPath = path.join(scudDir, 'tasks', 'tasks.json');
  const tasksScgPath = path.join(scudDir, 'tasks', 'tasks.scg');

  if (!fs.existsSync(scudDir)) {
    console.error('❌ Error: No .scud directory found. Run: scud init');
    process.exit(1);
  }

  // Check for either JSON or SCG format
  const hasJson = fs.existsSync(tasksJsonPath);
  const hasScg = fs.existsSync(tasksScgPath);

  if (!hasJson && !hasScg) {
    console.error('❌ Error: No tasks found. Run: scud parse-prd <prd-file>');
    process.exit(1);
  }

  // Load task data - prefer SCG format as it's the current default
  let tasksData;
  if (hasScg) {
    const scgContent = fs.readFileSync(tasksScgPath, 'utf8');
    tasksData = parseScgFile(scgContent);
  } else {
    tasksData = JSON.parse(fs.readFileSync(tasksJsonPath, 'utf8'));
  }

  // Compute waves directly from task data (same algorithm as Rust CLI)
  const wavesData = {};
  const phases = Object.keys(tasksData);
  for (const phase of phases) {
    const phaseData = tasksData[phase];
    const tasks = phaseData?.tasks || [];
    wavesData[phase] = computeWaves(tasks);
  }

  // Generate HTML (diagrams are generated dynamically in JavaScript)
  const html = generateViewerHtml(tasksData, wavesData);

  // Write to temp file
  const tempFile = path.join(tmpdir(), `scud-view-${Date.now()}.html`);
  fs.writeFileSync(tempFile, html);

  console.log('✅ Opening SCUD viewer...');
  // Dynamic import for ESM-only open package
  const { default: open } = await import('open');
  await open(tempFile);
}

/**
 * Compute execution waves using Kahn's algorithm (topological sort with level assignment)
 * This is the same algorithm used in the Rust CLI (waves.rs)
 * Returns array of waves, each wave containing task info
 */
function computeWaves(tasks) {
  // Filter to actionable tasks only (not done, not expanded parents, not cancelled)
  const actionable = tasks.filter(task => {
    if (task.status === 'Done' || task.status === 'done' ||
        task.status === 'Expanded' || task.status === 'expanded' ||
        task.status === 'Cancelled' || task.status === 'cancelled') {
      return false;
    }

    // If it's a subtask, only include if parent is expanded
    if (task.parent_id) {
      const parent = tasks.find(t => t.id === task.parent_id);
      if (parent && (parent.status === 'Expanded' || parent.status === 'expanded')) {
        return true;
      }
      return false;
    }

    return true;
  });

  if (actionable.length === 0) {
    return [];
  }

  // Build set of actionable task IDs
  const taskIds = new Set(actionable.map(t => t.id));

  // Build in-degree map (how many dependencies does each task have within our set?)
  const inDegree = new Map();
  const dependents = new Map(); // task -> tasks that depend on it

  for (const task of actionable) {
    if (!inDegree.has(task.id)) {
      inDegree.set(task.id, 0);
    }

    for (const dep of (task.dependencies || [])) {
      // Only count dependencies that are in our actionable task set
      if (taskIds.has(dep)) {
        inDegree.set(task.id, (inDegree.get(task.id) || 0) + 1);
        if (!dependents.has(dep)) {
          dependents.set(dep, []);
        }
        dependents.get(dep).push(task.id);
      }
    }
  }

  // Kahn's algorithm with wave tracking
  const waves = [];
  const remaining = new Map(inDegree);
  let waveNumber = 1;

  while (remaining.size > 0) {
    // Find all tasks with no remaining dependencies (in-degree = 0)
    const ready = [];
    for (const [id, deg] of remaining) {
      if (deg === 0) {
        ready.push(id);
      }
    }

    if (ready.length === 0) {
      // Circular dependency detected - break out
      console.warn('Warning: Circular dependency detected in tasks');
      break;
    }

    // Remove ready tasks from remaining and update dependents
    for (const taskId of ready) {
      remaining.delete(taskId);

      const deps = dependents.get(taskId) || [];
      for (const depId of deps) {
        if (remaining.has(depId)) {
          remaining.set(depId, Math.max(0, remaining.get(depId) - 1));
        }
      }
    }

    // Create wave with task details
    const waveTasks = ready.map(taskId => {
      const task = actionable.find(t => t.id === taskId);
      return {
        id: taskId,
        title: task?.title || taskId,
        status: task?.status || 'pending',
        complexity: task?.complexity || 0,
        dependencies: task?.dependencies || []
      };
    });

    waves.push({
      wave: waveNumber,
      round: 1,
      tasks: waveTasks
    });

    waveNumber++;
  }

  return waves;
}

/**
 * Parse SCG (SCUD Graph) format file
 * Returns data in the same format as JSON: { phaseName: { tasks: [...] }, ... }
 */
function parseScgFile(content) {
  const result = {};

  // Split by "# SCUD Graph" header to separate phases
  const phaseBlocks = content.split(/(?=# SCUD Graph)/);

  for (const block of phaseBlocks) {
    if (!block.trim()) continue;

    const phase = parseScgPhase(block);
    if (phase && phase.name) {
      result[phase.name] = { tasks: phase.tasks };
    }
  }

  return result;
}

/**
 * Parse a single SCG phase block
 */
function parseScgPhase(content) {
  const lines = content.split('\n');

  // Find phase name
  let phaseName = null;
  for (const line of lines) {
    const match = line.match(/^# (?:Phase|Epic):\s*(.+)$/);
    if (match) {
      phaseName = match[1].trim();
      break;
    }
  }

  if (!phaseName) return null;

  const tasks = new Map();
  const edges = [];
  const parents = new Map();
  const details = new Map();
  const assignments = new Map();

  let currentSection = null;
  let currentDetailId = null;
  let currentDetailField = null;
  let currentDetailContent = [];

  // Status code mapping
  const statusCodes = {
    'P': 'pending',
    'I': 'in-progress',
    'D': 'done',
    'R': 'review',
    'B': 'blocked',
    'F': 'deferred',
    'C': 'cancelled',
    'X': 'expanded'
  };

  // Priority code mapping
  const priorityCodes = {
    'C': 'critical',
    'H': 'high',
    'M': 'medium',
    'L': 'low'
  };

  // Helper to split by pipe, respecting escapes
  function splitByPipe(line) {
    const parts = [];
    let current = '';
    let i = 0;
    while (i < line.length) {
      if (line[i] === '\\' && i + 1 < line.length) {
        current += line[i] + line[i + 1];
        i += 2;
      } else if (line[i] === '|') {
        parts.push(current.trim());
        current = '';
        i++;
      } else {
        current += line[i];
        i++;
      }
    }
    parts.push(current.trim());
    return parts;
  }

  // Helper to unescape text
  function unescape(text) {
    return text
      .replace(/\\n/g, '\n')
      .replace(/\\\|/g, '|')
      .replace(/\\\\/g, '\\');
  }

  // Helper to flush current detail
  function flushDetail() {
    if (currentDetailId && currentDetailField) {
      if (!details.has(currentDetailId)) {
        details.set(currentDetailId, {});
      }
      details.get(currentDetailId)[currentDetailField] = currentDetailContent.join('\n');
    }
    currentDetailId = null;
    currentDetailField = null;
    currentDetailContent = [];
  }

  for (const line of lines) {
    const trimmed = line.trim();

    // Skip empty lines and comments
    if (!trimmed || trimmed.startsWith('#')) continue;

    // Check for section headers
    if (trimmed.startsWith('@')) {
      flushDetail();
      if (trimmed === '@meta {' || trimmed === '@meta') {
        currentSection = 'meta';
      } else if (trimmed === '@nodes') {
        currentSection = 'nodes';
      } else if (trimmed === '@edges') {
        currentSection = 'edges';
      } else if (trimmed === '@parents') {
        currentSection = 'parents';
      } else if (trimmed === '@assignments') {
        currentSection = 'assignments';
      } else if (trimmed === '@details') {
        currentSection = 'details';
      }
      continue;
    }

    // Handle continuation lines in details
    if (currentSection === 'details' && line.startsWith('  ') && currentDetailId) {
      currentDetailContent.push(line.slice(2));
      continue;
    }

    // Skip meta closing brace
    if (trimmed === '}') continue;

    switch (currentSection) {
      case 'nodes': {
        // Parse "id | title | status | complexity | priority"
        const parts = splitByPipe(trimmed);
        if (parts.length >= 5) {
          const id = parts[0];
          const title = unescape(parts[1]);
          const status = statusCodes[parts[2].charAt(0)] || 'pending';
          const complexity = parseInt(parts[3], 10) || 0;
          const priority = priorityCodes[parts[4].charAt(0)] || 'medium';

          tasks.set(id, {
            id,
            title,
            description: '',
            status,
            complexity,
            priority,
            dependencies: [],
            subtasks: [],
            parent_id: null,
            assigned_to: null
          });
        }
        break;
      }

      case 'edges': {
        // Parse "dependent -> dependency"
        const match = trimmed.match(/^(.+?)\s*->\s*(.+)$/);
        if (match) {
          edges.push({ dependent: match[1].trim(), dependency: match[2].trim() });
        }
        break;
      }

      case 'parents': {
        // Parse "parent: child1, child2, ..."
        const match = trimmed.match(/^(.+?):\s*(.+)$/);
        if (match) {
          const parentId = match[1].trim();
          const childIds = match[2].split(',').map(s => s.trim()).filter(s => s);
          parents.set(parentId, childIds);
        }
        break;
      }

      case 'assignments': {
        // Parse "id | assigned_to"
        const parts = splitByPipe(trimmed);
        if (parts.length >= 2 && parts[1]) {
          assignments.set(parts[0], parts[1]);
        }
        break;
      }

      case 'details': {
        flushDetail();
        // Parse "id | field |"
        const parts = splitByPipe(trimmed);
        if (parts.length >= 2) {
          currentDetailId = parts[0];
          currentDetailField = parts[1];
          currentDetailContent = [];
        }
        break;
      }
    }
  }

  // Flush any remaining detail
  flushDetail();

  // Apply edges (dependencies)
  for (const edge of edges) {
    const task = tasks.get(edge.dependent);
    if (task) {
      task.dependencies.push(edge.dependency);
    }
  }

  // Apply parent-child relationships
  for (const [parentId, childIds] of parents) {
    const parent = tasks.get(parentId);
    if (parent) {
      parent.subtasks = childIds;
    }
    for (const childId of childIds) {
      const child = tasks.get(childId);
      if (child) {
        child.parent_id = parentId;
      }
    }
  }

  // Apply details
  for (const [id, fields] of details) {
    const task = tasks.get(id);
    if (task) {
      if (fields.description) task.description = fields.description;
      if (fields.details) task.details = fields.details;
      if (fields.test_strategy) task.test_strategy = fields.test_strategy;
    }
  }

  // Apply assignments
  for (const [id, assignee] of assignments) {
    const task = tasks.get(id);
    if (task) {
      task.assigned_to = assignee;
    }
  }

  // Convert to array and sort by ID naturally
  const taskArray = Array.from(tasks.values());
  taskArray.sort((a, b) => naturalSortIds(a.id, b.id));

  return { name: phaseName, tasks: taskArray };
}

/**
 * Natural sort for task IDs: "1" < "2" < "10", "1.1" < "1.2" < "1.10"
 */
function naturalSortIds(a, b) {
  const aParts = a.split('.');
  const bParts = b.split('.');

  const minLen = Math.min(aParts.length, bParts.length);
  for (let i = 0; i < minLen; i++) {
    const an = parseInt(aParts[i], 10);
    const bn = parseInt(bParts[i], 10);
    if (!isNaN(an) && !isNaN(bn)) {
      if (an !== bn) return an - bn;
    } else {
      if (aParts[i] !== bParts[i]) return aParts[i].localeCompare(bParts[i]);
    }
  }
  return aParts.length - bParts.length;
}

/**
 * Generate the complete HTML viewer
 */
function generateViewerHtml(tasksData, wavesData) {
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
      <button class="tab-btn" data-tab="waves">Waves</button>
      <button class="tab-btn" data-tab="diagram">Diagram</button>
      <button class="tab-btn" data-tab="stats">Stats</button>
    </nav>
  </header>

  <div class="layout">
    <main>
      <section id="tasks" class="tab-content active">
        <div class="phase-selector">
          <label>Phase: </label>
          <select id="phase-select"></select>
        </div>
        <div id="task-list"></div>
      </section>

      <section id="waves" class="tab-content">
        <div class="phase-selector">
          <label>Phase: </label>
          <select id="waves-phase-select"></select>
        </div>
        <div id="waves-list"></div>
      </section>

      <section id="diagram" class="tab-content">
        <div class="diagram-controls">
          <label class="checkbox-label">
            <input type="checkbox" id="simplified-view" checked> Simplified (hide subtasks)
          </label>
        </div>
        <div id="diagram-phases-container"></div>
      </section>

      <section id="stats" class="tab-content">
        <div id="stats-content"></div>
      </section>
    </main>

    <aside id="detail-panel" class="detail-panel hidden">
      <div class="detail-header">
        <h2 id="detail-title">Task Details</h2>
        <button id="close-detail" class="close-btn">&times;</button>
      </div>
      <div id="detail-content"></div>
    </aside>
  </div>

  <script>
    const TASKS_DATA = ${JSON.stringify(tasksData, null, 2)};
    const WAVES_DATA = ${JSON.stringify(wavesData, null, 2)};
${getViewerScript()}
  </script>
</body>
</html>`;
}

/**
 * CSS styles for the viewer
 */
function getViewerStyles() {
  return `
    * { box-sizing: border-box; margin: 0; padding: 0; }

    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      background: #0f172a;
      color: #e4e4e7;
      line-height: 1.6;
    }

    header {
      background: #1e293b;
      padding: 1rem 2rem;
      border-bottom: 1px solid #334155;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }

    h1 { font-size: 1.5rem; color: #3b82f6; }

    nav { display: flex; gap: 0.5rem; }

    .tab-btn {
      background: transparent;
      border: 1px solid #334155;
      color: #e4e4e7;
      padding: 0.5rem 1rem;
      cursor: pointer;
      border-radius: 4px;
      transition: all 0.2s;
    }

    .tab-btn:hover { background: #334155; }
    .tab-btn.active { background: #3b82f6; border-color: #3b82f6; }

    /* Layout with detail panel */
    .layout {
      display: flex;
      gap: 1rem;
    }

    main { padding: 2rem; flex: 1; max-width: 1000px; margin: 0 auto; }
    .layout.has-detail main { max-width: none; }

    .tab-content { display: none; }
    .tab-content.active { display: block; }

    /* Detail panel */
    .detail-panel {
      width: 400px;
      background: #1e293b;
      border-left: 1px solid #334155;
      padding: 1.5rem;
      height: calc(100vh - 60px);
      overflow-y: auto;
      position: sticky;
      top: 60px;
    }

    .detail-panel.hidden { display: none; }

    .detail-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 1rem;
      padding-bottom: 1rem;
      border-bottom: 1px solid #334155;
    }

    .detail-header h2 { font-size: 1.1rem; color: #3b82f6; }

    .close-btn {
      background: none;
      border: none;
      color: #9ca3af;
      font-size: 1.5rem;
      cursor: pointer;
      padding: 0 0.5rem;
    }

    .close-btn:hover { color: #60a5fa; }

    .detail-section {
      margin-bottom: 1.5rem;
    }

    .detail-section h3 {
      font-size: 0.8rem;
      color: #9ca3af;
      text-transform: uppercase;
      margin-bottom: 0.5rem;
    }

    .detail-section p, .detail-section pre {
      color: #e4e4e7;
      font-size: 0.9rem;
    }

    .detail-section pre {
      background: #0f172a;
      padding: 1rem;
      border-radius: 4px;
      overflow-x: auto;
      white-space: pre-wrap;
    }

    .phase-selector {
      margin-bottom: 1.5rem;
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }

    .phase-selector select {
      background: #1e293b;
      color: #e4e4e7;
      border: 1px solid #334155;
      padding: 0.5rem;
      border-radius: 4px;
      font-size: 1rem;
    }

    /* Task list styles */
    .task-item {
      background: #1e293b;
      border: 1px solid #334155;
      border-radius: 8px;
      padding: 1rem;
      margin-bottom: 0.75rem;
      transition: border-color 0.2s;
      cursor: pointer;
    }

    .task-item:hover { border-color: #3b82f6; }
    .task-item.selected { border-color: #3b82f6; background: #1e3a5f; }

    .task-header {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
    }

    .task-left {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      flex: 1;
    }

    .task-id {
      font-family: monospace;
      color: #00d9ff;
      font-weight: bold;
    }

    .task-title { font-weight: 500; flex: 1; }

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
      background: #334155;
      color: #60a5fa;
      padding: 0.2rem 0.5rem;
      border-radius: 4px;
      font-family: monospace;
    }

    /* Collapsible subtasks */
    .expand-btn {
      background: #334155;
      border: none;
      color: #60a5fa;
      width: 24px;
      height: 24px;
      border-radius: 4px;
      cursor: pointer;
      font-size: 0.9rem;
      display: flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
    }

    .expand-btn:hover { background: #3b82f6; }

    .subtasks {
      margin-left: 2rem;
      margin-top: 0.75rem;
      border-left: 2px solid #334155;
      padding-left: 1rem;
    }

    .subtasks.collapsed { display: none; }

    .subtask-item {
      background: #0f172a;
      border: 1px solid #334155;
      border-radius: 4px;
      padding: 0.75rem;
      margin-bottom: 0.5rem;
      cursor: pointer;
    }

    .subtask-item:hover { border-color: #3b82f6; }
    .subtask-item.selected { border-color: #3b82f6; background: #1e3a5f; }

    /* Dependencies */
    .dependencies {
      font-size: 0.8rem;
      color: #6b7280;
      margin-top: 0.5rem;
      margin-left: 2rem;
    }

    .dependencies span {
      background: #334155;
      padding: 0.1rem 0.4rem;
      border-radius: 3px;
      margin-right: 0.25rem;
    }

    /* Waves view */
    .wave-group {
      margin-bottom: 2rem;
    }

    .wave-header {
      background: #1e3a8a;
      padding: 0.75rem 1rem;
      border-radius: 8px 8px 0 0;
      font-weight: bold;
      color: #60a5fa;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }

    .wave-tasks {
      background: #1e293b;
      border: 1px solid #334155;
      border-top: none;
      border-radius: 0 0 8px 8px;
      padding: 1rem;
    }

    .wave-task {
      background: #0f172a;
      border: 1px solid #334155;
      border-radius: 4px;
      padding: 0.75rem;
      margin-bottom: 0.5rem;
      cursor: pointer;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }

    .wave-task:hover { border-color: #3b82f6; }
    .wave-task:last-child { margin-bottom: 0; }

    .wave-info {
      font-size: 0.85rem;
      color: #9ca3af;
    }

    /* Diagram controls */
    .diagram-controls {
      display: flex;
      gap: 2rem;
      align-items: center;
      margin-bottom: 1rem;
      flex-wrap: wrap;
    }

    .checkbox-label {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      cursor: pointer;
      color: #e4e4e7;
    }

    .checkbox-label input[type="checkbox"] {
      width: 1rem;
      height: 1rem;
      cursor: pointer;
    }

    /* Mermaid diagram */
    .diagram-phases-container {
      display: flex;
      flex-direction: column;
      gap: 1.5rem;
    }

    .phase-diagram {
      background: #1e293b;
      border: 1px solid #334155;
      border-radius: 8px;
      overflow: hidden;
    }

    .phase-diagram-header {
      background: #334155;
      padding: 0.75rem 1rem;
      font-weight: 600;
      color: #e4e4e7;
      border-bottom: 1px solid #475569;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }

    .phase-diagram-header .zoom-controls {
      display: flex;
      gap: 0.5rem;
      align-items: center;
    }

    .phase-diagram-header .zoom-controls button {
      background: #475569;
      border: none;
      color: #e4e4e7;
      padding: 0.25rem 0.5rem;
      border-radius: 4px;
      cursor: pointer;
      font-size: 0.9rem;
    }

    .phase-diagram-header .zoom-controls button:hover {
      background: #64748b;
    }

    .phase-diagram-header .zoom-level {
      color: #9ca3af;
      font-size: 0.8rem;
      min-width: 3rem;
      text-align: center;
    }

    .phase-diagram-viewport {
      height: 400px;
      overflow: hidden;
      cursor: grab;
      position: relative;
    }

    .phase-diagram-viewport:active {
      cursor: grabbing;
    }

    .phase-diagram-content {
      transform-origin: 0 0;
      padding: 1rem;
    }

    .phase-diagram-content svg {
      display: block;
    }

    .cross-phase-deps {
      background: #1e1b4b;
      border: 1px solid #4338ca;
      border-radius: 8px;
      padding: 1rem;
      margin-bottom: 1rem;
    }

    .cross-phase-deps h3 {
      color: #a5b4fc;
      margin-bottom: 0.5rem;
      font-size: 0.9rem;
    }

    .cross-phase-dep {
      color: #c7d2fe;
      font-size: 0.85rem;
      padding: 0.25rem 0;
    }

    /* Stats */
    .stats-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 1rem;
    }

    .stat-card {
      background: #1e293b;
      border: 1px solid #334155;
      border-radius: 8px;
      padding: 1.5rem;
      text-align: center;
    }

    .stat-value {
      font-size: 2.5rem;
      font-weight: bold;
      color: #3b82f6;
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
      border-bottom: 1px solid #334155;
    }

    .no-waves {
      color: #9ca3af;
      text-align: center;
      padding: 2rem;
    }

    .subtask-link:hover {
      background: #3b82f6 !important;
      color: white;
    }
  `;
}

/**
 * JavaScript for viewer interactivity
 */
function getViewerScript() {
  return `
    // Initialize mermaid
    mermaid.initialize({
      startOnLoad: false,
      theme: 'dark',
      securityLevel: 'loose',
      flowchart: {
        useMaxWidth: false,
        htmlLabels: true,
        curve: 'basis'
      },
      themeVariables: {
        primaryColor: '#3b82f6',
        primaryTextColor: '#e4e4e7',
        primaryBorderColor: '#334155',
        lineColor: '#64748b',
        secondaryColor: '#1e293b',
        tertiaryColor: '#0f172a'
      }
    });

    // Diagram state
    let diagramRenderCount = 0;
    let simplifiedView = true; // Default to simplified view
    const diagramStates = new Map(); // phase -> { zoom, panX, panY }

    // Generate mermaid diagram content for a single phase
    function generatePhaseDiagram(phaseName, simplified) {
      const phaseData = phases[phaseName];
      if (!phaseData || !phaseData.tasks) return null;

      let tasks = phaseData.tasks;

      // Filter to top-level tasks only if simplified view
      if (simplified) {
        tasks = tasks.filter(t => !t.parent_id);
      }

      if (tasks.length === 0) return null;

      let diagram = 'flowchart LR\\n';

      // Add class definitions for status colors
      diagram += '    classDef pending fill:#fef3c7,stroke:#f59e0b,color:#92400e\\n';
      diagram += '    classDef inprogress fill:#dbeafe,stroke:#3b82f6,color:#1e40af,stroke-width:2px\\n';
      diagram += '    classDef done fill:#d1fae5,stroke:#10b981,color:#065f46\\n';
      diagram += '    classDef blocked fill:#fee2e2,stroke:#ef4444,color:#991b1b\\n';
      diagram += '    classDef review fill:#fef3c7,stroke:#f59e0b,color:#92400e\\n';
      diagram += '    classDef expanded fill:#e0e7ff,stroke:#6366f1,color:#3730a3\\n';
      diagram += '    classDef cancelled fill:#f3f4f6,stroke:#9ca3af,color:#6b7280\\n';

      // Add nodes
      for (const task of tasks) {
        const safeId = 't' + task.id.replace(/\\./g, '_');
        const safeTitle = task.title.replace(/"/g, "'").replace(/[\\[\\]()]/g, '');
        const truncatedTitle = safeTitle.length > 35 ? safeTitle.substring(0, 32) + '...' : safeTitle;
        diagram += '    ' + safeId + '(["' + truncatedTitle + '"])\\n';

        // Add status class
        const statusClass = (task.status || 'pending').toLowerCase().replace(/-/g, '');
        diagram += '    class ' + safeId + ' ' + statusClass + '\\n';
      }

      // Add edges (dependencies)
      for (const task of tasks) {
        const safeId = 't' + task.id.replace(/\\./g, '_');
        for (const dep of (task.dependencies || [])) {
          const depTask = tasks.find(t => t.id === dep);
          if (depTask) {
            const safeDepId = 't' + dep.replace(/\\./g, '_');
            diagram += '    ' + safeDepId + ' --> ' + safeId + '\\n';
          }
        }

        // Add parent-child edges (dotted) if not simplified
        if (!simplified && task.parent_id) {
          const parentTask = tasks.find(t => t.id === task.parent_id);
          if (parentTask) {
            const safeParentId = 't' + task.parent_id.replace(/\\./g, '_');
            diagram += '    ' + safeParentId + ' -.-> ' + safeId + '\\n';
          }
        }
      }

      return diagram;
    }

    // Setup pan/zoom for a viewport
    function setupPanZoom(viewport, content, phaseName) {
      // Calculate initial zoom to fit content
      const svg = content.querySelector('svg');
      let initialZoom = 0.5;
      if (svg) {
        const svgRect = svg.getBoundingClientRect();
        const viewportRect = viewport.getBoundingClientRect();
        const scaleX = viewportRect.width / svgRect.width;
        const scaleY = viewportRect.height / svgRect.height;
        initialZoom = Math.min(scaleX, scaleY, 1) * 0.9; // 90% of fit, max 100%
        initialZoom = Math.max(0.1, Math.min(1, initialZoom)); // Clamp between 10% and 100%
      }

      let state = diagramStates.get(phaseName) || { zoom: initialZoom, panX: 10, panY: 10 };
      diagramStates.set(phaseName, state);

      let isPanning = false;
      let startX, startY;

      function updateTransform() {
        content.style.transform = 'translate(' + state.panX + 'px, ' + state.panY + 'px) scale(' + state.zoom + ')';
        const zoomLabel = viewport.parentElement.querySelector('.zoom-level');
        if (zoomLabel) zoomLabel.textContent = Math.round(state.zoom * 100) + '%';
      }

      // Mouse drag for panning
      viewport.addEventListener('mousedown', (e) => {
        isPanning = true;
        startX = e.clientX - state.panX;
        startY = e.clientY - state.panY;
        viewport.style.cursor = 'grabbing';
      });

      document.addEventListener('mousemove', (e) => {
        if (!isPanning) return;
        state.panX = e.clientX - startX;
        state.panY = e.clientY - startY;
        updateTransform();
      });

      document.addEventListener('mouseup', () => {
        isPanning = false;
        viewport.style.cursor = 'grab';
      });

      // Scroll wheel for zoom
      viewport.addEventListener('wheel', (e) => {
        e.preventDefault();
        const delta = e.deltaY > 0 ? 0.9 : 1.1;
        const newZoom = Math.max(0.1, Math.min(3, state.zoom * delta));

        // Zoom toward mouse position
        const rect = viewport.getBoundingClientRect();
        const mouseX = e.clientX - rect.left;
        const mouseY = e.clientY - rect.top;

        state.panX = mouseX - (mouseX - state.panX) * (newZoom / state.zoom);
        state.panY = mouseY - (mouseY - state.panY) * (newZoom / state.zoom);
        state.zoom = newZoom;

        updateTransform();
      });

      // Zoom buttons
      const header = viewport.parentElement.querySelector('.phase-diagram-header');
      if (header) {
        header.querySelector('.zoom-in')?.addEventListener('click', () => {
          state.zoom = Math.min(3, state.zoom * 1.2);
          updateTransform();
        });
        header.querySelector('.zoom-out')?.addEventListener('click', () => {
          state.zoom = Math.max(0.1, state.zoom * 0.8);
          updateTransform();
        });
        header.querySelector('.zoom-reset')?.addEventListener('click', () => {
          state.zoom = initialZoom;
          state.panX = 10;
          state.panY = 10;
          updateTransform();
        });
      }

      updateTransform();
    }

    // Render all phase diagrams
    async function renderDiagrams() {
      const container = document.getElementById('diagram-phases-container');
      if (!container) return;

      container.innerHTML = '';

      for (const phaseName of Object.keys(phases)) {
        const diagram = generatePhaseDiagram(phaseName, simplifiedView);
        if (!diagram) continue;

        // Create phase section
        const section = document.createElement('div');
        section.className = 'phase-diagram';
        section.innerHTML =
          '<div class="phase-diagram-header">' +
            '<span>' + phaseName + '</span>' +
            '<div class="zoom-controls">' +
              '<button class="zoom-out" title="Zoom out">−</button>' +
              '<span class="zoom-level">80%</span>' +
              '<button class="zoom-in" title="Zoom in">+</button>' +
              '<button class="zoom-reset" title="Reset view">⟲</button>' +
            '</div>' +
          '</div>' +
          '<div class="phase-diagram-viewport">' +
            '<div class="phase-diagram-content"></div>' +
          '</div>';

        container.appendChild(section);

        const content = section.querySelector('.phase-diagram-content');
        const viewport = section.querySelector('.phase-diagram-viewport');

        try {
          diagramRenderCount++;
          const { svg } = await mermaid.render('mermaid-' + phaseName.replace(/[^a-zA-Z0-9]/g, '_') + '-' + diagramRenderCount, diagram);
          content.innerHTML = svg;
          setupPanZoom(viewport, content, phaseName);
        } catch (e) {
          console.error('Mermaid render error for ' + phaseName + ':', e);
          content.innerHTML = '<p style="color: #ef4444; padding: 1rem;">Error: ' + e.message + '</p>';
        }
      }
    }

    // State
    let selectedTaskId = null;
    let currentPhase = null;

    // Tab navigation
    let diagramsRendered = false;
    document.querySelectorAll('.tab-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
        document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
        btn.classList.add('active');
        document.getElementById(btn.dataset.tab).classList.add('active');

        // Render diagrams when diagram tab is shown (first time only, or after toggle)
        if (btn.dataset.tab === 'diagram' && !diagramsRendered) {
          diagramsRendered = true;
          setTimeout(renderDiagrams, 100);
        }
      });
    });

    // Get phases from data
    function getPhases() {
      if (Array.isArray(TASKS_DATA)) {
        return { 'default': { tasks: TASKS_DATA } };
      }
      return TASKS_DATA;
    }

    const phases = getPhases();

    // Populate phase selectors
    function populatePhaseSelectors() {
      const selectors = ['phase-select', 'waves-phase-select'];
      selectors.forEach(id => {
        const select = document.getElementById(id);
        if (!select) return;
        select.innerHTML = '';
        Object.keys(phases).forEach(phase => {
          const option = document.createElement('option');
          option.value = phase;
          option.textContent = phase;
          select.appendChild(option);
        });
      });

      // Event handler for simplified view checkbox
      const simplifiedCheckbox = document.getElementById('simplified-view');
      if (simplifiedCheckbox) {
        simplifiedCheckbox.addEventListener('change', (e) => {
          simplifiedView = e.target.checked;
          diagramStates.clear(); // Reset pan/zoom states
          renderDiagrams();
        });
      }
    }

    populatePhaseSelectors();

    // Build task map for current phase
    function getTaskMap(phaseName) {
      const tasks = phases[phaseName]?.tasks || [];
      const taskMap = new Map();
      tasks.forEach(t => taskMap.set(t.id, t));
      return taskMap;
    }

    // Show task detail panel
    function showTaskDetail(task) {
      const panel = document.getElementById('detail-panel');
      const content = document.getElementById('detail-content');
      const layout = document.querySelector('.layout');

      selectedTaskId = task.id;
      document.querySelectorAll('.task-item, .subtask-item, .wave-task').forEach(el => {
        el.classList.toggle('selected', el.dataset.taskId === task.id);
      });

      panel.classList.remove('hidden');
      layout.classList.add('has-detail');

      const statusClass = 'status-' + (task.status || 'pending').toLowerCase().replace(/[^a-z]/g, '-');

      let html = '';

      // Status and meta
      html += '<div class="detail-section">';
      html += '<span class="badge ' + statusClass + '">' + escapeHtml(task.status || 'pending') + '</span>';
      if (task.complexity) html += ' <span class="complexity">Complexity: ' + task.complexity + '</span>';
      if (task.priority) html += ' <span class="badge">' + escapeHtml(task.priority) + '</span>';
      html += '</div>';

      // Description
      if (task.description) {
        html += '<div class="detail-section"><h3>Description</h3><p>' + escapeHtml(task.description) + '</p></div>';
      }

      // Details
      if (task.details) {
        html += '<div class="detail-section"><h3>Details</h3><pre>' + escapeHtml(task.details) + '</pre></div>';
      }

      // Test Strategy
      if (task.test_strategy) {
        html += '<div class="detail-section"><h3>Test Strategy</h3><pre>' + escapeHtml(task.test_strategy) + '</pre></div>';
      }

      // Dependencies
      if (task.dependencies && task.dependencies.length > 0) {
        html += '<div class="detail-section"><h3>Dependencies</h3><p>';
        task.dependencies.forEach(dep => {
          html += '<span class="badge" style="background:#334155;margin-right:0.25rem;">' + escapeHtml(dep) + '</span>';
        });
        html += '</p></div>';
      }

      // Subtasks (clickable to navigate)
      if (task.subtasks && task.subtasks.length > 0) {
        html += '<div class="detail-section"><h3>Subtasks (' + task.subtasks.length + ')</h3><p>';
        task.subtasks.forEach(st => {
          html += '<span class="badge subtask-link" data-subtask-id="' + escapeHtml(st) + '" style="background:#334155;margin-right:0.25rem;cursor:pointer;">' + escapeHtml(st) + '</span>';
        });
        html += '</p></div>';
      }

      // Assignment
      if (task.assigned_to) {
        html += '<div class="detail-section"><h3>Assigned To</h3><p>' + escapeHtml(task.assigned_to) + '</p></div>';
      }

      content.innerHTML = html;
      document.getElementById('detail-title').textContent = task.id + ': ' + task.title;

      // Add click handlers for subtask links in detail panel
      content.querySelectorAll('.subtask-link').forEach(link => {
        link.addEventListener('click', () => {
          const subtaskId = link.dataset.subtaskId;
          const taskMap = getTaskMap(currentPhase);
          const subtask = taskMap.get(subtaskId);
          if (subtask) {
            showTaskDetail(subtask);
          }
        });
      });
    }

    // Close detail panel
    document.getElementById('close-detail').addEventListener('click', () => {
      document.getElementById('detail-panel').classList.add('hidden');
      document.querySelector('.layout').classList.remove('has-detail');
      selectedTaskId = null;
      document.querySelectorAll('.task-item, .subtask-item, .wave-task').forEach(el => {
        el.classList.remove('selected');
      });
    });

    // Render tasks for selected phase
    function renderTasks(phaseName) {
      currentPhase = phaseName;
      const tasks = phases[phaseName]?.tasks || [];
      const container = document.getElementById('task-list');
      container.innerHTML = '';

      const taskMap = getTaskMap(phaseName);
      const rootTasks = tasks.filter(t => !t.parent_id);

      rootTasks.forEach(task => {
        container.appendChild(createTaskElement(task, taskMap, false));
      });
    }

    function createTaskElement(task, taskMap, isSubtask) {
      const div = document.createElement('div');
      const priorityClass = 'priority-' + (task.priority || 'medium').toLowerCase();
      div.className = (isSubtask ? 'subtask-item' : 'task-item') + ' ' + priorityClass;
      div.dataset.taskId = task.id;

      if (selectedTaskId === task.id) div.classList.add('selected');

      const statusClass = 'status-' + (task.status || 'pending').toLowerCase().replace(/[^a-z]/g, '-');
      const hasSubtasks = task.subtasks && task.subtasks.length > 0;

      let html = '<div class="task-header">';
      html += '<div class="task-left">';

      if (hasSubtasks) {
        html += '<button class="expand-btn" data-expanded="true">-</button>';
      }

      html += '<span class="task-id">' + escapeHtml(task.id) + '</span>';
      html += '<span class="task-title">' + escapeHtml(task.title) + '</span>';
      html += '</div>';
      html += '<div class="task-meta">';
      html += '<span class="badge ' + statusClass + '">' + escapeHtml(task.status || 'pending') + '</span>';
      if (task.complexity) {
        html += '<span class="complexity">C:' + task.complexity + '</span>';
      }
      html += '</div></div>';

      div.innerHTML = html;

      // Click to show detail (but not on expand button)
      div.addEventListener('click', (e) => {
        e.stopPropagation(); // Prevent bubbling to parent tasks
        if (!e.target.classList.contains('expand-btn')) {
          showTaskDetail(task);
        }
      });

      // Add subtasks
      if (hasSubtasks) {
        const subtasksDiv = document.createElement('div');
        subtasksDiv.className = 'subtasks';

        task.subtasks.forEach(subtaskId => {
          const subtask = taskMap.get(subtaskId);
          if (subtask) {
            subtasksDiv.appendChild(createTaskElement(subtask, taskMap, true));
          }
        });

        div.appendChild(subtasksDiv);

        // Expand/collapse handler
        const expandBtn = div.querySelector('.expand-btn');
        expandBtn.addEventListener('click', (e) => {
          e.stopPropagation();
          const isExpanded = expandBtn.dataset.expanded === 'true';
          expandBtn.dataset.expanded = isExpanded ? 'false' : 'true';
          expandBtn.textContent = isExpanded ? '+' : '-';
          subtasksDiv.classList.toggle('collapsed', isExpanded);
        });
      }

      // Dependencies indicator
      if (task.dependencies && task.dependencies.length > 0 && !isSubtask) {
        const depsDiv = document.createElement('div');
        depsDiv.className = 'dependencies';
        depsDiv.innerHTML = 'Depends on: ' + task.dependencies.map(d => '<span>' + escapeHtml(d) + '</span>').join('');
        div.appendChild(depsDiv);
      }

      return div;
    }

    // Render waves view
    function renderWaves(phaseName) {
      const container = document.getElementById('waves-list');
      const waves = WAVES_DATA[phaseName] || [];
      const taskMap = getTaskMap(phaseName);

      if (waves.length === 0) {
        container.innerHTML = '<div class="no-waves">No wave data available for this phase.<br>Run <code>scud waves --tag ' + escapeHtml(phaseName) + '</code> to generate.</div>';
        return;
      }

      container.innerHTML = '';

      waves.forEach(wave => {
        const waveDiv = document.createElement('div');
        waveDiv.className = 'wave-group';

        const headerText = wave.round > 1 ? 'Wave ' + wave.wave + ' (Round ' + wave.round + ')' : 'Wave ' + wave.wave;

        let headerHtml = '<div class="wave-header">';
        headerHtml += '<span>' + headerText + '</span>';
        headerHtml += '<span class="wave-info">' + wave.tasks.length + ' task' + (wave.tasks.length !== 1 ? 's' : '') + '</span>';
        headerHtml += '</div>';

        let tasksHtml = '<div class="wave-tasks">';
        wave.tasks.forEach(waveTask => {
          const fullTask = taskMap.get(waveTask.id) || waveTask;
          const statusClass = 'status-' + (fullTask.status || 'pending').toLowerCase().replace(/[^a-z]/g, '-');

          tasksHtml += '<div class="wave-task" data-task-id="' + escapeHtml(waveTask.id) + '">';
          tasksHtml += '<div class="task-left">';
          tasksHtml += '<span class="task-id">' + escapeHtml(waveTask.id) + '</span>';
          tasksHtml += '<span class="task-title">' + escapeHtml(waveTask.title || fullTask.title) + '</span>';
          tasksHtml += '</div>';
          tasksHtml += '<div class="task-meta">';
          tasksHtml += '<span class="badge ' + statusClass + '">' + escapeHtml(fullTask.status || 'pending') + '</span>';
          if (fullTask.complexity) {
            tasksHtml += '<span class="complexity">C:' + fullTask.complexity + '</span>';
          }
          tasksHtml += '</div></div>';
        });
        tasksHtml += '</div>';

        waveDiv.innerHTML = headerHtml + tasksHtml;

        // Add click handlers for wave tasks
        waveDiv.querySelectorAll('.wave-task').forEach(el => {
          el.addEventListener('click', () => {
            const taskId = el.dataset.taskId;
            const task = taskMap.get(taskId);
            if (task) showTaskDetail(task);
          });
        });

        container.appendChild(waveDiv);
      });
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

      html += '<h3 style="margin: 2rem 0 1rem; color: #3b82f6;">By Status</h3>';
      html += '<div class="stat-card"><div class="stat-breakdown">';
      Object.entries(byStatus).sort((a, b) => b[1] - a[1]).forEach(([status, count]) => {
        html += '<div class="stat-row"><span>' + status + '</span><span>' + count + '</span></div>';
      });
      html += '</div></div>';

      html += '<h3 style="margin: 2rem 0 1rem; color: #3b82f6;">By Priority</h3>';
      html += '<div class="stat-card"><div class="stat-breakdown">';
      Object.entries(byPriority).sort((a, b) => b[1] - a[1]).forEach(([priority, count]) => {
        html += '<div class="stat-row"><span>' + priority + '</span><span>' + count + '</span></div>';
      });
      html += '</div></div>';

      container.innerHTML = html;
    }

    // Phase selector changes
    document.getElementById('phase-select').addEventListener('change', (e) => {
      renderTasks(e.target.value);
    });

    document.getElementById('waves-phase-select').addEventListener('change', (e) => {
      renderWaves(e.target.value);
    });

    // Initial render
    const firstPhase = Object.keys(phases)[0];
    if (firstPhase) {
      renderTasks(firstPhase);
      renderWaves(firstPhase);
    }
    renderStats();
  `;
}
