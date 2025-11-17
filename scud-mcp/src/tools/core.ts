/**
 * Core SCUD tools - basic commands that don't require AI
 */

import type { Server } from '@modelcontextprotocol/sdk/server/index.js';
import type {
  CallToolRequest,
  CallToolResult,
  Tool,
} from '@modelcontextprotocol/sdk/types.js';
import { executeScudCommand } from '../utils/exec.js';

export const CORE_TOOLS: Tool[] = [
  {
    name: 'scud_init',
    description: 'Initialize SCUD in the current directory. Creates .taskmaster/ directory structure.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'scud_list',
    description: 'List all tasks in the active epic. Optionally filter by status.',
    inputSchema: {
      type: 'object',
      properties: {
        status: {
          type: 'string',
          description: 'Filter by task status',
          enum: ['pending', 'in-progress', 'done', 'review', 'blocked', 'deferred', 'cancelled'],
        },
      },
    },
  },
  {
    name: 'scud_next',
    description: 'Find the next available task to work on. Respects dependencies and current status.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'scud_stats',
    description: 'Show statistics for the active epic (task counts, complexity breakdown).',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
];

export async function handleCoreTool(
  request: CallToolRequest
): Promise<CallToolResult> {
  const { name, arguments: args } = request.params;

  switch (name) {
    case 'scud_init': {
      const result = await executeScudCommand(['init']);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error initializing SCUD: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || 'SCUD initialized successfully',
        }],
      };
    }

    case 'scud_list': {
      const cmdArgs = ['list'];
      if (args?.status) {
        cmdArgs.push('--status', args.status as string);
      }

      const result = await executeScudCommand(cmdArgs);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error listing tasks: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || 'No tasks found',
        }],
      };
    }

    case 'scud_next': {
      const result = await executeScudCommand(['next']);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error finding next task: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || 'No available tasks',
        }],
      };
    }

    case 'scud_stats': {
      const result = await executeScudCommand(['stats']);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error getting stats: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || 'No statistics available',
        }],
      };
    }

    default:
      return {
        content: [{
          type: 'text',
          text: `Unknown core tool: ${name}`,
        }],
        isError: true,
      };
  }
}
