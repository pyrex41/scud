#!/usr/bin/env node

/**
 * SCUD Validator
 *
 * Validates SCUD state and enforces workflow rules.
 * Used by slash commands to ensure correct workflow usage.
 */

const fs = require('fs');
const path = require('path');

class ScudValidator {
  constructor(projectRoot = process.cwd()) {
    this.projectRoot = projectRoot;
    this.scudDir = path.join(projectRoot, '.scud');
    this.taskmasterDir = path.join(projectRoot, '.taskmaster');
    this.workflowStatePath = this.resolvePath('workflow-state.json');
    this.tasksPath = this.resolvePath('tasks', 'tasks.json');
  }

  resolvePath(...segments) {
    const preferred = path.join(this.scudDir, ...segments);
    if (fs.existsSync(preferred)) {
      return preferred;
    }
    const fallback = path.join(this.taskmasterDir, ...segments);
    if (fs.existsSync(fallback)) {
      return fallback;
    }
    return preferred;
  }

  /**
   * Load workflow state from disk
   */
  loadWorkflowState() {
    if (!fs.existsSync(this.workflowStatePath)) {
      throw new Error(`Workflow state not found: ${this.workflowStatePath}\nRun installation script first.`);
    }
    return JSON.parse(fs.readFileSync(this.workflowStatePath, 'utf8'));
  }

  /**
   * Load SCUD tasks from disk
   */
  loadTasks() {
    if (!fs.existsSync(this.tasksPath)) {
      throw new Error(`Task data not found: ${this.tasksPath}\nRun: scud init`);
    }
    return JSON.parse(fs.readFileSync(this.tasksPath, 'utf8'));
  }

  /**
   * Save workflow state to disk
   */
  saveWorkflowState(state) {
    fs.writeFileSync(this.workflowStatePath, JSON.stringify(state, null, 2));
  }

  /**
   * Validate that SCUD CLI is available
   */
  validateScudCLI() {
    const { execSync } = require('child_process');
    try {
      execSync('scud --version', { stdio: 'ignore' });
      return { valid: true };
    } catch (error) {
      return {
        valid: false,
        error: 'SCUD CLI not found. Install: npm install -g scud-task'
      };
    }
  }

  /**
   * Validate workflow phase for agent activation
   */
  validatePhase(agentName, allowedPhases) {
    const state = this.loadWorkflowState();
    const currentPhase = state.current_phase;

    if (!allowedPhases.includes(currentPhase)) {
      return {
        valid: false,
        currentPhase,
        allowedPhases,
        error: `Agent '${agentName}' can only run in phases: ${allowedPhases.join(', ')}. Current phase: ${currentPhase}`
      };
    }

    return { valid: true, currentPhase };
  }

  /**
   * Validate that active epic exists in SCUD
   */
  validateActiveEpic() {
    const state = this.loadWorkflowState();
    const tasks = this.loadTasks();

    if (!state.active_epic) {
      return {
        valid: false,
        error: 'No active epic in workflow state. Run /scud:pm to create one.'
      };
    }

    if (!tasks[state.active_epic]) {
      return {
        valid: false,
        error: `Active epic '${state.active_epic}' not found in SCUD.`
      };
    }

    return {
      valid: true,
      epic: state.active_epic,
      tasks: tasks[state.active_epic].tasks
    };
  }

  /**
   * Validate task dependencies are met
   */
  validateDependencies(epicTag, taskId) {
    const tasks = this.loadTasks();
    const epic = tasks[epicTag];

    if (!epic) {
      return {
        valid: false,
        error: `Epic '${epicTag}' not found in SCUD.`
      };
    }

    const task = epic.tasks.find(t => t.id === taskId);
    if (!task) {
      return {
        valid: false,
        error: `Task ${taskId} not found in epic '${epicTag}'.`
      };
    }

    const dependencies = task.dependencies || [];
    const unmetDependencies = [];

    for (const depId of dependencies) {
      const depTask = epic.tasks.find(t => t.id === depId);
      if (!depTask) {
        return {
          valid: false,
          error: `Dependency task ${depId} not found in epic.`
        };
      }

      if (depTask.status !== 'done') {
        unmetDependencies.push({
          id: depTask.id,
          title: depTask.title,
          status: depTask.status
        });
      }
    }

    if (unmetDependencies.length > 0) {
      return {
        valid: false,
        unmetDependencies,
        error: `Task ${taskId} has ${unmetDependencies.length} incomplete dependencies.`
      };
    }

    return {
      valid: true,
      task,
      dependencies
    };
  }

