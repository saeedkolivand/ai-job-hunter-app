import type { PipelineStageEvent } from '@ajh/shared';
import type { PipelineRunStatus } from '@ajh/shared/ipc';

import { createMachine } from '@/lib/machine';

/**
 * Staged résumé-generation (quality AND max depth) run state machine.
 *
 * One `resumePipeline.run` walks the Rust stage list for its depth — eight at
 * quality (`analyze_job`, `match_evidence`, `strategy`, `draft`,
 * `cover_letter`, `validate`, `repair`, `humanize`) and eight at max (`draft`
 * splits into `sections` + `assemble`, no `cover_letter`/`humanize`, and
 * `llm_judge` runs LAST, after `repair`) — and this machine folds either list
 * into the same coarse states:
 *
 *   idle → queued → preparing → drafting → validating → repairing → humanizing
 *                                                     ↘ done | needsReview | cancelled | error
 *
 * The three JSON extraction stages collapse into one `preparing` state because
 * they are indistinguishable to a user (nothing streams, no document exists
 * yet); `index`/`total` off the event carries the real granularity into the
 * progress dots. `stageToEvent` keys on the stage NAME, not on its index,
 * precisely because the two depths run different lists.
 *
 * **`cover_letter` folds into `drafting`** (PR-2) — it is a second streamed
 * pass writing a second document, the same "the document is being written"
 * meaning `sections`/`assemble` already share with `draft`. **`humanize` gets
 * its OWN state, `humanizing`** — it is quality depth's own LAST stage, a
 * distinct "removing AI signs" phase the stepper UI (PR-3) shows as its own
 * step, unlike `cover_letter` which has no separate row.
 *
 * **A max run still ends in `validating`, not `repairing` or `humanizing`** —
 * the judge is a review pass over the finished document and reads as checking,
 * max depth has no `humanize` stage at all, and the ordering is the backend's
 * (`MAX_STAGES`), not this machine's opinion. Nothing downstream depends on
 * which busy state the last stage leaves behind: every busy state is equally
 * non-terminal, which is the invariant below.
 *
 * ## The one rule this machine exists to enforce
 *
 * **No stage event is terminal.** {@link stageToEvent} can never return
 * `COMPLETE`/`NEEDS_REVIEW`, and the last stage's `finish` leaves the machine in
 * a BUSY state (`repairing` at quality depth, `validating` at max). Two
 * independent reasons, both load-bearing:
 *
 * 1. The draft stage streams under the run's own umbrella `jobId`, so the shared
 *    stream machinery fires `job_complete` (and resolves `awaitAiStream`) the
 *    moment the draft's last delta lands — with validation and up to two repair
 *    rounds still to come. A renderer that treats that as "done" shows an
 *    unvalidated, unrepaired draft as final.
 * 2. A run stopped at a stage BOUNDARY (cancel, run deadline) emits no event at
 *    all for the stage it refused to start — `RunHooks::before` returns `Err`
 *    before its `start` emit — so "the last stage finished" is not even reliably
 *    observable.
 *
 * The authority on how a run ended is the run RECORD: feed
 * {@link statusToEvent}(detail.status) in and let it drive the terminal
 * transition. `stoppedReason` is explanatory only and may be absent on a
 * terminal run.
 */

/** The live progress of ONE stage, straight off the `pipeline:stage` payload. */
export interface PipelineStageProgress {
  stage: string;
  phase: PipelineStageEvent['phase'];
  /** 0-based position of this stage in the pipeline. */
  index: number;
  total: number;
  attempt: number;
  issueCount?: number;
  criticalCount?: number;
}

export type ResumePipelineState =
  | 'idle'
  | 'queued'
  | 'preparing'
  | 'drafting'
  | 'validating'
  | 'repairing'
  | 'humanizing'
  | 'done'
  | 'needsReview'
  | 'cancelled'
  | 'error';

export type ResumePipelineEvent =
  | 'START'
  | 'QUEUED'
  | 'PREPARE'
  | 'DRAFT'
  | 'VALIDATE'
  | 'REPAIR'
  | 'HUMANIZE'
  | 'COMPLETE'
  | 'NEEDS_REVIEW'
  | 'CANCEL'
  | 'ERROR'
  | 'RESET';

/**
 * Transitions available from every non-terminal state. Stage events loop freely
 * among the busy states (a reconnect can arrive mid-run at any stage, and a
 * `get` replay walks them in order), and all four terminal events are reachable
 * from all of them — a run can fail, be cancelled, or come back `needsReview`
 * at any point.
 */
const BUSY_TRANSITIONS = {
  QUEUED: 'queued',
  PREPARE: 'preparing',
  DRAFT: 'drafting',
  VALIDATE: 'validating',
  REPAIR: 'repairing',
  HUMANIZE: 'humanizing',
  COMPLETE: 'done',
  NEEDS_REVIEW: 'needsReview',
  CANCEL: 'cancelled',
  ERROR: 'error',
} as const;

