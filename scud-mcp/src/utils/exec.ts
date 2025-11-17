/**
 * CLI execution wrapper for SCUD commands
 */

import { exec } from 'child_process';
import { promisify } from 'util';
import type { ScudCommandResult } from '../types.js';

const execAsync = promisify(exec);

export interface ExecOptions {
  cwd?: string;
  timeout?: number;
}

/**
 * Execute a SCUD CLI command and return the result
 */
export async function executeScudCommand(
  args: string[],
  options?: ExecOptions
): Promise<ScudCommandResult> {
  const command = `scud ${args.join(' ')}`;

  try {
    const { stdout, stderr } = await execAsync(command, {
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
