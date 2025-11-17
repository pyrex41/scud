/**
 * Task operation tools - working with individual tasks
 */

import type {
  CallToolRequest,
  CallToolResult,
  Tool,
} from '@modelcontextprotocol/sdk/types.js';
import { executeScudCommand } from '../utils/exec.js';

export const TASK_TOOLS: Tool[] = [
  {
    name: 'scud_show',
    description: 'Show detailed information about a specific task.',
    inputSchema: {
      type: 'object',
      properties: {
        task_id: {
          type: 'string',
          description: 'The task ID to show details for (e.g., "TASK-1")',
        },
      },
      required: ['task_id'],
    },
  },
  {
    name: 'scud_set_status',
    description: 'Update the status of a task.',
    inputSchema: {
      type: 'object',
      properties: {
        task_id: {
          type: 'string',
          description: 'The task ID to update',
        },
        status: {
          type: 'string',
          description: 'New status for the task',
          enum: ['pending', 'in-progress', 'done', 'review', 'blocked', 'deferred', 'cancelled'],
        },
      },
      required: ['task_id', 'status'],
    },
  },
];

export async function handleTaskTool(
  request: CallToolRequest
): Promise<CallToolResult> {
  const { name, arguments: args } = request.params;

  switch (name) {
    case 'scud_show': {
      if (!args?.task_id) {
        return {
          content: [{
            type: 'text',
            text: 'Error: task_id is required',
          }],
          isError: true,
        };
      }

      const result = await executeScudCommand(['show', args.task_id as string]);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error showing task: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout,
        }],
      };
    }

    case 'scud_set_status': {
      if (!args?.task_id || !args?.status) {
        return {
          content: [{
            type: 'text',
            text: 'Error: task_id and status are required',
          }],
          isError: true,
        };
      }

      const result = await executeScudCommand([
        'set-status',
        args.task_id as string,
        args.status as string,
      ]);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error setting task status: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || `Task ${args.task_id} status updated to ${args.status}`,
        }],
      };
    }

    default:
      return {
        content: [{
          type: 'text',
          text: `Unknown task tool: ${name}`,
        }],
        isError: true,
      };
  }
}
