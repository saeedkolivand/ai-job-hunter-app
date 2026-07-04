import type { AgentRunRequest } from '../../schemas/index.js';

/**
 * The "prep this application" agentic flow. `run` starts the background loop and
 * returns its job id immediately; progress streams as `agent:step` events
 * (subscribe via `onStep`) and the run finishes as a `jobs:event`.
 */
export interface AgentContract {
  run(req: AgentRunRequest): Promise<{ jobId: string }>;

  /** Subscribe to the `agent:step` narration stream. Returns an unsubscribe fn. */
  onStep(handler: (event: AgentStepEvent) => void): () => void;
}

/**
 * What kind of step this is:
 * - `turn` — a per-turn narration from inside the loop (plan text + tool calls).
 * - `proposal` — the terminal step: the agent's final answer, which proposes a
 *   status update. Display-only in Phase 2 (no write executes; a Phase-3 confirm
 *   gate will make it actionable).
 */
export type AgentStepKind = 'turn' | 'proposal';

/** Payload of the `agent:step` event (Rust `AgentStep`, camelCase). */
export interface AgentStepEvent {
  /** The `agent_run` job id this step belongs to — filter on it when more than
   *  one run can be in flight (`AGENT_RUN_CONCURRENCY_MAX`) or a panel outlives
   *  the run it started (e.g. the user switches jobs mid-run). */
  jobId: string;
  /** 1-based turn index (the terminal proposal is `steps + 1`). */
  step: number;
  /** The model's plan/answer text for this step. */
  text: string;
  /** Names of the tools the model asked to run this turn. */
  tools: string[];
  /** Names of Write tools DENIED this turn (none reachable in the prep flow). */
  denied: string[];
  /** Whether this is an in-loop turn or the terminal display-only proposal. */
  kind: AgentStepKind;
}

export const AGENT_CHANNELS = {
  run: 'agent:run',
} as const;
