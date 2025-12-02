/**
 * Tasks resources - provides read-only access to tasks
 * Uses scud CLI to read task data (SCG format is authoritative)
 */

import type {
  ReadResourceRequest,
  ReadResourceResult,
  Resource,
} from '@modelcontextprotocol/sdk/types.js';
import { executeScudCommand } from '../utils/exec.js';

export const TASK_RESOURCES: Resource[] = [
  {
    uri: 'scud://tasks/list',
    name: 'All tasks in active phase',
    description: 'Read all tasks for the currently active phase',
    mimeType: 'application/json',
  },
];

export async function handleTaskResource(
  request: ReadResourceRequest
): Promise<ReadResourceResult> {
  const { uri } = request.params;

  if (uri === 'scud://tasks/list') {
    try {
      // Use scud CLI to get task list (SCG is authoritative format)
      const result = await executeScudCommand(['list']);

      if (result.exitCode !== 0) {
        return {
          contents: [{
            uri,
            mimeType: 'text/plain',
            text: `Error reading tasks: ${result.stderr || result.stdout}`,
          }],
        };
      }

      return {
        contents: [{
          uri,
          mimeType: 'text/plain',
          text: result.stdout,
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
