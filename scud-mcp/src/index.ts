#!/usr/bin/env node

/**
 * SCUD MCP Server - Model Context Protocol server for SCUD task management
 *
 * This server wraps the SCUD CLI and exposes it through the MCP protocol,
 * enabling AI assistants like Claude to interact with SCUD task management.
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  ListResourcesRequestSchema,
  ReadResourceRequestSchema,
} from '@modelcontextprotocol/sdk/types.js';

// Import all tool handlers
import { CORE_TOOLS, handleCoreTool } from './tools/core.js';
import { PHASE_TOOLS, handlePhaseTool } from './tools/phase.js';
import { TASK_TOOLS, handleTaskTool } from './tools/task.js';
import { AI_TOOLS, handleAITool } from './tools/ai.js';
import { PARALLEL_TOOLS, handleParallelTool } from './tools/parallel.js';

// Import all resource handlers
import { TASK_RESOURCES, handleTaskResource } from './resources/tasks.js';
import { STATS_RESOURCES, handleStatsResource } from './resources/stats.js';

import { checkScudAvailable } from './utils/exec.js';

// Combine all tools
const ALL_TOOLS = [
  ...CORE_TOOLS,
  ...PHASE_TOOLS,
  ...TASK_TOOLS,
  ...AI_TOOLS,
  ...PARALLEL_TOOLS,
];

// Combine all resources
const ALL_RESOURCES = [
  ...TASK_RESOURCES,
  ...STATS_RESOURCES,
];

// Create MCP server
const server = new Server(
  {
    name: 'scud-mcp',
    version: '1.0.0',
  },
  {
    capabilities: {
      tools: {},
      resources: {},
    },
  }
);

// List available tools
server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: ALL_TOOLS,
  };
});

// Handle tool execution
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const toolName = request.params.name;

  // Route to appropriate handler based on tool name
  if (CORE_TOOLS.some(t => t.name === toolName)) {
    return handleCoreTool(request);
  }

  if (PHASE_TOOLS.some(t => t.name === toolName)) {
    return handlePhaseTool(request);
  }

  if (TASK_TOOLS.some(t => t.name === toolName)) {
    return handleTaskTool(request);
  }

  if (AI_TOOLS.some(t => t.name === toolName)) {
    return handleAITool(request);
  }

  if (PARALLEL_TOOLS.some(t => t.name === toolName)) {
    return handleParallelTool(request);
  }

  // Unknown tool
  return {
    content: [{
      type: 'text',
      text: `Unknown tool: ${toolName}`,
    }],
    isError: true,
  };
});

// List available resources
server.setRequestHandler(ListResourcesRequestSchema, async () => {
  return {
    resources: ALL_RESOURCES,
  };
});

// Handle resource reads
server.setRequestHandler(ReadResourceRequestSchema, async (request) => {
  const uri = request.params.uri;

  // Route to appropriate handler based on URI
  if (uri.startsWith('scud://tasks/')) {
    return handleTaskResource(request);
  }

  if (uri.startsWith('scud://stats/')) {
    return handleStatsResource(request);
  }

  // Unknown resource
  throw new Error(`Unknown resource URI: ${uri}`);
});

// Start server
async function main() {
  // Check if SCUD CLI is available
  const isAvailable = await checkScudAvailable();
  if (!isAvailable) {
    console.error('Error: SCUD CLI not found in PATH');
    console.error('Please install SCUD first: npm install -g scud');
    process.exit(1);
  }

  // Start MCP server with stdio transport
  const transport = new StdioServerTransport();
  await server.connect(transport);

  console.error('SCUD MCP server started successfully');
  console.error(`Exposing ${ALL_TOOLS.length} tools and ${ALL_RESOURCES.length} resources`);
}

main().catch((error) => {
  console.error('Fatal error starting SCUD MCP server:', error);
  process.exit(1);
});
