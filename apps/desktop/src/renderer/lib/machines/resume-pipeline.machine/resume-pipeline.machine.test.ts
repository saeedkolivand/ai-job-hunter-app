import { describe, expect, it } from 'vitest';

import { transition } from '@/lib/machine';

import {
  foldSectionStates,
  type PipelineSectionStates,
  resumePipelineMachine,
  type ResumePipelineState,
  stageToEvent,
  statusToEvent,
} from './resume-pipeline.machine';

/** The eight quality-depth stages, in the order `quality_pipeline()` runs them. */
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

/**
 * The eight max-depth stages, in `MAX_STAGES` order — `draft` split into
 * `sections` + `assemble`, and the judge LAST, after repair, because it grades
 * the finished document.
 */
const MAX_STAGES = [
  'analyze_job',
  'match_evidence',
  'strategy',
  'sections',
  'assemble',
  'validate',
  'repair',
  'llm_judge',
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

  it('maps the three max-only stages, none of them to a terminal state', () => {
    // Phase 4. `sections` and `assemble` are max depth's split of `draft`, and
    // the judge — which runs LAST, after repair — is a review pass over the
    // finished document, so it reads as checking rather than repairing.
    expect(stageToEvent('sections', 'start')).toBe('DRAFT');
    expect(stageToEvent('assemble', 'start')).toBe('DRAFT');
    expect(stageToEvent('llm_judge', 'start')).toBe('VALIDATE');
  });

  it('leaves an unknown stage name where it is instead of guessing', () => {
    // A stage added to a FUTURE depth this build predates. It must not guess:
    // the dots still advance off index/total, and the run record still ends it.
    expect(stageToEvent('a_stage_from_a_later_build', 'start')).toBeNull();
  });

  it('walks the whole max stage list and lands in `validating`, still busy', () => {
    let state: ResumePipelineState = transition(resumePipelineMachine, 'idle', 'START');
    for (const stage of MAX_STAGES) {
      for (const phase of ['start', 'finish'] as const) {
        const event = stageToEvent(stage, phase);
        if (event) state = transition(resumePipelineMachine, state, event);
      }
    }
    // The judge is the last stage, so a max run ends its trail in `validating`
    // rather than `repairing` — and, exactly like quality, still busy.
    expect(state).toBe('validating');
    expect(resumePipelineMachine.busyStates).toContain(state);
    expect(state).not.toBe('done');
  });

  // ── The load-bearing guard ────────────────────────────────────────────────
  //
  // The draft stage streams under the run's umbrella jobId, so the shared
  // stream machinery fires `job_complete` (and resolves `awaitAiStream`) as
  // soon as the draft's last delta lands — with `validate` and up to two repair
  // rounds still ahead. If a stage `finish` were terminal here, the panel would
  // show an unvalidated, unrepaired draft as the finished résumé.
  describe('no stage event may end a run', () => {
    it.each([...new Set([...STAGES, ...MAX_STAGES])])(
      'a `finish` on stage "%s" produces no machine event',
      (stage) => {
        expect(stageToEvent(stage, 'finish')).toBeNull();
      }
    );

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

// ── The per-section fold ────────────────────────────────────────────────────
//
// Everything here is a claim about what the WIRE proves. The temptation this
// suite exists to resist is filling the ladder in from plausible inference —
// a `needsChanges` on the section that "probably" caused the issue count, a
// `queued` row for a section the run may never generate.
describe('foldSectionStates', () => {
  const event = (
    stage: string,
    phase: 'start' | 'finish' | 'error',
    extra: { sectionKey?: string; issueCount?: number } = {}
  ) => ({ stage, phase, ...extra }) as Parameters<typeof foldSectionStates>[1];

  it('walks one section from generating to done', () => {
    let states: PipelineSectionStates = {};
    states = foldSectionStates(states, event('sections', 'start', { sectionKey: 'summary' }));
    expect(states).toEqual({ summary: 'generating' });
    states = foldSectionStates(states, event('sections', 'finish', { sectionKey: 'summary' }));
    expect(states).toEqual({ summary: 'done' });
  });

  it('tracks each experience entry separately', () => {
    let states: PipelineSectionStates = {};
    states = foldSectionStates(states, event('sections', 'finish', { sectionKey: 'experience:0' }));
    states = foldSectionStates(states, event('sections', 'start', { sectionKey: 'experience:1' }));
    expect(states).toEqual({ 'experience:0': 'done', 'experience:1': 'generating' });
  });

  // Drop the `event.sectionKey` read (fold on the stage name alone) and this
  // fails: every section would collapse onto one key.
  it('needs the sectionKey — a bare `sections` stage event moves nothing', () => {
    const states = { summary: 'done' } as PipelineSectionStates;
    expect(foldSectionStates(states, event('sections', 'start'))).toBe(states);
    expect(foldSectionStates(states, event('sections', 'finish'))).toBe(states);
  });

  it('moves finished sections to checking when validate starts', () => {
    const states: PipelineSectionStates = { summary: 'done', skills: 'generating' };
    expect(foldSectionStates(states, event('validate', 'start'))).toEqual({
      summary: 'checking',
      // Still generating: `validate` cannot have checked a section that had not
      // finished when it started.
      skills: 'generating',
    });
  });

  it('calls a section clean only when the WHOLE document came back clean', () => {
    const checking: PipelineSectionStates = { summary: 'checking', skills: 'checking' };
    expect(foldSectionStates(checking, event('validate', 'finish', { issueCount: 0 }))).toEqual({
      summary: 'clean',
      skills: 'clean',
    });
  });

  // The honesty guard. `validate`'s counts are run-level: the artifact is
  // `{issues, criticals, codes}` with no section attribution, and `repair`'s is
  // `{rounds, reverted, …}`. Attributing either to a section would be a guess.
  it('leaves sections alone when the run-level counts cannot name one', () => {
    const checking: PipelineSectionStates = { summary: 'checking' };
    expect(foldSectionStates(checking, event('validate', 'finish', { issueCount: 3 }))).toBe(
      checking
    );
    // No count at all proves nothing either — not "clean".
    expect(foldSectionStates(checking, event('validate', 'finish'))).toBe(checking);
    for (const stage of ['repair', 'llm_judge', 'assemble']) {
      expect(foldSectionStates(checking, event(stage, 'start'))).toBe(checking);
      expect(foldSectionStates(checking, event(stage, 'finish', { issueCount: 0 }))).toBe(checking);
    }
  });

  it('never invents a queued section — the wire cannot say what is coming', () => {
    // The live event's index/total belong to the STAGE, and the section counts
    // ride only in the persisted artifact, so the roster is unknowable in
    // advance. Every state the fold produces is one an event proved.
    let states: PipelineSectionStates = {};
    for (const key of ['summary', 'skills']) {
      states = foldSectionStates(states, event('sections', 'start', { sectionKey: key }));
      states = foldSectionStates(states, event('sections', 'finish', { sectionKey: key }));
    }
    expect(Object.keys(states)).toEqual(['summary', 'skills']);
    expect(Object.values(states)).not.toContain('queued');
  });

  it('returns the same object when nothing moved, so a quality run never re-renders', () => {
    const states: PipelineSectionStates = {};
    // The whole quality-depth stage list carries no sectionKey.
    for (const stage of STAGES) {
      for (const phase of ['start', 'finish'] as const) {
        expect(foldSectionStates(states, event(stage, phase))).toBe(states);
      }
    }
  });
});
