import type { AgentStepEvent } from '@ajh/shared';

import { createMachine } from '@/lib/machine';

/**
 * Agent ("Prep this application") execution state machine.
 *
 * Tracks the lifecycle of a single `agent.run`: the agent plans, researches
 * the company, scores the résumé match, drafts a cover letter + interview
 * questions, then ends by PROPOSING a status update. When the agent wants to
 * perform a Write action (e.g. `save_cover_letter`) it SUSPENDS — the run
 * blocks until the user approves/denies via the Phase-3 confirm gate.
 *
 *   idle → planning ⇄ researching ⇄ matching ⇄ drafting ⇄ proposing → done
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
  busyStates: ['planning', 'researching', 'matching', 'drafting', 'proposing', 'confirming'],
  errorStates: ['error'],
});

/**
 * Map a streamed `agent:step` event to an {@link AgentRunEvent}. A
 * `confirm_request` step always maps to `CONFIRM_REQUEST` (the run is
 * suspended, awaiting `APPROVE`/`DENY` from the confirm UI). The terminal
 * `proposal` step always maps to `PROPOSE`. A `turn` step is keyed by its
 * `tools[]` (research → match → draft, where `draft_cover_letter`,
 * `draft_resume`, and `suggest_interview_questions` all fall under the single
 * `drafting` machine state — the panel's checklist tracks those separately
 * from the raw step log). A plan-only turn (no tool calls yet) maps to `null`
 * — the caller stays in whatever busy state it's already in. `CANCEL` is
 * never produced here — it's driven by the `jobs:event` `job.cancelled` type,
 * which carries no `tools`/`kind` shape to key off of.
 */
export function stepToEvent(step: AgentStepEvent): AgentRunEvent | null {
  if (step.kind === 'confirm_request') return 'CONFIRM_REQUEST';
  if (step.kind === 'proposal') return 'PROPOSE';
  if (step.tools.includes('research_company')) return 'RESEARCH';
  if (step.tools.includes('match_resume')) return 'MATCH';
  if (
    step.tools.includes('draft_cover_letter') ||
    step.tools.includes('draft_resume') ||
    step.tools.includes('suggest_interview_questions')
  ) {
    return 'DRAFT';
  }
  return null;
}
