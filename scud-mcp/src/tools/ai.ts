/**
 * AI-powered tools - require ANTHROPIC_API_KEY
 */

import type {
  CallToolRequest,
  CallToolResult,
  Tool,
} from '@modelcontextprotocol/sdk/types.js';
import { executeScudCommand } from '../utils/exec.js';

export const AI_TOOLS: Tool[] = [
  {
    name: 'scud_parse_prd',
    description: 'Parse a PRD markdown file into tasks using AI. Requires ANTHROPIC_API_KEY environment variable.',
    inputSchema: {
      type: 'object',
      properties: {
        file: {
          type: 'string',
          description: 'Path to PRD markdown file (e.g., "docs/phases/phase-1-auth.md")',
        },
        tag: {
          type: 'string',
          description: 'Phase tag to create (e.g., "phase-1-auth")',
        },
      },
      required: ['file', 'tag'],
    },
  },
  {
    name: 'scud_analyze_complexity',
    description: 'Analyze task complexity using AI. Returns Fibonacci complexity score (1,2,3,5,8,13,21) with reasoning. Requires ANTHROPIC_API_KEY.',
    inputSchema: {
      type: 'object',
      properties: {
        task: {
          type: 'string',
          description: 'Specific task ID to analyze (analyzes all tasks if not provided)',
        },
      },
    },
  },
  {
    name: 'scud_expand',
    description: 'Break down complex tasks (>13 complexity) into smaller subtasks using AI. Requires ANTHROPIC_API_KEY.',
    inputSchema: {
      type: 'object',
      properties: {
        task_id: {
          type: 'string',
          description: 'Task ID to expand (expands all tasks >13 if not provided)',
        },
        all: {
          type: 'boolean',
          description: 'Expand all tasks with complexity > 13',
          default: false,
        },
      },
    },
  },
  {
    name: 'scud_research',
    description: 'Perform AI-powered research on a topic and save findings. Requires ANTHROPIC_API_KEY.',
    inputSchema: {
      type: 'object',
      properties: {
        query: {
          type: 'string',
          description: 'Research query or question',
        },
      },
      required: ['query'],
    },
  },
];

export async function handleAITool(
  request: CallToolRequest
): Promise<CallToolResult> {
  const { name, arguments: args } = request.params;

  // Check for API key
  if (!process.env.ANTHROPIC_API_KEY) {
    return {
      content: [{
        type: 'text',
        text: 'Error: ANTHROPIC_API_KEY environment variable not set. AI tools require this API key.',
      }],
      isError: true,
    };
  }

  switch (name) {
    case 'scud_parse_prd': {
      if (!args?.file || !args?.tag) {
        return {
          content: [{
            type: 'text',
            text: 'Error: file and tag are required',
          }],
          isError: true,
        };
      }

      const result = await executeScudCommand([
        'parse-prd',
        args.file as string,
        '--tag',
        args.tag as string,
      ]);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error parsing PRD: ${result.stderr || result.stdout}`,
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

    case 'scud_analyze_complexity': {
      const cmdArgs = ['analyze-complexity'];
      if (args?.task) {
        cmdArgs.push('--task', args.task as string);
      }

      const result = await executeScudCommand(cmdArgs);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error analyzing complexity: ${result.stderr || result.stdout}`,
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

    case 'scud_expand': {
      const cmdArgs = ['expand'];

      if (args?.task_id) {
        cmdArgs.push(args.task_id as string);
      } else if (args?.all) {
        cmdArgs.push('--all');
      }

      const result = await executeScudCommand(cmdArgs);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error expanding tasks: ${result.stderr || result.stdout}`,
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

    case 'scud_research': {
      if (!args?.query) {
        return {
          content: [{
            type: 'text',
            text: 'Error: query is required',
          }],
          isError: true,
        };
      }

      const result = await executeScudCommand([
        'research',
        args.query as string,
      ]);

      if (result.exitCode !== 0) {
        return {
          content: [{
            type: 'text',
            text: `Error performing research: ${result.stderr || result.stdout}`,
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

    default:
      return {
        content: [{
          type: 'text',
          text: `Unknown AI tool: ${name}`,
        }],
        isError: true,
      };
  }
}
