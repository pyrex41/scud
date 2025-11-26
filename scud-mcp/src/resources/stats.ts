/**
 * Statistics resources - provides read-only access to phase statistics
 */

import type {
  ReadResourceRequest,
  ReadResourceResult,
  Resource,
} from '@modelcontextprotocol/sdk/types.js';
import { executeScudCommand } from '../utils/exec.js';

export const STATS_RESOURCES: Resource[] = [
  {
    uri: 'scud://stats/phase',
    name: 'Phase statistics',
    description: 'Read statistics for the active phase (task counts, complexity breakdown)',
    mimeType: 'text/plain',
  },
];

export async function handleStatsResource(
  request: ReadResourceRequest
): Promise<ReadResourceResult> {
  const { uri } = request.params;

  if (uri === 'scud://stats/phase') {
    try {
      const result = await executeScudCommand(['stats']);

      if (result.exitCode !== 0) {
        return {
          contents: [{
            uri,
            mimeType: 'text/plain',
            text: `Error reading stats: ${result.stderr || result.stdout}`,
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
          text: `Error reading stats: ${error.message}`,
        }],
      };
    }
  }

  throw new Error(`Unknown stats resource: ${uri}`);
}
