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

export type Priority = 'high' | 'medium' | 'low';

export interface ScudPhase {
  tag: string;
  tasks: ScudTask[];
}

export interface WorkflowState {
  active_phase?: string;
  current_stage: string;
  stages: Record<string, StageInfo>;
  completed_phases: CompletedPhase[];
}

export interface StageInfo {
  status: string;
  started_at?: string;
  completed_at?: string;
}

export interface CompletedPhase {
  tag: string;
  completed_at: string;
  total_tasks: number;
  total_complexity: number;
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
