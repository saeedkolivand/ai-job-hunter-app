import type { AutopilotRunState } from '@/lib/machines/autopilot-run.machine';

export interface StepLog {
  step: string;
  detail: string;
  ts: number;
}

export type RunStateMap = Record<string, AutopilotRunState>;
export type StepLogMap = Record<string, StepLog[]>;
