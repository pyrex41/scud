/**
 * TypeScript type definitions for SCUD MCP server
 */

export interface ScudCommandResult {
  stdout: string;
  stderr: string;
  exitCode: number;
}

export interface ScudTask {
  id: string;
  title: string;
  description: string;
  status: TaskStatus;
  priority: Priority;
  complexity: number;
  dependencies: string[];
  assigned_to?: string;
  locked_by?: string;
  locked_at?: string;
  created_at: string;
  updated_at: string;
  details?: string;
}

export type TaskStatus =
  | 'pending'
  | 'in-progress'
  | 'done'
  | 'review'
  | 'blocked'
  | 'deferred'
  | 'cancelled';

export type Priority = 'critical' | 'high' | 'medium' | 'low';

export interface ScudPhase {
  tag: string;
  tasks: ScudTask[];
}

export interface PhaseStats {
  total_tasks: number;
  by_status: Record<TaskStatus, number>;
  total_complexity: number;
  completed_complexity: number;
}

export interface PhaseGroup {
  id: string;
  name: string;
  description?: string;
  phase_tags: string[];
  created_at: string;
}
