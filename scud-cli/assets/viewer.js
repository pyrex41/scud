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

  let diagram = 'flowchart LR\n';

  // Add class definitions for status colors
  diagram += '    classDef pending fill:#fef3c7,stroke:#f59e0b,color:#92400e\n';
  diagram += '    classDef inprogress fill:#dbeafe,stroke:#3b82f6,color:#1e40af,stroke-width:2px\n';
  diagram += '    classDef done fill:#d1fae5,stroke:#10b981,color:#065f46\n';
  diagram += '    classDef blocked fill:#fee2e2,stroke:#ef4444,color:#991b1b\n';
  diagram += '    classDef review fill:#fef3c7,stroke:#f59e0b,color:#92400e\n';
  diagram += '    classDef expanded fill:#e0e7ff,stroke:#6366f1,color:#3730a3\n';
  diagram += '    classDef cancelled fill:#f3f4f6,stroke:#9ca3af,color:#6b7280\n';

  // Add nodes
  for (const task of tasks) {
    const safeId = 't' + task.id.replace(/\./g, '_');
    const safeTitle = task.title.replace(/"/g, "'").replace(/[\[\]()]/g, '');
    const truncatedTitle = safeTitle.length > 35 ? safeTitle.substring(0, 32) + '...' : safeTitle;
    diagram += '    ' + safeId + '(["' + truncatedTitle + '"])\n';

    // Add status class
    const statusClass = (task.status || 'pending').toLowerCase().replace(/-/g, '');
    diagram += '    class ' + safeId + ' ' + statusClass + '\n';
  }

  // Add edges (dependencies)
  for (const task of tasks) {
    const safeId = 't' + task.id.replace(/\./g, '_');
    for (const dep of (task.dependencies || [])) {
      const depTask = tasks.find(t => t.id === dep);
      if (depTask) {
        const safeDepId = 't' + dep.replace(/\./g, '_');
        diagram += '    ' + safeDepId + ' --> ' + safeId + '\n';
      }
    }

    // Add parent-child edges (dotted) if not simplified
    if (!simplified && task.parent_id) {
      const parentTask = tasks.find(t => t.id === task.parent_id);
      if (parentTask) {
        const safeParentId = 't' + task.parent_id.replace(/\./g, '_');
        diagram += '    ' + safeParentId + ' -.-> ' + safeId + '\n';
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
          '<button class="zoom-out" title="Zoom out">-</button>' +
          '<span class="zoom-level">80%</span>' +
          '<button class="zoom-in" title="Zoom in">+</button>' +
          '<button class="zoom-reset" title="Reset view">R</button>' +
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
