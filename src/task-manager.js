#!/usr/bin/env node

/**
 * Simple Task Manager for SCUD
 * Sprint Cycle Unified Development
 *
 * DEPRECATED: This file is kept for reference only.
 * Use the scud CLI (Rust) for all task operations.
 *
 * The Rust CLI reads SCG format directly and is the authoritative source.
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

class TaskManager {
  constructor(projectRoot = process.cwd()) {
    this.projectRoot = projectRoot;
    this.scudDir = path.join(projectRoot, '.scud');
    this.taskmasterDir = path.join(projectRoot, '.taskmaster');
  }

  /**
   * Get active tag from active-tag file
   */
  getActiveTag() {
    const activeTagFile = this.resolvePath('active-tag');
    if (!fs.existsSync(activeTagFile)) {
      return null;
    }
    return fs.readFileSync(activeTagFile, 'utf8').trim() || null;
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
   * Run scud CLI command
   */
  runScud(args) {
    try {
      const result = execSync(`scud ${args.join(' ')}`, { encoding: 'utf8' });
      return { success: true, output: result };
    } catch (error) {
      return { success: false, error: error.message };
    }
  }

  /**
   * List all tags using scud CLI
   */
  listTags() {
    return this.runScud(['tags']);
  }

  /**
   * List tasks in active tag
   */
  listTasks(options = {}) {
    const args = ['list'];
    if (options.status) {
      args.push('--status', options.status);
    }
    return this.runScud(args);
  }

  /**
   * Show task details
   */
  showTask(taskId) {
    return this.runScud(['show', taskId]);
  }

  /**
   * Update task status
   */
  setStatus(taskId, status) {
    return this.runScud(['set-status', taskId, status]);
  }

  /**
   * Find next available task
   */
  findNext() {
    return this.runScud(['next']);
  }

  /**
   * Get task statistics
   */
  getStats() {
    return this.runScud(['stats']);
  }
}

// CLI Interface
if (require.main === module) {
  console.log(`
DEPRECATED: Use the scud CLI directly instead.

Examples:
  scud tags                      # List all tags
  scud list                      # List tasks
  scud next                      # Find next task
  scud show 3                    # Show task 3
  scud set-status 3 in-progress  # Start task 3
  scud set-status 3 done         # Complete task 3
  scud stats                     # Show statistics
  `);
  process.exit(0);
}

module.exports = TaskManager;
