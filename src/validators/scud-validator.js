#!/usr/bin/env node

/**
 * SCUD Validator
 *
 * Validates SCUD state and provides task information.
 * Simplified version - workflow phases removed (use task DAG instead).
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

class ScudValidator {
  constructor(projectRoot = process.cwd()) {
    this.projectRoot = projectRoot;
    this.scudDir = path.join(projectRoot, '.scud');
    this.taskmasterDir = path.join(projectRoot, '.taskmaster');
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
   * Get active tag from active-tag file
   */
  getActiveTag() {
    const activeTagFile = this.resolvePath('active-tag');
    if (!fs.existsSync(activeTagFile)) {
      return null;
    }
    return fs.readFileSync(activeTagFile, 'utf8').trim() || null;
  }

  /**
   * Validate that SCUD CLI is available
   */
  validateScudCLI() {
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
   * Get command availability (simplified - no workflow phases)
   */
  getCommandAvailability() {
    const activeTag = this.getActiveTag();

    const commands = {
      'scud:task-list': { available: true, reason: 'List tasks in active tag' },
      'scud:task-next': { available: true, reason: 'Find next available task' },
      'scud:task-show': { available: true, reason: 'Show task details' },
      'scud:task-status': { available: true, reason: 'Update task status' },
      'scud:task-claim': { available: true, reason: 'Claim/release task locks' },
    };

    if (!activeTag) {
      for (const cmd of Object.keys(commands)) {
        commands[cmd].available = false;
        commands[cmd].reason = 'No active tag set. Run: scud tags <tag-name>';
      }
    }

    return commands;
  }

  /**
   * List all tags using scud CLI
   */
  listTags() {
    try {
      const result = execSync('scud tags', { encoding: 'utf8' });
      return { valid: true, output: result };
    } catch (error) {
      return { valid: false, error: error.message };
    }
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

      case 'get-command-availability':
        result = validator.getCommandAvailability();
        break;

      case 'list-tags':
        result = validator.listTags();
        break;

      case 'get-active-tag':
        const tag = validator.getActiveTag();
        result = { valid: true, activeTag: tag };
        break;

      default:
        console.error(`Unknown command: ${command}`);
        console.log(`
Usage: scud-validator.js <command>

Commands:
  validate-cli              Check if SCUD CLI is available
  get-command-availability  Get which commands are available
  list-tags                 List all tags
  get-active-tag            Get currently active tag
        `);
        process.exit(1);
    }

    console.log(JSON.stringify(result, null, 2));
    process.exit(result.valid !== false ? 0 : 1);
  } catch (error) {
    console.error(JSON.stringify({
      valid: false,
      error: error.message
    }, null, 2));
    process.exit(1);
  }
}

module.exports = ScudValidator;
