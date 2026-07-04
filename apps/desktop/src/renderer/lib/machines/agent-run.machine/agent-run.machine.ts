import type { AgentStepEvent } from '@ajh/shared';

import { createMachine } from '@/lib/machine';

/**
 * Agent ("Prep this application") execution state machine.
 *
 * Tracks the lifecycle of a single `agent.run` (Phase 2): the agent plans,
 * researches the company, scores the résumé match, drafts a cover letter +
 * interview questions, then ends by PROPOSING a status update (display-only —
 * no write executes until a Phase-3 confirm gate).
 *
 *   idle → planning → researching → matching → drafting → proposing → done
 *                                                                    ↘ cancelled
 *                                                                    ↘ error
 *
 * Busy states loop back into each other (`BUSY_TRANSITIONS`) because a turn's
 * `tools[]` can revisit an earlier tool (e.g. the model re-researches before
 * drafting), mirroring `autopilotRunMachine`'s self-looping `ranking` state.
 * `job.cancelled` routes to its own `cancelled` state (not `error`) — a
 * deliberate Stop is not a failure and gets its own copy. Every terminal state
 * (`done`/`cancelled`/`error`) also accepts `START` directly (in addition to
 * `RESET`) so clicking Retry actually restarts the run instead of leaving the
 * machine stuck at its terminal state while a new `agent.run` is already
 * in flight underneath it.
 */

export type AgentRunState =
  | 'idle'
  | 'planning'
  | 'researching'
  | 'matching'
  | 'drafting'
  | 'proposing'
  | 'done'
  | 'cancelled'
  | 'error';

export type AgentRunEvent =
  'START' | 'RESEARCH' | 'MATCH' | 'DRAFT' | 'PROPOSE' | 'COMPLETE' | 'CANCEL' | 'ERROR' | 'RESET';

const BUSY_TRANSITIONS = {
  RESEARCH: 'researching',
  MATCH: 'matching',
  DRAFT: 'drafting',
  PROPOSE: 'proposing',
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
    done: { ...TERMINAL_TRANSITIONS },
    cancelled: { ...TERMINAL_TRANSITIONS },
    error: { ...TERMINAL_TRANSITIONS },
  },
  busyStates: ['planning', 'researching', 'matching', 'drafting', 'proposing'],
  errorStates: ['error'],
});

/**
 * Map a streamed `agent:step` event to an {@link AgentRunEvent}. A `turn` step
 * is keyed by its `tools[]` (research → match → draft, where BOTH
 * `draft_cover_letter` and `suggest_interview_questions` fall under the single
 * `drafting` machine state — the panel's checklist tracks those two
 * separately from the raw step log). The terminal `proposal` step always maps
 * to `PROPOSE`. A plan-only turn (no tool calls yet) maps to `null` — the
 * caller stays in whatever busy state it's already in. `CANCEL` is never
 * produced here — it's driven by the `jobs:event` `job.cancelled` type, which
 * carries no `tools`/`kind` shape to key off of.
 */
export function stepToEvent(step: AgentStepEvent): AgentRunEvent | null {
  if (step.kind === 'proposal') return 'PROPOSE';
  if (step.tools.includes('research_company')) return 'RESEARCH';
  if (step.tools.includes('match_resume')) return 'MATCH';
  if (
    step.tools.includes('draft_cover_letter') ||
    step.tools.includes('suggest_interview_questions')
  ) {
    return 'DRAFT';
  }
  return null;
}
