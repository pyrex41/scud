/**
 * Phase management tools - working with phase tags
 */

import type {
  CallToolRequest,
  CallToolResult,
  Tool,
} from '@modelcontextprotocol/sdk/types.js';
import { executeScudCommand } from '../utils/exec.js';

export const PHASE_TOOLS: Tool[] = [
  {
    name: 'scud_tags',
    description: 'List all available phase tags in the project.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'scud_use_tag',
    description: 'Set the active phase tag to work with.',
    inputSchema: {
      type: 'object',
      properties: {
        tag: {
          type: 'string',
          description: 'The phase tag to activate (e.g., "phase-1-auth")',
        },
      },
      required: ['tag'],
    },
  },
];

export async function handlePhaseTool(
  request: CallToolRequest
): Promise<CallToolResult> {
  const { name, arguments: args } = request.params;

  switch (name) {
    case 'scud_tags': {
      const result = await executeScudCommand(['tags']);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error listing tags: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || 'No phase tags found',
        }],
      };
    }

    case 'scud_use_tag': {
      if (!args?.tag) {
        return {
          content: [{
            type: 'text',
            text: 'Error: tag is required',
          }],
          isError: true,
        };
      }

      const result = await executeScudCommand(['use-tag', args.tag as string]);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error setting active tag: ${result.stderr || result.stdout}`,
          }],
          isError: true,
        };
      }

      return {
        content: [{
          type: 'text',
          text: result.stdout || `Active phase set to: ${args.tag}`,
        }],
      };
    }

    default:
      return {
        content: [{
          type: 'text',
          text: `Unknown phase tool: ${name}`,
        }],
        isError: true,
      };
  }
}