  /**
   * Validate all tasks in epic are complete
   */
  validateEpicComplete(epicTag) {
    const tasks = this.loadTasks();
    const epic = tasks[epicTag];

    if (!epic) {
      return {
        valid: false,
        error: `Epic '${epicTag}' not found in SCUD.`
      };
    }

    const incompleteTasks = epic.tasks.filter(t => t.status !== 'done');

    if (incompleteTasks.length > 0) {
      return {
        valid: false,
        incompleteTasks: incompleteTasks.map(t => ({
          id: t.id,
          title: t.title,
          status: t.status
        })),
        error: `Epic has ${incompleteTasks.length} incomplete tasks.`
      };
    }

    return {
      valid: true,
      totalTasks: epic.tasks.length
    };
  }

  /**
   * Get available tasks (no unmet dependencies)
   */
  getAvailableTasks(epicTag) {
    const tasks = this.loadTasks();
    const epic = tasks[epicTag];

    if (!epic) {
      return {
        valid: false,
        error: `Epic '${epicTag}' not found in SCUD.`
      };
    }

    const availableTasks = [];
    const blockedTasks = [];

    for (const task of epic.tasks) {
      if (task.status === 'done') continue;

      const depCheck = this.validateDependencies(epicTag, task.id);

      if (depCheck.valid) {
        availableTasks.push({
          id: task.id,
          title: task.title,
          status: task.status,
          priority: task.priority,
          complexity: task.complexity
        });
      } else {
        blockedTasks.push({
          id: task.id,
          title: task.title,
          status: task.status,
          unmetDependencies: depCheck.unmetDependencies
        });
      }
    }

    return {
      valid: true,
      availableTasks,
      blockedTasks
    };
  }

  /**
   * Update workflow phase
   */
  updatePhase(newPhase, updates = {}) {
    const state = this.loadWorkflowState();
    const now = new Date().toISOString();

    // Mark current phase as complete
    if (state.current_phase && state.phases[state.current_phase]) {
      state.phases[state.current_phase].status = 'completed';
      state.phases[state.current_phase].completed_at = now;
    }

    // Activate new phase
    state.current_phase = newPhase;
    if (state.phases[newPhase]) {
      state.phases[newPhase].status = 'active';
    }

    // Apply additional updates
    Object.assign(state, updates);

    state.last_updated = now;

    this.saveWorkflowState(state);

    return { success: true, state };
  }

  /**
   * Add entry to workflow history
   */
  addHistoryEntry(entry) {
    const state = this.loadWorkflowState();

    if (!state.history) {
      state.history = [];
    }

    state.history.push({
      ...entry,
      timestamp: new Date().toISOString()
    });

    state.last_updated = new Date().toISOString();

    this.saveWorkflowState(state);

    return { success: true };
  }

  /**
   * Get epic statistics
   */
  getEpicStats(epicTag) {
    const tasks = this.loadTasks();
    const epic = tasks[epicTag];

    if (!epic) {
      return {
        valid: false,
        error: `Epic '${epicTag}' not found in SCUD.`
      };
    }

    const tasksByStatus = {
      done: [],
      'in-progress': [],
      blocked: [],
      pending: []
    };

    let totalComplexity = 0;

    for (const task of epic.tasks) {
      const status = task.status || 'pending';
      if (tasksByStatus[status]) {
        tasksByStatus[status].push(task);
      }
      totalComplexity += task.complexity || 0;
    }

    return {
      valid: true,
      epic: epicTag,
      totalTasks: epic.tasks.length,
      totalComplexity,
      byStatus: {
        done: tasksByStatus.done.length,
        inProgress: tasksByStatus['in-progress'].length,
        blocked: tasksByStatus.blocked.length,
        pending: tasksByStatus.pending.length
      },
      tasks: tasksByStatus
    };
  }

  /**
   * List all epic tags in SCUD
   */
  listEpicTags() {
    const tasks = this.loadTasks();
    const tags = Object.keys(tasks);

    return {
      valid: true,
      tags,
      count: tags.length
    };
  }

  /**
   * Get currently active epic tag from workflow state
   */
  getActiveEpicTag() {
    const state = this.loadWorkflowState();
    return {
      valid: true,
      activeEpic: state.active_epic || null
    };
  }

  /**
   * Set active epic tag in workflow state
   */
  setActiveEpicTag(epicTag) {
    const state = this.loadWorkflowState();
    const tasks = this.loadTasks();

    // Verify epic exists
    if (!tasks[epicTag]) {
      return {
        valid: false,
        error: `Epic '${epicTag}' not found in SCUD.`
      };
    }

    state.active_epic = epicTag;
    state.last_updated = new Date().toISOString();

    this.saveWorkflowState(state);

    return {
      valid: true,
      activeEpic: epicTag
    };
  }

