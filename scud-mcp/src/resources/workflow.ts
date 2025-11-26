/**
 * Workflow state resource - provides read-only access to workflow state
 */

import type {
  ReadResourceRequest,
  ReadResourceResult,
  Resource,
} from '@modelcontextprotocol/sdk/types.js';
import { existsSync } from 'fs';
import { readFile } from 'fs/promises';
import { join } from 'path';

export const WORKFLOW_RESOURCES: Resource[] = [
  {
    uri: 'scud://workflow/state',
    name: 'Current workflow state',
    description: 'Read the current workflow state including active epic and phase information',
    mimeType: 'application/json',
  },
];

function resolveStateFile(): string {
  const scudPath = join(process.cwd(), '.scud', 'workflow-state.json');
  if (existsSync(scudPath)) {
    return scudPath;
  }
  const legacyPath = join(process.cwd(), '.taskmaster', 'workflow-state.json');
  if (existsSync(legacyPath)) {
    return legacyPath;
  }
  return scudPath;
}

export async function handleWorkflowResource(
  request: ReadResourceRequest
): Promise<ReadResourceResult> {
  const { uri } = request.params;

  if (uri === 'scud://workflow/state') {
    try {
      // Read workflow state directly from file
      const stateFile = resolveStateFile();
      const content = await readFile(stateFile, 'utf-8');
      const state = JSON.parse(content);

      return {
        contents: [{
          uri,
          mimeType: 'application/json',
          text: JSON.stringify(state, null, 2),
        }],
      };
    } catch (error: any) {
      return {
        contents: [{
          uri,
          mimeType: 'text/plain',
          text: `Error reading workflow state: ${error.message}`,
        }],
      };
    }
  }

  throw new Error(`Unknown workflow resource: ${uri}`);
}
