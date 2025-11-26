/**
 * Parallel development tools - phase groups and task assignments
 */

import type {
  CallToolRequest,
  CallToolResult,
  Tool,
} from '@modelcontextprotocol/sdk/types.js';
import { executeScudCommand } from '../utils/exec.js';

export const PARALLEL_TOOLS: Tool[] = [
  {
    name: 'scud_create_group',
    description: 'Create a phase group for parallel development across multiple phases.',
    inputSchema: {
      type: 'object',
      properties: {
        name: {
          type: 'string',
          description: 'Group name (e.g., "sprint-1")',
        },
        phases: {
          type: 'string',
          description: 'Comma-separated list of phase tags (e.g., "phase-1,phase-2,phase-3")',
        },
        description: {
          type: 'string',
          description: 'Optional group description',
        },
      },
      required: ['name', 'phases'],
    },
  },
  {
    name: 'scud_list_groups',
    description: 'List all phase groups.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'scud_group_status',
    description: 'Show status and progress for a phase group.',
    inputSchema: {
      type: 'object',
      properties: {
        group_id: {
          type: 'string',
          description: 'Group ID or name',
        },
      },
      required: ['group_id'],
    },
  },
  {
    name: 'scud_assign',
    description: 'Assign a task to a developer.',
    inputSchema: {
      type: 'object',
      properties: {
        task_id: {
          type: 'string',
          description: 'Task ID to assign',
        },
        assignee: {
          type: 'string',
          description: 'Developer name or username',
        },
      },
      required: ['task_id', 'assignee'],
    },
  },
  {
    name: 'scud_claim',
    description: 'Claim a task for yourself. Prevents others from working on it.',
    inputSchema: {
      type: 'object',
      properties: {
        task_id: {
          type: 'string',
          description: 'Task ID to claim',
        },
        name: {
          type: 'string',
          description: 'Your name or username',
        },
      },
      required: ['task_id', 'name'],
    },
  },
  {
    name: 'scud_release',
    description: 'Release a claimed task so others can work on it.',
    inputSchema: {
      type: 'object',
      properties: {
        task_id: {
          type: 'string',
          description: 'Task ID to release',
        },
        force: {
          type: 'boolean',
          description: 'Force release even if claimed by someone else',
          default: false,
        },
      },
      required: ['task_id'],
    },
  },
  {
    name: 'scud_whois',
    description: 'Show task assignments and who is working on what.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
];

export async function handleParallelTool(
  request: CallToolRequest
): Promise<CallToolResult> {
  const { name, arguments: args } = request.params;

  switch (name) {
    case 'scud_create_group': {
      if (!args?.name || !args?.phases) {
        return {
          content: [{
            type: 'text',
            text: 'Error: name and phases are required',
          }],
          isError: true,
        };
      }

      const cmdArgs = [
        'create-group',
        args.name as string,
        '--phases',
        args.phases as string,
      ];

      if (args.description) {
        cmdArgs.push('--description', args.description as string);
      }

      const result = await executeScudCommand(cmdArgs);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error creating group: ${result.stderr || result.stdout}`,
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

    case 'scud_list_groups': {
      const result = await executeScudCommand(['list-groups']);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error listing groups: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || 'No phase groups found',
        }],
      };
    }

    case 'scud_group_status': {
      if (!args?.group_id) {
        return {
          content: [{
            type: 'text',
            text: 'Error: group_id is required',
          }],
          isError: true,
        };
      }

      const result = await executeScudCommand(['group-status', args.group_id as string]);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error getting group status: ${result.stderr || result.stdout}`,
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

    case 'scud_assign': {
      if (!args?.task_id || !args?.assignee) {
        return {
          content: [{
            type: 'text',
            text: 'Error: task_id and assignee are required',
          }],
          isError: true,
        };
      }

      const result = await executeScudCommand([
        'assign',
        args.task_id as string,
        args.assignee as string,
      ]);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error assigning task: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || `Task ${args.task_id} assigned to ${args.assignee}`,
        }],
      };
    }

    case 'scud_claim': {
      if (!args?.task_id || !args?.name) {
        return {
          content: [{
            type: 'text',
            text: 'Error: task_id and name are required',
          }],
          isError: true,
        };
      }

      const result = await executeScudCommand([
        'claim',
        args.task_id as string,
        '--name',
        args.name as string,
      ]);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error claiming task: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || `Task ${args.task_id} claimed by ${args.name}`,
        }],
      };
    }

    case 'scud_release': {
      if (!args?.task_id) {
        return {
          content: [{
            type: 'text',
            text: 'Error: task_id is required',
          }],
          isError: true,
        };
      }

      const cmdArgs = ['release', args.task_id as string];
      if (args.force) {
        cmdArgs.push('--force');
      }

      const result = await executeScudCommand(cmdArgs);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error releasing task: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || `Task ${args.task_id} released`,
        }],
      };
    }

    case 'scud_whois': {
      const result = await executeScudCommand(['whois']);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error getting assignments: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || 'No task assignments found',
        }],
      };
    }

    default:
      return {
        content: [{
          type: 'text',
          text: `Unknown parallel tool: ${name}`,
        }],
        isError: true,
      };
  }
}
