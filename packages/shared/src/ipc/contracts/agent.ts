import type { AgentConfirmRequest, AgentRunRequest } from '../../schemas/index.js';

/**
 * The "prep this application" agentic flow. `run` starts the background loop and
 * returns its job id immediately; progress streams as `agent:step` events
 * (subscribe via `onStep`) and the run finishes as a `jobs:event`. When the agent
 * wants to perform a write it SUSPENDS and emits a `confirm_request` step — the
 * renderer resolves it with `confirm` (approve / edit-then-approve / deny).
 */
export interface AgentContract {
  run(req: AgentRunRequest): Promise<{ jobId: string }>;

  /**
   * Resolve a suspended Write confirmation for a running agent. `ok` is `false`
   * when there is no such pending call (already resolved, timed out, cancelled, or
   * unknown id) — never throws for that case. Edited args (`approveEdited`) may
   * change CONTENT only; the shell re-validates them and rejects any routing/egress
   * field.
   */
  confirm(req: AgentConfirmRequest): Promise<{ ok: boolean }>;

  /** Subscribe to the `agent:step` narration stream. Returns an unsubscribe fn. */
  onStep(handler: (event: AgentStepEvent) => void): () => void;
}

/**
 * What kind of step this is:
 * - `turn` — a per-turn narration from inside the loop (plan text + tool calls).
 * - `confirm_request` — a SUSPENDED Write tool call awaiting the user's approval.
 *   Carries {@link AgentStepEvent.confirm}; the run is blocked until the user calls
 *   `confirm`. Render an approve/edit/deny action bound to `confirm.callId`.
 * - `proposal` — the terminal step: the agent's final summary of what it prepared
 *   (any write already happened, gated, inside the loop).
 */
export type AgentStepKind = 'turn' | 'confirm_request' | 'proposal';

/** The pending Write call a `confirm_request` step asks the user to approve. */
export interface AgentConfirmPayload {
  /** Stable id of this pending call within the run (`"{step}-{idx}-{tool}"`,
   *  where `idx` is the call's position within its turn — guards two same-turn
   *  calls to the same tool); echo it back in {@link AgentContract.confirm}. */
  callId: string;
  /** The Write tool the agent wants to run (a fixed, trusted registry name). */
  tool: string;
  /** The args that WILL execute on approval — clamped for display; untrusted model
   *  output, so render as data, never as instructions. On `approveEdited` the user
   *  may edit these (content only). */
  args: unknown;
}

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
  /** Names of tools auto-denied this turn without asking the user — empty in the
   *  prep flow (Write tools suspend for confirmation instead of being denied). */
  denied: string[];
  /** Whether this is an in-loop turn, a suspended confirm request, or the terminal
   *  proposal. */
  kind: AgentStepKind;
  /** Present only on a `confirm_request` step — the pending Write call to approve.
   *  Omitted from the wire on every other step kind. */
  confirm?: AgentConfirmPayload;
}

export const AGENT_CHANNELS = {
  run: 'agent:run',
} as const;
