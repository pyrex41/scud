#!/usr/bin/env node

/**
 * BMAD-TM Validator
 *
 * Validates Task Master state and enforces workflow rules.
 * Used by slash commands to ensure correct workflow usage.
 */

const fs = require('fs');
const path = require('path');

class TaskMasterValidator {
  constructor(projectRoot = process.cwd()) {
    this.projectRoot = projectRoot;
    this.workflowStatePath = path.join(projectRoot, '.taskmaster', 'workflow-state.json');
    this.tasksPath = path.join(projectRoot, '.taskmaster', 'tasks', 'tasks.json');
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
   * Load Task Master tasks from disk
   */
  loadTasks() {
    if (!fs.existsSync(this.tasksPath)) {
      throw new Error(`Task Master tasks not found: ${this.tasksPath}\nRun: task-master init`);
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
   * Validate that Task Master CLI is available
   */
  validateTaskMasterCLI() {
    const { execSync } = require('child_process');
    try {
      execSync('task-master --version', { stdio: 'ignore' });
      return { valid: true };
    } catch (error) {
      return {
        valid: false,
        error: 'Task Master CLI not found. Install: npm install -g task-master'
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
   * Validate that active epic exists in Task Master
   */
  validateActiveEpic() {
    const state = this.loadWorkflowState();
    const tasks = this.loadTasks();

    if (!state.active_epic) {
      return {
        valid: false,
        error: 'No active epic in workflow state. Run /tm-pm to create one.'
      };
    }

    if (!tasks[state.active_epic]) {
      return {
        valid: false,
        error: `Active epic '${state.active_epic}' not found in Task Master.`
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
        error: `Epic '${epicTag}' not found in Task Master.`
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
        error: `Epic '${epicTag}' not found in Task Master.`
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
        error: `Epic '${epicTag}' not found in Task Master.`
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
        error: `Epic '${epicTag}' not found in Task Master.`
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
   * Get command availability for /status
   */
  getCommandAvailability() {
    const state = this.loadWorkflowState();
    const currentPhase = state.current_phase;

    const commands = {
      'tm-pm': { available: false, reason: '' },
      'tm-architect': { available: false, reason: '' },
      'tm-dev': { available: false, reason: '' },
      'tm-retrospective': { available: false, reason: '' }
    };

    // tm-pm: Always available in ideation/planning
    if (['ideation', 'planning'].includes(currentPhase)) {
      commands['tm-pm'].available = true;
      commands['tm-pm'].reason = 'Ready to create PRD or parse into Task Master';
    } else {
      commands['tm-pm'].reason = `Only available in ideation/planning phases (current: ${currentPhase})`;
    }

    // tm-architect: Available when planning complete and epic exists
    if (currentPhase === 'architecture' && state.active_epic) {
      commands['tm-architect'].available = true;
      commands['tm-architect'].reason = 'Ready to design architecture';
    } else if (!state.active_epic) {
      commands['tm-architect'].reason = 'No epic in Task Master - run /tm-pm first';
    } else {
      commands['tm-architect'].reason = `Only available in architecture phase (current: ${currentPhase})`;
    }

    // tm-dev: Available when architecture complete
    if (currentPhase === 'implementation' && state.active_epic) {
      commands['tm-dev'].available = true;
      commands['tm-dev'].reason = 'Ready to implement tasks';
    } else if (!state.active_epic) {
      commands['tm-dev'].reason = 'No epic in Task Master - complete planning first';
    } else {
      commands['tm-dev'].reason = `Only available in implementation phase (current: ${currentPhase})`;
    }

    // tm-retrospective: Available when all tasks done
    if (state.active_epic) {
      const epicComplete = this.validateEpicComplete(state.active_epic);
      if (epicComplete.valid) {
        commands['tm-retrospective'].available = true;
        commands['tm-retrospective'].reason = 'All tasks complete - ready for retrospective';
      } else {
        commands['tm-retrospective'].reason = `Epic has ${epicComplete.incompleteTasks.length} incomplete tasks`;
      }
    } else {
      commands['tm-retrospective'].reason = 'No active epic';
    }

    return commands;
  }
}

// CLI Interface
if (require.main === module) {
  const validator = new TaskMasterValidator();
  const command = process.argv[2];

  try {
    let result;

    switch (command) {
      case 'validate-cli':
        result = validator.validateTaskMasterCLI();
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

      default:
        console.error(`Unknown command: ${command}`);
        console.log(`
Usage: taskmaster-validator.js <command> [args]

Commands:
  validate-cli                                  Check if Task Master CLI is available
  validate-phase <agent> <phase1> [phase2...]   Validate current phase for agent
  validate-epic                                  Check if active epic exists
  validate-dependencies <epic> <task-id>         Check if task dependencies are met
  validate-epic-complete <epic>                  Check if all tasks in epic are done
  get-available-tasks <epic>                     Get tasks with no unmet dependencies
  get-epic-stats <epic>                          Get epic statistics
  get-command-availability                       Get which commands are available
  update-phase <new-phase> [json-updates]       Update workflow phase
  add-history <json-entry>                       Add entry to workflow history
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

module.exports = TaskMasterValidator;
