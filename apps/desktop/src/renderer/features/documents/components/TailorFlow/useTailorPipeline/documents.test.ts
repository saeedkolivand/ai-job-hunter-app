/**
 * useTailorPipeline — document text sources, inline-edit persistence, meta seeding, and which document opens.
 * Mocks + render helpers live in `harness.ts` (see its header).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import type { AiGenerationRecord } from '@ajh/shared';

import {
  detail,
  qualityRecheckArgs,
  record,
  render,
  resetHarness,
  sessionBus,
  updateAiGenerationMutate,
} from './harness';

beforeEach(resetHarness);

describe('useTailorPipeline — document text sources', () => {
  it('reads the résumé from the run detail and the letter from the aggregate record', () => {
    sessionBus.detail = detail({ resumeText: 'RESUME FROM RUN' });
    const generation = record({
      id: 'gen-1',
      coverLetterText: 'LETTER FROM AGGREGATE',
    });
    const { result } = render({ latestGeneration: generation });

    expect(result.current.resumeOut).toBe('RESUME FROM RUN');
    act(() => result.current.setActiveOut('cover'));
    expect(result.current.coverOut).toBe('LETTER FROM AGGREGATE');
    expect(result.current.output).toBe('LETTER FROM AGGREGATE');
  });

  it('hasOutput is false while idle with no detail and no aggregate letter', () => {
    const { result } = render();
    expect(result.current.hasOutput).toBe(false);
  });

  it('cold entry: falls back to the aggregate résumé text when no live run detail exists', () => {
    // No session.detail — a fresh session that never started/reconnected a
    // run, but the posting already has a saved result from elsewhere.
    const generation = record({
      id: 'gen-1',
      resumeText: 'RESUME FROM A PAST RUN',
      coverLetterText: '',
    });
    const { result } = render({ latestGeneration: generation });

    expect(result.current.resumeOut).toBe('RESUME FROM A PAST RUN');
    expect(result.current.hasOutput).toBe(true);
    expect(result.current.meta).not.toBeNull();
  });

  it('prefers the LIVE run detail over the aggregate once one exists', () => {
    sessionBus.detail = detail({ resumeText: 'LIVE RESUME' });
    const generation = record({
      id: 'gen-1',
      resumeText: 'STALE RESUME',
      coverLetterText: '',
    });
    const { result } = render({ latestGeneration: generation });

    expect(result.current.resumeOut).toBe('LIVE RESUME');
  });
});

describe('useTailorPipeline — inline edit persistence', () => {
  // CR-8: teardown belongs in `afterEach`, not the last line of a test body —
  // a failed assertion above it would skip `vi.useRealTimers()` and leak fake
  // timers into every later test in the file (an order-dependent green).
  afterEach(() => {
    vi.useRealTimers();
  });

  it('debounce-persists to the aggregate id once one exists', () => {
    vi.useFakeTimers();
    sessionBus.detail = detail();
    const generation = record({ id: 'gen-1', coverLetterText: '' });
    const { result } = render({ latestGeneration: generation });

    act(() => result.current.editActiveOutput('hand-edited résumé'));
    vi.runAllTimers();

    expect(updateAiGenerationMutate).toHaveBeenCalledWith({
      id: 'gen-1',
      resumeText: 'hand-edited résumé',
    });
  });

  it('never calls updateAiGeneration without an aggregate id (session-only edit)', () => {
    vi.useFakeTimers();
    const { result } = render();
    act(() => result.current.editActiveOutput('edited text'));
    vi.runAllTimers();
    expect(updateAiGenerationMutate).not.toHaveBeenCalled();
  });

  // CR-2: unmounting inside the debounce window previously CLEARED the timer
  // without flushing — the pending write (and the local override that would
  // have re-surfaced it) both vanished with the component. Silent user data
  // loss: type a hand-edit, leave the tab (or the host remounts
  // `DocumentsTab`) before the debounce fires, and the edit never persists.
  it('flushes a pending edit on unmount instead of dropping it', () => {
    vi.useFakeTimers();
    sessionBus.detail = detail();
    const generation = record({ id: 'gen-1', coverLetterText: '' });
    const { result, unmount } = render({ latestGeneration: generation });

    act(() => result.current.editActiveOutput('hand-edited résumé'));
    // Still inside the debounce window — nothing persisted YET, proving the
    // assertion below is about the unmount flush, not a race with the timer.
    expect(updateAiGenerationMutate).not.toHaveBeenCalled();

    unmount();

    expect(updateAiGenerationMutate).toHaveBeenCalledWith({
      id: 'gen-1',
      resumeText: 'hand-edited résumé',
    });
  });

  it('does not double-persist if the debounce timer somehow still fires after the unmount flush', () => {
    vi.useFakeTimers();
    sessionBus.detail = detail();
    const generation = record({ id: 'gen-1', coverLetterText: '' });
    const { result, unmount } = render({ latestGeneration: generation });

    act(() => result.current.editActiveOutput('hand-edited résumé'));
    unmount();
    updateAiGenerationMutate.mockClear();

    vi.runAllTimers();

    expect(updateAiGenerationMutate).not.toHaveBeenCalled();
  });
});

// Stage 6d — the fabricated `meta` stub silently dropped the "top requirement
// hits" metric from a Re-check (`use-quality-recheck.ts` sends
// `meta.topRequirements` verbatim) and short-circuited the answers
// assistant's own metadata extraction (`useApplicationAnswers.ts` treats any
// non-null `meta` as already detected). Reverting to the fabricated
// `topRequirements: []` makes this fail.
describe('useTailorPipeline — meta is seeded from the aggregate, not fabricated', () => {
  it("meta.topRequirements equals the record's list, not an empty array", () => {
    sessionBus.detail = detail({ resumeText: 'RESUME' });
    const generation = record({
      id: 'gen-1',
      candidateName: 'Jane Doe',
      resumeLanguage: 'de',
      jobAdLanguage: 'en',
      mismatch: true,
      topRequirements: ['Kubernetes', 'Rust'],
      coverLetterText: '',
    });

    const { result } = render({ latestGeneration: generation });

    expect(result.current.meta?.topRequirements).toEqual(['Kubernetes', 'Rust']);
    expect(result.current.meta?.candidateName).toBe('Jane Doe');
    expect(result.current.meta?.resumeLanguage).toBe('de');
    expect(result.current.meta?.jobAdLanguage).toBe('en');
    expect(result.current.meta?.mismatch).toBe(true);
  });

  it('falls back to the derived defaults when no aggregate exists', () => {
    sessionBus.detail = detail({ resumeText: 'RESUME' });
    const { result } = render();

    expect(result.current.meta?.topRequirements).toEqual([]);
    expect(result.current.meta?.candidateName).toBe('');
    expect(result.current.meta?.mismatch).toBe(false);
  });
});

describe('useTailorPipeline — which document the panel opens on', () => {
  // A finished run whose detail still carries a résumé — the posting's
  // aggregate keeps whatever an earlier run saved, so "there is no résumé to
  // show" is NOT what makes the cover-only case below pass.
  const RESUME = 'FINAL RESUME';
  const LETTER = 'THE LETTER';
  const withLetter = {
    id: 'gen-1',
    coverLetterText: LETTER,
  } as unknown as AiGenerationRecord;

  beforeEach(() => {
    sessionBus.detail = detail({ resumeText: RESUME });
  });

  it('opens a cover-only run on the letter, never on the résumé', () => {
    const { result } = render({ target: 'cover', latestGeneration: withLetter });

    expect(result.current.activeOut).toBe('cover');
    expect(result.current.output).toBe(LETTER);
    // The reported bug, verbatim.
    expect(result.current.output).not.toBe(RESUME);
  });

  it.each(['resume', 'both'] as const)(
    'opens a %s run on the résumé, exactly as before',
    (target) => {
      const { result } = render({ target, latestGeneration: withLetter });
      expect(result.current.activeOut).toBe('resume');
      expect(result.current.output).toBe(RESUME);
    }
  );

  it('lets a tab click override the target-derived default', () => {
    const { result } = render({ target: 'cover', latestGeneration: withLetter });
    act(() => result.current.setActiveOut('resume'));
    // Derivation, not a lock: the résumé tab a cover-only run shows for a
    // previously saved document has to be selectable.
    expect(result.current.activeOut).toBe('resume');
    expect(result.current.output).toBe(RESUME);
  });

  it('offers no review controls on a document this run did not write', () => {
    const { result } = render({ target: 'cover', latestGeneration: withLetter });
    // The letter — this run's own document — keeps its review controls.
    expect(result.current.pipelineReview).toBeDefined();
    expect(result.current.recheck).toBeDefined();

    act(() => result.current.setActiveOut('resume'));
    // Re-check re-validates the ACTIVE document and persists the merged wrapper
    // back onto the aggregate (`useQualityRecheck`'s `persistReport`), so
    // leaving it live would let a run with no résumé of its own overwrite the
    // posting's résumé report. The same hole as Fix section, one affordance
    // over — found by review, not by me.
    expect(result.current.recheck).toBeUndefined();
    // …and the reason is that the writer was WITHHELD, not that some other
    // precondition happened to be missing.
    expect(qualityRecheckArgs.current?.onReportChange).toBeUndefined();
    // The résumé does not. `PipelineRunDetail.report` is read off the per-job
    // AGGREGATE, not the run, so an older run's `resume` slot is still present
    // here — and `regenerateSection` would ACCEPT a Fix click against it (this
    // IS the posting's newest run), spending a provider call rewriting a
    // document the run on screen never produced.
    expect(result.current.pipelineReview).toBeUndefined();
  });
});
