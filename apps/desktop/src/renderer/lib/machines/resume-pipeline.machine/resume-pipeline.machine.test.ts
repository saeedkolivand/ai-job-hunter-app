import { describe, expect, it } from 'vitest';

import { transition } from '@/lib/machine';

import {
  resumePipelineMachine,
  type ResumePipelineState,
  stageToEvent,
  statusToEvent,
} from './resume-pipeline.machine';

/** The eight stages, in the order `quality_pipeline()` runs them. */
const STAGES = [
  'analyze_job',
  'match_evidence',
  'strategy',
  'draft',
  'cover_letter',
  'validate',
  'repair',
  'humanize',
] as const;

describe('resumePipelineMachine', () => {
  it('folds the three extraction stages into one coarse `preparing` state', () => {
    expect(stageToEvent('analyze_job', 'start')).toBe('PREPARE');
    expect(stageToEvent('match_evidence', 'start')).toBe('PREPARE');
    expect(stageToEvent('strategy', 'start')).toBe('PREPARE');
    expect(stageToEvent('draft', 'start')).toBe('DRAFT');
    expect(stageToEvent('validate', 'start')).toBe('VALIDATE');
    expect(stageToEvent('repair', 'start')).toBe('REPAIR');
  });

  // PR-2. `cover_letter` folds into the SAME drafting-family event `draft`
  // already produces — a second document being streamed, not a new phase.
  // `humanize` gets its own event/state: it is quality depth's genuinely
  // distinct LAST phase.
  it('folds cover_letter into DRAFT and gives humanize its own HUMANIZE event', () => {
    expect(stageToEvent('cover_letter', 'start')).toBe('DRAFT');
    expect(stageToEvent('humanize', 'start')).toBe('HUMANIZE');
  });

  it('walks idle → preparing → drafting → validating → repairing → humanizing on stage starts', () => {
    let state: ResumePipelineState = 'idle';
    state = transition(resumePipelineMachine, state, 'START');
    expect(state).toBe('queued');
    for (const stage of STAGES) {
      const event = stageToEvent(stage, 'start');
      if (event) state = transition(resumePipelineMachine, state, event);
    }
    expect(state).toBe('humanizing');
  });

  it('leaves an unknown stage name where it is instead of guessing', () => {
    // A stage this build predates (or one a later build renames a stage to).
    // It must not guess: the dots still advance off index/total, and the run
    // record still ends it.
    expect(stageToEvent('a_stage_from_a_later_build', 'start')).toBeNull();
  });

  // ── The load-bearing guard ────────────────────────────────────────────────
  //
  // The draft stage streams under the run's umbrella jobId, so the shared
  // stream machinery fires `job_complete` (and resolves `awaitAiStream`) as
  // soon as the draft's last delta lands — with `validate` and up to two repair
  // rounds still ahead. If a stage `finish` were terminal here, the panel would
  // show an unvalidated, unrepaired draft as the finished résumé.
  describe('no stage event may end a run', () => {
    it.each(STAGES)('a `finish` on stage "%s" produces no machine event', (stage) => {
      expect(stageToEvent(stage, 'finish')).toBeNull();
    });

    it('the LAST stage finishing leaves the machine busy, not done', () => {
      let state: ResumePipelineState = transition(resumePipelineMachine, 'idle', 'START');
      for (const stage of STAGES) {
        for (const phase of ['start', 'finish'] as const) {
          const event = stageToEvent(stage, phase);
          if (event) state = transition(resumePipelineMachine, state, event);
        }
      }
      expect(state).toBe('humanizing');
      expect(resumePipelineMachine.busyStates).toContain(state);
      expect(state).not.toBe('done');
      expect(state).not.toBe('needsReview');
    });

    it('only the run RECORD can finish a run', () => {
      const busy = transition(resumePipelineMachine, 'idle', 'START');
      const afterRecord = transition(
        resumePipelineMachine,
        busy,
        statusToEvent('needsReview') ?? 'RESET'
      );
      expect(afterRecord).toBe('needsReview');
    });
  });

  it('reports a stage `error` as a failed run — the one phase that can end it', () => {
    expect(stageToEvent('draft', 'error')).toBe('ERROR');
    expect(stageToEvent('anything-at-all', 'error')).toBe('ERROR');
  });

  describe('statusToEvent', () => {
    it('keeps a running record non-terminal', () => {
      expect(statusToEvent('running')).toBeNull();
    });

    it('maps each terminal status to its own state — needsReview is NOT done', () => {
      expect(statusToEvent('completed')).toBe('COMPLETE');
      expect(statusToEvent('needsReview')).toBe('NEEDS_REVIEW');
      expect(statusToEvent('cancelled')).toBe('CANCEL');
      expect(statusToEvent('failed')).toBe('ERROR');
    });
  });

  it('lets a resolved review leave `needsReview` for `done`', () => {
    expect(transition(resumePipelineMachine, 'needsReview', 'COMPLETE')).toBe('done');
  });

  it('restarts from every terminal state so a retry is not stuck', () => {
    for (const terminal of ['done', 'needsReview', 'cancelled', 'error'] as const) {
      expect(transition(resumePipelineMachine, terminal, 'START')).toBe('queued');
      expect(transition(resumePipelineMachine, terminal, 'RESET')).toBe('idle');
    }
  });

  it('accepts a cancel from any busy state (a boundary stop can land anywhere)', () => {
    for (const state of resumePipelineMachine.busyStates ?? []) {
      expect(transition(resumePipelineMachine, state, 'CANCEL')).toBe('cancelled');
    }
  });
});