/** Every terminal state restarts on `START` (a fresh run) as well as `RESET`. */
const TERMINAL_TRANSITIONS = { START: 'queued', RESET: 'idle' } as const;

export const resumePipelineMachine = createMachine<ResumePipelineState, ResumePipelineEvent>({
  transitions: {
    idle: { START: 'queued', RESET: 'idle' },
    queued: { ...BUSY_TRANSITIONS },
    preparing: { ...BUSY_TRANSITIONS },
    drafting: { ...BUSY_TRANSITIONS },
    validating: { ...BUSY_TRANSITIONS },
    repairing: { ...BUSY_TRANSITIONS },
    humanizing: { ...BUSY_TRANSITIONS },
    done: { ...TERMINAL_TRANSITIONS },
    // A run may leave `needsReview` for `done` once the user resolves the last
    // flagged bullet — the write commands return a fresh `PipelineRunDetail`
    // whose status drives that transition through `statusToEvent`.
    needsReview: { ...TERMINAL_TRANSITIONS, COMPLETE: 'done' },
    cancelled: { ...TERMINAL_TRANSITIONS },
    error: { ...TERMINAL_TRANSITIONS },
  },
  busyStates: ['queued', 'preparing', 'drafting', 'validating', 'repairing', 'humanizing'],
  errorStates: ['error'],
});

/** The states in which the run is still doing work — nothing final to show. */
export const RESUME_PIPELINE_BUSY_STATES: readonly ResumePipelineState[] =
  resumePipelineMachine.busyStates ?? [];

/**
 * Stage names (Rust `Stage::name`) → the coarse state each one puts the run in.
 * A name this build doesn't know (a stage added after it shipped) maps to
 * nothing and leaves the machine where it is, which is honest: the progress
 * dots still advance off `index`/`total`.
 *
 * Both depths' lists live in one map because the names are unique across them:
 * quality's `draft` and max's `sections`/`assemble` are three different stages
 * that all mean "the document is being written", and `llm_judge` is max-only.
 * Keying on the name (rather than on a per-depth list) is also what keeps a
 * depth this build cannot enumerate from mapping to the WRONG state — it maps
 * to none.
 */
const STAGE_EVENTS: Record<string, ResumePipelineEvent> = {
  analyze_job: 'PREPARE',
  match_evidence: 'PREPARE',
  strategy: 'PREPARE',
  draft: 'DRAFT',
  // PR-2, quality only. A second streamed pass writing a second document —
  // the same "the document is being written" meaning `draft`/`sections`/
  // `assemble` already share, so it folds into the SAME drafting-family event
  // rather than getting its own busy state.
  cover_letter: 'DRAFT',
  // Max depth writes the document in two stages instead of one: `sections`
  // generates each section on its own and `assemble` renders them into the body
  // (pure, no provider call). Both are "writing" to a reader — the per-section
  // timeline is where the finer state lives.
  sections: 'DRAFT',
  assemble: 'DRAFT',
  validate: 'VALIDATE',
  repair: 'REPAIR',
  // PR-2, quality only — the LAST stage, and the only one that gets its own
  // busy state (`humanizing`) rather than folding into an existing one: it is
  // a genuinely distinct "removing AI signs" phase, not a variant of writing
  // or checking.
  humanize: 'HUMANIZE',
  // The judge reads the FINISHED document (it runs after repair, last), so it
  // is a checking pass, not a repairing one. It cannot fail a run — its stage
  // records `{skipped}` on any error — so `VALIDATE` never strands a run: the
  // record's status is still what ends it.
  llm_judge: 'VALIDATE',
};

/**
 * Map one `pipeline:stage` event to a machine event.
 *
 * `error` is the only phase that can end a run here, and it does so because the
 * stage itself failed — not because the pipeline reached its end. **A `finish`
 * phase NEVER produces a terminal event**, not even for the last stage: see the
 * module doc. `null` means "stay where you are".
 */
export function stageToEvent(
  stage: string,
  phase: 'start' | 'finish' | 'error'
): ResumePipelineEvent | null {
  if (phase === 'error') return 'ERROR';
  if (phase === 'finish') return null;
  return STAGE_EVENTS[stage] ?? null;
}

/**
 * Map a run record's status to the machine event that ends (or doesn't end) the
 * run. This — not any stage event, not `jobs:event`, not `awaitAiStream` — is
 * the completion signal.
 *
 * `needsReview` is deliberately its own terminal state rather than a flavour of
 * `done`: the document is usable but carries findings awaiting a per-bullet
 * verdict, and a surface that renders it as clean is the failure the terminal
 * review exists to prevent.
 */
export function statusToEvent(status: PipelineRunStatus): ResumePipelineEvent | null {
  switch (status) {
    case 'running':
      return null;
    case 'completed':
      return 'COMPLETE';
    case 'needsReview':
      return 'NEEDS_REVIEW';
    case 'cancelled':
      return 'CANCEL';
    case 'failed':
      return 'ERROR';
  }
}