  /**
   * Get command availability for /status
   */
  getCommandAvailability() {
    const state = this.loadWorkflowState();
    const currentPhase = state.current_phase;

    const commands = {
      'scud:pm': { available: false, reason: '' },
      'scud:architect': { available: false, reason: '' },
      'scud:dev': { available: false, reason: '' },
      'scud:retrospective': { available: false, reason: '' }
    };

    // scud:pm: Always available in ideation/planning
    if (['ideation', 'planning'].includes(currentPhase)) {
      commands['scud:pm'].available = true;
      commands['scud:pm'].reason = 'Ready to create PRD or parse into SCUD';
    } else {
      commands['scud:pm'].reason = `Only available in ideation/planning phases (current: ${currentPhase})`;
    }

    // scud:architect: Available when planning complete and tag exists
    if (currentPhase === 'architecture' && state.active_epic) {
      commands['scud:architect'].available = true;
      commands['scud:architect'].reason = 'Ready to design architecture';
    } else if (!state.active_epic) {
      commands['scud:architect'].reason = 'No tag in SCUD - run /scud:pm first';
    } else {
      commands['scud:architect'].reason = `Only available in architecture phase (current: ${currentPhase})`;
    }

    // scud:dev: Available when architecture complete
    if (currentPhase === 'implementation' && state.active_epic) {
      commands['scud:dev'].available = true;
      commands['scud:dev'].reason = 'Ready to implement tasks';
    } else if (!state.active_epic) {
      commands['scud:dev'].reason = 'No tag in SCUD - complete planning first';
    } else {
      commands['scud:dev'].reason = `Only available in implementation phase (current: ${currentPhase})`;
    }

    // scud:retrospective: Available when all tasks done
    if (state.active_epic) {
      const epicComplete = this.validateEpicComplete(state.active_epic);
      if (epicComplete.valid) {
        commands['scud:retrospective'].available = true;
        commands['scud:retrospective'].reason = 'All tasks complete - ready for retrospective';
      } else {
        commands['scud:retrospective'].reason = `Tag has ${epicComplete.incompleteTasks.length} incomplete tasks`;
      }
    } else {
      commands['scud:retrospective'].reason = 'No active tag';
    }

    return commands;
  }
}

// CLI Interface
if (require.main === module) {
  const validator = new ScudValidator();
  const command = process.argv[2];

  try {
    let result;

    switch (command) {
      case 'validate-cli':
        result = validator.validateScudCLI();
        break;

      case 'validate-phase':
        const agent = process.argv[3];
        const phases = process.argv.slice(4);
        result = validator.validatePhase(agent, phases);
        break;

      case 'validate-epic':
        result = validator.validateActiveEpic();
        break;

      case 'validate-dependencies':
        const epicTag = process.argv[3];
        const taskId = process.argv[4];
        result = validator.validateDependencies(epicTag, taskId);
        break;

      case 'validate-epic-complete':
        result = validator.validateEpicComplete(process.argv[3]);
        break;

      case 'get-available-tasks':
        result = validator.getAvailableTasks(process.argv[3]);
        break;

      case 'get-epic-stats':
        result = validator.getEpicStats(process.argv[3]);
        break;

      case 'get-command-availability':
        result = validator.getCommandAvailability();
        break;

      case 'update-phase':
        const newPhase = process.argv[3];
        const updates = process.argv[4] ? JSON.parse(process.argv[4]) : {};
        result = validator.updatePhase(newPhase, updates);
        break;

      case 'add-history':
        const entry = JSON.parse(process.argv[3]);
        result = validator.addHistoryEntry(entry);
        break;

      case 'list-epic-tags':
        result = validator.listEpicTags();
        break;

      case 'get-active-epic-tag':
        result = validator.getActiveEpicTag();
        break;

      case 'set-active-epic-tag':
        const epicTagToSet = process.argv[3];
        result = validator.setActiveEpicTag(epicTagToSet);
        break;

      default:
        console.error(`Unknown command: ${command}`);
        console.log(`
Usage: scud-validator.js <command> [args]

Commands:
  validate-cli                                  Check if SCUD CLI is available
  validate-phase <agent> <phase1> [phase2...]   Validate current phase for agent
  validate-epic                                  Check if active epic exists
  validate-dependencies <epic> <task-id>         Check if task dependencies are met
  validate-epic-complete <epic>                  Check if all tasks in epic are done
  get-available-tasks <epic>                     Get tasks with no unmet dependencies
  get-epic-stats <epic>                          Get epic statistics
  get-command-availability                       Get which commands are available
  update-phase <new-phase> [json-updates]       Update workflow phase
  add-history <json-entry>                       Add entry to workflow history
  list-epic-tags                                 List all epic tags in SCUD
  get-active-epic-tag                            Get currently active epic tag
  set-active-epic-tag <epic-tag>                 Set active epic tag in workflow state
        `);
        process.exit(1);
    }

    console.log(JSON.stringify(result, null, 2));
    process.exit(result.valid !== false && result.success !== false ? 0 : 1);
  } catch (error) {
    console.error(JSON.stringify({
      valid: false,
      error: error.message
    }, null, 2));
    process.exit(1);
  }
}

module.exports = ScudValidator;
