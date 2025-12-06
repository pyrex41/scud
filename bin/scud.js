#!/usr/bin/env node

/**
 * SCUD CLI
 * Sprint Cycle Unified Development
 * Main entry point for scud commands
 */

const { execSync, spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const { tmpdir } = require('os');

const command = process.argv[2];
const args = process.argv.slice(3);

// Task management commands (use Rust CLI)
const taskCommands = ['tags', 'use-tag', 'list', 'show', 'set-status', 'next', 'stats', 'mermaid', 'waves', 'doctor', 'convert', 'assign', 'whois', 'migrate', 'hooks', 'warmup', 'commit', 'next-batch', 'who-is', 'reanalyze-deps'];

// AI-powered commands (use Rust CLI)
const aiCommands = ['parse-prd', 'analyze-complexity', 'expand', 'research'];

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
╭────────────────────────────────────╮
│                                    │
│   SCUD CLI                         │
│   Sprint Cycle Unified Development │
│                                    │
╰────────────────────────────────────╯

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

  if (!fs.existsSync(scudDir)) {
    console.error('❌ Error: No .scud directory found. Run: scud init');
    process.exit(1);
  }

  if (!fs.existsSync(tasksJsonPath)) {
    console.error('❌ Error: No tasks found. Run: scud parse-prd <prd-file>');
    process.exit(1);
  }

  // Load task data
  const tasksData = JSON.parse(fs.readFileSync(tasksJsonPath, 'utf8'));

  // Find Rust binary - prefer system-installed (cargo) over local builds
  const homedir = require('os').homedir();
  const cargoBinary = path.join(homedir, '.cargo', 'bin', 'scud');
  const localRelease = path.join(__dirname, '..', 'scud-cli', 'target', 'release', 'scud');
  const localDebug = path.join(__dirname, '..', 'scud-cli', 'target', 'debug', 'scud');
  const scudBinary = fs.existsSync(cargoBinary) ? cargoBinary :
                     fs.existsSync(localRelease) ? localRelease : localDebug;

  // Generate mermaid diagram by calling Rust binary directly
  let mermaidDiagram = '';
  try {
    if (fs.existsSync(scudBinary)) {
      mermaidDiagram = execSync(`"${scudBinary}" mermaid --all-tags`, {
        encoding: 'utf8',
        cwd: process.cwd()
      });
    } else {
      mermaidDiagram = '```mermaid\ngraph TD\n  A[Rust binary not found - run: cd scud-cli && cargo build --release]\n```';
    }
  } catch (e) {
    mermaidDiagram = '```mermaid\ngraph TD\n  A[No diagram available]\n```';
  }

  // Generate waves data by parsing scud waves output
  let wavesData = {};
  try {
    if (fs.existsSync(scudBinary)) {
      // Get waves for each phase
      const phases = Object.keys(tasksData);
      for (const phase of phases) {
        try {
          const wavesOutput = execSync(`"${scudBinary}" waves --tag "${phase}"`, {
            encoding: 'utf8',
            cwd: process.cwd()
          });
          wavesData[phase] = parseWavesOutput(wavesOutput);
        } catch (e) {
          wavesData[phase] = [];
        }
      }
    }
  } catch (e) {
    // Waves data not available
  }

  // Generate HTML
  const html = generateViewerHtml(tasksData, mermaidDiagram, wavesData);

  // Write to temp file
  const tempFile = path.join(tmpdir(), `scud-view-${Date.now()}.html`);
  fs.writeFileSync(tempFile, html);

  console.log('✅ Opening SCUD viewer...');
  // Dynamic import for ESM-only open package
  const { default: open } = await import('open');
  await open(tempFile);
}

/**
 * Parse waves output from scud waves command
 * Returns array of waves, each wave containing task info
 */
function parseWavesOutput(output) {
  const waves = [];
  let currentWave = null;

  const lines = output.split('\n');
  for (const line of lines) {
    // Match wave header like "Wave 1:" or "  Wave 1 (Round 1):"
    const waveMatch = line.match(/Wave\s+(\d+)(?:\s+\(Round\s+(\d+)\))?:/i);
    if (waveMatch) {
      currentWave = {
        wave: parseInt(waveMatch[1]),
        round: waveMatch[2] ? parseInt(waveMatch[2]) : 1,
        tasks: []
      };
      waves.push(currentWave);
      continue;
    }

    // Match task line like "  [P] task:1 - Task title (C:3) deps: [task:0]"
    // or simpler format "  [D] task:1 - Task title"
    const taskMatch = line.match(/\[([PIDBRCXF])\]\s+(\S+)\s+-\s+(.+?)(?:\s+\(C:(\d+)\))?(?:\s+deps:\s+\[([^\]]*)\])?$/);
    if (taskMatch && currentWave) {
      const statusMap = {
        'P': 'pending',
        'I': 'in-progress',
        'D': 'done',
        'B': 'blocked',
        'R': 'review',
        'C': 'cancelled',
        'X': 'expanded',
        'F': 'deferred'
      };
      currentWave.tasks.push({
        id: taskMatch[2],
        title: taskMatch[3].trim(),
        status: statusMap[taskMatch[1]] || 'pending',
        complexity: taskMatch[4] ? parseInt(taskMatch[4]) : 0,
        dependencies: taskMatch[5] ? taskMatch[5].split(',').map(d => d.trim()).filter(d => d) : []
      });
    }
  }

  return waves;
}

/**
 * Generate the complete HTML viewer
 */
function generateViewerHtml(tasksData, mermaidDiagram, wavesData) {
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
        <div class="mermaid">
${mermaidContent}
        </div>
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

    /* Mermaid diagram */
    .mermaid {
      background: #1e293b;
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
      startOnLoad: true,
      theme: 'dark',
      themeVariables: {
        primaryColor: '#3b82f6',
        primaryTextColor: '#e4e4e7',
        primaryBorderColor: '#334155',
        lineColor: '#334155',
        secondaryColor: '#1e293b',
        tertiaryColor: '#0f172a'
      }
    });

    // State
    let selectedTaskId = null;
    let currentPhase = null;

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
