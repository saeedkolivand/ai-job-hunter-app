import type { AgentFlowKind, AgentStepEvent } from '@ajh/shared';

import { createMachine } from '@/lib/machine';

/**
 * Agent run (`agent.run`) execution state machine — shared by every flow in
 * `AGENT_FLOW_KINDS`.
 *
 * Tracks the lifecycle of a single run. The `prep_application` flow plans,
 * researches the company, scores the résumé match, drafts a cover letter +
 * interview questions, then ends by PROPOSING a status update; the
 * `improve_resume` flow reads the last quality report, re-checks the generated
 * résumé against the candidate's own evidence, then offers the corrected text.
 * When either wants to perform a Write action (`save_cover_letter`,
 * `save_resume`) it SUSPENDS — the run blocks until the user approves/denies
 * via the Phase-3 confirm gate.
 *
 *   idle → planning ⇄ researching ⇄ matching ⇄ drafting ⇄ reviewing ⇄ proposing → done
 *                  ↘ confirming (suspended, awaiting the user) ↗
 *                                                             ↘ cancelled
 *                                                             ↘ error
 *
 * Busy states loop back into each other (`BUSY_TRANSITIONS`) because a turn's
 * `tools[]` can revisit an earlier tool (e.g. the model re-researches before
 * drafting), mirroring `autopilotRunMachine`'s self-looping `ranking` state.
 * `confirming` is reached from ANY busy state on `CONFIRM_REQUEST` (a Write
 * tool can be proposed at any point in the loop) and is itself treated as
 * busy — the run is genuinely blocked on the user, not idle. Resolving it
 * (`APPROVE`/`DENY`) returns to `planning`, a generic "busy, awaiting the next
 * turn" state — the loop resumes and the next real `turn`/`proposal` step
 * re-routes the machine via `stepToEvent` as usual. `job.cancelled` routes to
 * its own `cancelled` state (not `error`) — a deliberate Stop is not a failure
 * and gets its own copy; it's reachable from `confirming` too (Stop must still
 * work while suspended). Every terminal state (`done`/`cancelled`/`error`)
 * also accepts `START` directly (in addition to `RESET`) so clicking Retry
 * actually restarts the run instead of leaving the machine stuck at its
 * terminal state while a new `agent.run` is already in flight underneath it.
 */

export type AgentRunState =
  | 'idle'
  | 'planning'
  | 'researching'
  | 'matching'
  | 'drafting'
  /** The `improve_resume` flow's working state: reading the persisted quality
   *  report, validating the generation, weighing evidence or trims. */
  | 'reviewing'
  | 'proposing'
  | 'confirming'
  | 'done'
  | 'cancelled'
  | 'error';

export type AgentRunEvent =
  | 'START'
  | 'RESEARCH'
  | 'MATCH'
  | 'DRAFT'
  | 'REVIEW'
  | 'PROPOSE'
  | 'CONFIRM_REQUEST'
  | 'APPROVE'
  | 'DENY'
  | 'COMPLETE'
  | 'CANCEL'
  | 'ERROR'
  | 'RESET';

const BUSY_TRANSITIONS = {
  RESEARCH: 'researching',
  MATCH: 'matching',
  DRAFT: 'drafting',
  REVIEW: 'reviewing',
  PROPOSE: 'proposing',
  CONFIRM_REQUEST: 'confirming',
  COMPLETE: 'done',
  CANCEL: 'cancelled',
  ERROR: 'error',
} as const;

/** Every terminal state restarts on `START` (Retry) as well as resetting on `RESET`. */
const TERMINAL_TRANSITIONS = { START: 'planning', RESET: 'idle' } as const;

