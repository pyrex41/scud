/**
 * Tasks resources - provides read-only access to tasks
 */

import type {
  ReadResourceRequest,
  ReadResourceResult,
  Resource,
} from '@modelcontextprotocol/sdk/types.js';
import { existsSync } from 'fs';
import { readFile } from 'fs/promises';
import { join } from 'path';

export const TASK_RESOURCES: Resource[] = [
  {
    uri: 'scud://tasks/list',
    name: 'All tasks in active phase',
    description: 'Read all tasks for the currently active phase',
    mimeType: 'application/json',
  },
];

function resolveDataPath(...segments: string[]): string {
  const scudPath = join(process.cwd(), '.scud', ...segments);
  if (existsSync(scudPath)) {
    return scudPath;
  }
  const legacyPath = join(process.cwd(), '.taskmaster', ...segments);
  if (existsSync(legacyPath)) {
    return legacyPath;
  }
  // Prefer .scud even if it doesn't exist yet (new installs)
  return scudPath;
}

export async function handleTaskResource(
  request: ReadResourceRequest
): Promise<ReadResourceResult> {
  const { uri } = request.params;

  if (uri === 'scud://tasks/list') {
    try {
      // Read tasks file
      const tasksFile = resolveDataPath('tasks', 'tasks.json');
      const content = await readFile(tasksFile, 'utf-8');
      const allTasks = JSON.parse(content);

      // Read workflow state to get active phase
      const stateFile = resolveDataPath('workflow-state.json');
      const stateContent = await readFile(stateFile, 'utf-8');
      const state = JSON.parse(stateContent);

      if (!state.active_group) {
        return {
          contents: [{
            uri,
            mimeType: 'text/plain',
            text: 'No active phase set',
          }],
        };
      }

      // Get tasks for active phase
      const activeTasks = allTasks[state.active_group] || { tasks: [] };

      return {
        contents: [{
          uri,
          mimeType: 'application/json',
          text: JSON.stringify(activeTasks, null, 2),
        }],
      };
    } catch (error: any) {
      return {
        contents: [{
          uri,
          mimeType: 'text/plain',
          text: `Error reading tasks: ${error.message}`,
        }],
      };
    }
  }

  throw new Error(`Unknown task resource: ${uri}`);
}
