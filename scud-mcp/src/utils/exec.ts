/**
 * CLI execution wrapper for SCUD commands
 *
 * Uses execFile (not exec) to avoid shell injection vulnerabilities.
 * Arguments are passed as an array, not concatenated into a shell string.
 */

import { execFile } from 'child_process';
import { promisify } from 'util';
import type { ScudCommandResult } from '../types.js';

const execFileAsync = promisify(execFile);

export interface ExecOptions {
  cwd?: string;
  timeout?: number;
}

/**
 * Execute a SCUD CLI command and return the result
 *
 * Uses execFile to safely pass arguments without shell interpolation.
 * This prevents command injection and properly handles arguments with spaces.
 */
export async function executeScudCommand(
  args: string[],
  options?: ExecOptions
): Promise<ScudCommandResult> {
  try {
    // Use execFile with argument array - no shell interpolation
    const { stdout, stderr } = await execFileAsync('scud', args, {
      cwd: options?.cwd || process.cwd(),
      timeout: options?.timeout || 30000, // 30 second default timeout
      env: {
        ...process.env,
        // Inherit ANTHROPIC_API_KEY and other env vars
      },
      // Increase buffer size for large outputs
      maxBuffer: 10 * 1024 * 1024, // 10MB
    });

    return {
      stdout: stdout.trim(),
      stderr: stderr.trim(),
      exitCode: 0,
    };
  } catch (error: any) {
    return {
      stdout: error.stdout?.trim() || '',
      stderr: error.stderr?.trim() || error.message,
      exitCode: error.code || 1,
    };
  }
}

/**
 * Parse JSON output from SCUD command
 */
export function parseJsonOutput<T>(stdout: string): T {
  try {
    return JSON.parse(stdout);
  } catch (error) {
    throw new Error(`Failed to parse SCUD output as JSON: ${error}`);
  }
}

/**
 * Check if SCUD CLI is available in PATH
 */
export async function checkScudAvailable(): Promise<boolean> {
  try {
    const result = await executeScudCommand(['--version']);
    return result.exitCode === 0;
  } catch {
    return false;
  }
}

/**
 * Validate that a command succeeded
 */
export function ensureSuccess(result: ScudCommandResult, context: string): void {
  if (result.exitCode !== 0) {
    throw new Error(
      `SCUD command failed (${context}): ${result.stderr || result.stdout}`
    );
  }
}
