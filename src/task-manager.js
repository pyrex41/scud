#!/usr/bin/env node

/**
 * Simple Task Manager for BMAD-TM
 *
 * Provides core task operations without external dependencies.
 * For AI-powered features (expand, analyze-complexity, parse-prd),
 * use the external task-master CLI.
 */

const fs = require('fs');
const path = require('path');

class TaskManager {
  constructor(projectRoot = process.cwd()) {
    this.projectRoot = projectRoot;
    this.tasksPath = path.join(projectRoot, '.taskmaster', 'tasks', 'tasks.json');
    this.workflowPath = path.join(projectRoot, '.taskmaster', 'workflow-state.json');
  }

  /**
   * Load tasks from disk
   */
  loadTasks() {
    if (!fs.existsSync(this.tasksPath)) {
      throw new Error(`Tasks file not found: ${this.tasksPath}\nRun: bmad-tm init`);
    }
    return JSON.parse(fs.readFileSync(this.tasksPath, 'utf8'));
  }

  /**
   * Save tasks to disk
   */
  saveTasks(tasks) {
    const dir = path.dirname(this.tasksPath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
    fs.writeFileSync(this.tasksPath, JSON.stringify(tasks, null, 2));
  }

  /**
   * Load workflow state
   */
  loadWorkflowState() {
    if (!fs.existsSync(this.workflowPath)) {
      throw new Error(`Workflow state not found: ${this.workflowPath}\nRun: bmad-tm init`);
    }
    return JSON.parse(fs.readFileSync(this.workflowPath, 'utf8'));
  }

  /**
   * Save workflow state
   */
  saveWorkflowState(state) {
    fs.writeFileSync(this.workflowPath, JSON.stringify(state, null, 2));
  }

  /**
   * Get active epic tag from workflow state
   */
  getActiveEpic() {
    const state = this.loadWorkflowState();
    return state.active_epic;
  }

  /**
   * Set active epic tag in workflow state
   */
  setActiveEpic(epicTag) {
    const tasks = this.loadTasks();
    if (!tasks[epicTag]) {
      throw new Error(`Epic '${epicTag}' not found`);
    }

    const state = this.loadWorkflowState();
    state.active_epic = epicTag;
    state.last_updated = new Date().toISOString();
    this.saveWorkflowState(state);

    return epicTag;
  }

  /**
   * List all epic tags
   */
  listTags() {
    const tasks = this.loadTasks();
    const tags = Object.keys(tasks);
    const activeEpic = this.getActiveEpic();

    return tags.map(tag => ({
      tag,
      active: tag === activeEpic,
      taskCount: tasks[tag].tasks ? tasks[tag].tasks.length : 0
    }));
  }

  /**
   * List tasks in active epic
   */
  listTasks(options = {}) {
    const activeEpic = this.getActiveEpic();
    if (!activeEpic) {
      throw new Error('No active epic. Run: bmad-tm use-tag <epic-tag>');
    }

    const tasks = this.loadTasks();
    const epic = tasks[activeEpic];
    if (!epic || !epic.tasks) {
      return [];
    }

    let taskList = epic.tasks;

    // Filter by status if provided
    if (options.status) {
      taskList = taskList.filter(t => t.status === options.status);
    }

    return taskList.map(task => ({
      id: task.id,
      title: task.title,
      status: task.status || 'pending',
      complexity: task.complexity || 0,
      priority: task.priority || 'medium',
      dependencies: task.dependencies || []
    }));
  }

  /**
   * Show detailed task information
   */
  showTask(taskId) {
    const activeEpic = this.getActiveEpic();
    if (!activeEpic) {
      throw new Error('No active epic. Run: bmad-tm use-tag <epic-tag>');
    }

    const tasks = this.loadTasks();
    const epic = tasks[activeEpic];
    const task = epic.tasks.find(t => t.id === taskId || t.id === String(taskId));

    if (!task) {
      throw new Error(`Task ${taskId} not found in epic '${activeEpic}'`);
    }

    return task;
  }

  /**
   * Update task status
   */
  setStatus(taskId, status) {
    const validStatuses = ['pending', 'in-progress', 'done', 'review', 'blocked', 'deferred', 'cancelled'];
    if (!validStatuses.includes(status)) {
      throw new Error(`Invalid status: ${status}. Valid: ${validStatuses.join(', ')}`);
    }

    const activeEpic = this.getActiveEpic();
    if (!activeEpic) {
      throw new Error('No active epic. Run: bmad-tm use-tag <epic-tag>');
    }

    const allTasks = this.loadTasks();
    const epic = allTasks[activeEpic];
    const task = epic.tasks.find(t => t.id === taskId || t.id === String(taskId));

    if (!task) {
      throw new Error(`Task ${taskId} not found in epic '${activeEpic}'`);
    }

    task.status = status;
    task.updated_at = new Date().toISOString();

    this.saveTasks(allTasks);

    return task;
  }

  /**
   * Find next available task (dependencies met, status pending)
   */
  findNext() {
    const activeEpic = this.getActiveEpic();
    if (!activeEpic) {
      throw new Error('No active epic. Run: bmad-tm use-tag <epic-tag>');
    }

    const tasks = this.loadTasks();
    const epic = tasks[activeEpic];

    if (!epic || !epic.tasks) {
      return null;
    }

    // Find pending tasks with all dependencies met
    for (const task of epic.tasks) {
      if (task.status !== 'pending') {
        continue;
      }

      // Check dependencies
      const dependencies = task.dependencies || [];
      const allDepsMet = dependencies.every(depId => {
        const depTask = epic.tasks.find(t => t.id === depId || t.id === String(depId));
        return depTask && depTask.status === 'done';
      });

      if (allDepsMet) {
        return task;
      }
    }

    return null;
  }

  /**
   * Get task statistics
   */
  getStats() {
    const activeEpic = this.getActiveEpic();
    if (!activeEpic) {
      throw new Error('No active epic. Run: bmad-tm use-tag <epic-tag>');
    }

    const tasks = this.loadTasks();
    const epic = tasks[activeEpic];

    if (!epic || !epic.tasks) {
      return {
        total: 0,
        pending: 0,
        inProgress: 0,
        done: 0,
        blocked: 0
      };
    }

    const stats = {
      total: epic.tasks.length,
      pending: 0,
      inProgress: 0,
      done: 0,
      blocked: 0,
      totalComplexity: 0
    };

    epic.tasks.forEach(task => {
      const status = task.status || 'pending';
      if (status === 'pending') stats.pending++;
      else if (status === 'in-progress') stats.inProgress++;
      else if (status === 'done') stats.done++;
      else if (status === 'blocked') stats.blocked++;

      stats.totalComplexity += task.complexity || 0;
    });

    return stats;
  }
}

// CLI Interface
if (require.main === module) {
  const tm = new TaskManager();
  const command = process.argv[2];
  const args = process.argv.slice(3);

  try {
    let result;

    switch (command) {
      case 'tags':
      case 'list-tags':
        result = tm.listTags();
        const activeTag = result.find(t => t.active);
        console.log('\nEpic Tags:');
        result.forEach(({ tag, active, taskCount }) => {
          const marker = active ? '→' : ' ';
          console.log(`  ${marker} ${tag} (${taskCount} tasks)`);
        });
        if (activeTag) {
          console.log(`\nActive: ${activeTag.tag}`);
        }
        break;

      case 'use-tag':
        if (!args[0]) {
          console.error('Usage: bmad-tm use-tag <epic-tag>');
          process.exit(1);
        }
        tm.setActiveEpic(args[0]);
        console.log(`✓ Switched to epic: ${args[0]}`);
        break;

      case 'list':
        const status = args[0] && args[0].startsWith('--status=')
          ? args[0].split('=')[1]
          : null;
        result = tm.listTasks(status ? { status } : {});

        if (result.length === 0) {
          console.log('No tasks found');
        } else {
          console.log(`\nTasks in ${tm.getActiveEpic()}:\n`);
          result.forEach(task => {
            const statusIcon = {
              'pending': '○',
              'in-progress': '◐',
              'done': '●',
              'blocked': '✖',
              'review': '◔'
            }[task.status] || '○';

            console.log(`  ${statusIcon} Task ${task.id}: ${task.title}`);
            console.log(`    Status: ${task.status} | Complexity: ${task.complexity} | Priority: ${task.priority}`);
            if (task.dependencies.length > 0) {
              console.log(`    Dependencies: ${task.dependencies.join(', ')}`);
            }
            console.log('');
          });
        }
        break;

      case 'show':
        if (!args[0]) {
          console.error('Usage: bmad-tm show <task-id>');
          process.exit(1);
        }
        result = tm.showTask(args[0]);
        console.log(`\nTask ${result.id}: ${result.title}\n`);
        console.log(`Status: ${result.status || 'pending'}`);
        console.log(`Complexity: ${result.complexity || 0}`);
        console.log(`Priority: ${result.priority || 'medium'}`);
        if (result.dependencies && result.dependencies.length > 0) {
          console.log(`Dependencies: ${result.dependencies.join(', ')}`);
        }
        console.log(`\nDescription:\n${result.description || 'No description'}`);
        if (result.details) {
          console.log(`\nDetails:\n${result.details}`);
        }
        if (result.testStrategy) {
          console.log(`\nTest Strategy:\n${result.testStrategy}`);
        }
        break;

      case 'set-status':
        if (!args[0] || !args[1]) {
          console.error('Usage: bmad-tm set-status <task-id> <status>');
          console.error('Valid statuses: pending, in-progress, done, review, blocked, deferred, cancelled');
          process.exit(1);
        }
        result = tm.setStatus(args[0], args[1]);
        console.log(`✓ Task ${result.id} status updated to: ${result.status}`);
        break;

      case 'next':
        result = tm.findNext();
        if (result) {
          console.log(`\nNext available task:\n`);
          console.log(`Task ${result.id}: ${result.title}`);
          console.log(`Complexity: ${result.complexity || 0}`);
          console.log(`Priority: ${result.priority || 'medium'}`);
          console.log(`\nRun: bmad-tm show ${result.id}`);
        } else {
          console.log('No tasks available (all tasks are blocked or complete)');
        }
        break;

      case 'stats':
        result = tm.getStats();
        console.log(`\nTask Statistics for ${tm.getActiveEpic()}:\n`);
        console.log(`Total Tasks: ${result.total}`);
        console.log(`  ○ Pending: ${result.pending}`);
        console.log(`  ◐ In Progress: ${result.inProgress}`);
        console.log(`  ● Done: ${result.done}`);
        console.log(`  ✖ Blocked: ${result.blocked}`);
        console.log(`\nTotal Complexity: ${result.totalComplexity}`);
        break;

      default:
        console.log(`
Simple Task Manager for BMAD-TM

Core Commands (fast, no dependencies):
  tags                      List all epic tags
  use-tag <tag>            Switch to epic
  list [--status=<status>]  List tasks in active epic
  show <id>                Show task details
  set-status <id> <status> Update task status
  next                     Find next available task
  stats                    Show task statistics

AI-Powered Commands (use task-master CLI):
  task-master parse-prd <file> --tag=<tag>    Parse PRD into tasks
  task-master analyze-complexity              Analyze task complexity
  task-master expand --id=<id>                Expand task into subtasks
  task-master research "<query>"              AI research

Examples:
  bmad-tm tags                      # List all epics
  bmad-tm use-tag epic-1-auth       # Switch to epic
  bmad-tm list                      # List tasks
  bmad-tm next                      # Find next task
  bmad-tm show 3                    # Show task 3
  bmad-tm set-status 3 in-progress  # Start task 3
  bmad-tm set-status 3 done         # Complete task 3
        `);
        process.exit(command ? 1 : 0);
    }

    process.exit(0);
  } catch (error) {
    console.error(`Error: ${error.message}`);
    process.exit(1);
  }
}

module.exports = TaskManager;
