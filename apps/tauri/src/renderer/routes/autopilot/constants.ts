export interface StepLog {
  step: string;
  detail: string;
  ts: number;
}

export type RunStateMap = Record<
  string,
  import('@/lib/machines/autopilot-run.machine').AutopilotRunState
>;
export type StepLogMap = Record<string, StepLog[]>;
