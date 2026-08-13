import type { AgentConfirmRequest, AgentRunRequest } from '../../schemas/index.js';

/**
 * The agentic flows. `run` starts the background loop and returns its job id
 * immediately; progress streams as `agent:step` events (subscribe via `onStep`)
 * and the run finishes as a `jobs:event`. When the agent wants to perform a write
 * it SUSPENDS and emits a `confirm_request` step — the renderer resolves it with
 * `confirm` (approve / edit-then-approve / deny).
 *
 * `AgentRunRequest.kind` picks WHICH flow runs (`AGENT_FLOW_KINDS`, defaulting to
 * `prep_application`): prepare a whole application, or review the résumé already
 * generated for the job and offer targeted fixes. The flow's prompt, tool
 * whitelist and spend budget are backend constants — the request selects one of
 * two fixed shapes, it does not describe one.
 *
 * The two flows differ in what a run emits, which matters to anything mapping
 * steps to UI state: the prep flow's tool names are the research/drafting set and
 * it can suspend on `save_cover_letter` AND `save_resume`; the improve flow's are
 * `get_quality_report`, `validate_resume`, `search_candidate_evidence`,
 * optionally `get_trim_suggestions` or `run_quality_pipeline`, and it suspends
 * only on `save_resume`.
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