export const agentRunMachine = createMachine<AgentRunState, AgentRunEvent>({
  transitions: {
    idle: { START: 'planning', RESET: 'idle' },
    planning: { ...BUSY_TRANSITIONS },
    researching: { ...BUSY_TRANSITIONS },
    matching: { ...BUSY_TRANSITIONS },
    drafting: { ...BUSY_TRANSITIONS },
    reviewing: { ...BUSY_TRANSITIONS },
    proposing: { ...BUSY_TRANSITIONS },
    // Resolving the suspended confirm (approve or deny) resumes the loop at
    // `planning` — a generic busy state; the next streamed step re-routes via
    // `stepToEvent` as normal. Every other BUSY_TRANSITIONS entry (CANCEL,
    // ERROR, a re-entrant CONFIRM_REQUEST) still applies while suspended.
    confirming: { ...BUSY_TRANSITIONS, APPROVE: 'planning', DENY: 'planning' },
    done: { ...TERMINAL_TRANSITIONS },
    cancelled: { ...TERMINAL_TRANSITIONS },
    error: { ...TERMINAL_TRANSITIONS },
  },
  busyStates: [
    'planning',
    'researching',
    'matching',
    'drafting',
    'reviewing',
    'proposing',
    'confirming',
  ],
  errorStates: ['error'],
});

/**
 * Tool name → machine event, PER FLOW. Each flow's whitelist is its own
 * vocabulary (`agent::flows::FLOWS`), so the mapping is keyed by the run's
 * `kind` rather than by tool name alone: a tool a flow cannot call must not
 * move that flow's machine, and two flows may legitimately register the same
 * name for different phases of their own sequence.
 *
 * Entries are ORDERED, and the first one whose tool appears in the step's
 * `tools[]` wins — a turn that names two recognized tools resolves by this
 * precedence rather than by the order the model happened to emit them in.
 */
const FLOW_TOOL_EVENTS: Record<
  AgentFlowKind,
  ReadonlyArray<readonly [tool: string, event: AgentRunEvent]>
> = {
  // Research → match → draft, where `draft_cover_letter`, `draft_resume` and
  // `suggest_interview_questions` all fall under the single `drafting` state
  // (`PrepApplicationPanel`'s checklist tracks those separately, off the raw
  // step log).
  prep_application: [
    ['research_company', 'RESEARCH'],
    ['match_resume', 'MATCH'],
    ['draft_cover_letter', 'DRAFT'],
    ['draft_resume', 'DRAFT'],
    ['suggest_interview_questions', 'DRAFT'],
  ],
  // Every read tool of the review flow is one `reviewing` state: the sequence's
  // granularity lives in the panel's checklist (which keys off the step log),
  // and `validate_resume` legitimately appears TWICE in one run — the post-fix
  // re-check. Mapping both appearances onto the same self-looping busy state is
  // what makes that idempotent instead of a mis-sequence. `run_quality_pipeline`
  // is here too: it can hold ONE step for up to 90 minutes
  // (`Budget::AGENT_IMPROVE.step_timeout`), which is a healthy run, not a stall.
  improve_resume: [
    ['get_quality_report', 'REVIEW'],
    ['validate_resume', 'REVIEW'],
    ['search_candidate_evidence', 'REVIEW'],
    ['get_trim_suggestions', 'REVIEW'],
    ['run_quality_pipeline', 'REVIEW'],
  ],
};

/**
 * Map a streamed `agent:step` event to an {@link AgentRunEvent} for the flow
 * that produced it. A `confirm_request` step always maps to `CONFIRM_REQUEST`
 * (the run is suspended, awaiting `APPROVE`/`DENY` from the confirm UI) and
 * the terminal `proposal` step always maps to `PROPOSE` — both are flow-
 * independent step KINDS, which is why the gated writes (`save_cover_letter`,
 * `save_resume`) need no entry in {@link FLOW_TOOL_EVENTS}. A `turn` step is
 * keyed by its `tools[]` through that flow's map. A plan-only turn (no tool
 * calls yet), or a tool the flow does not own, maps to `null` — the caller
 * stays in whatever busy state it's already in. `CANCEL` is never produced
 * here — it's driven by the `jobs:event` `job.cancelled` type, which carries
 * no `tools`/`kind` shape to key off of.
 *
 * `kind` defaults to `prep_application` for the same reason the wire does: an
 * older caller that names no flow gets the behaviour it always had.
 */
export function stepToEvent(
  step: AgentStepEvent,
  kind: AgentFlowKind = 'prep_application'
): AgentRunEvent | null {
  if (step.kind === 'confirm_request') return 'CONFIRM_REQUEST';
  if (step.kind === 'proposal') return 'PROPOSE';
  for (const [tool, event] of FLOW_TOOL_EVENTS[kind]) {
    if (step.tools.includes(tool)) return event;
  }
  return null;
}
