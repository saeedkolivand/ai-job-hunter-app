import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import type { AiStreamChunk } from '@ajh/shared';

import { useResumePipelineSession } from '../use-resume-pipeline-session';
import { bus, cancelJobMock, refreshRunsMock } from './test-mocks';

vi.mock('@/services/use-resume-pipeline', async () =>
  (await import('./test-mocks')).pipelineMock()
);
vi.mock('@/services/use-jobs', async () => (await import('./test-mocks')).jobsMock());

beforeEach(resetPipelineMocks);
import {
  detail,
  JOB_ID,
  jobFailed,
  resetPipelineMocks,
  RUN_ID,
  stage,
  STAGES,
  startRun,
} from './test-support';

describe('useResumePipelineSession', () => {
  it('starts a run and records both ids', async () => {
    const { result } = renderHook(() => useResumePipelineSession());
    await startRun(result);
    expect(result.current.runId).toBe(RUN_ID);
    expect(result.current.jobId).toBe(JOB_ID);
    expect(result.current.state).toBe('queued');
  });

  it('tracks the live stage counter from pipeline:stage', async () => {
    const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
    act(() => bus.stage?.(stage('draft', 'start', 3)));
    expect(result.current.state).toBe('drafting');
    expect(result.current.stage).toMatchObject({ stage: 'draft', index: 3, total: 6 });
  });

  it('ignores stage events belonging to another in-flight run', () => {
    const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
    act(() => bus.stage?.(stage('draft', 'start', 3, 'someone-elses-run')));
    expect(result.current.stage).toBeNull();
    // Still the reconnect's starting state — the other run moved nothing here.
    expect(result.current.state).toBe('queued');
  });

  it('appends draft deltas for display', () => {
    const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
    act(() => {
      bus.delta?.('Ada ');
      bus.delta?.('Lovelace');
    });
    expect(result.current.draft).toBe('Ada Lovelace');
  });

  it('appends reasoning chunks to `thinking`, separate from the document text', () => {
    const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
    act(() => {
      bus.thinking?.('considering the ');
      bus.thinking?.('evidence');
      bus.delta?.('Ada');
    });
    expect(result.current.thinking).toBe('considering the evidence');
    expect(result.current.draft).toBe('Ada');
  });

  // The backend may write the letter BESIDE the draft, so the two stream under
  // separate ids: the draft's tokens must never land in the letter's buffer or
  // the reverse, and the letter stays hidden until its own stage starts.
  describe('the letter stream split', () => {
    it('routes the draft stream to `draft` only', () => {
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => {
        bus.stage?.(stage('draft', 'start', 3));
        bus.delta?.('resume text');
      });
      expect(result.current.draft).toBe('resume text');
      expect(result.current.letterDraft).toBe('');
    });

    it('keeps a letter that streams beside the draft out of both buffers until cover_letter starts', () => {
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => {
        bus.stage?.(stage('draft', 'start', 3));
        bus.delta?.('resume text');
        bus.letterDelta?.('Dear hiring team,');
      });
      expect(result.current.draft).toBe('resume text');
      // Held back: a panel showing `letterDraft || draft` still shows the résumé.
      expect(result.current.letterDraft).toBe('');

      act(() => {
        bus.stage?.(stage('draft', 'finish', 3));
        bus.stage?.(stage('cover_letter', 'start', 4));
        bus.letterDelta?.(' Ada');
      });
      expect(result.current.draft).toBe('resume text');
      expect(result.current.letterDraft).toBe('Dear hiring team, Ada');
    });

    it('reveals the letter on a remount that replays a trail already past cover_letter', async () => {
      bus.detail = {
        ...detail('running'),
        events: [{ stage: 'cover_letter', phase: 'start', ts: 1, artifact: {} }],
      } as never;
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      await waitFor(() => expect(result.current.stage?.stage).toBe('cover_letter'));
      act(() => bus.letterDelta?.('still streaming'));
      expect(result.current.letterDraft).toBe('still streaming');
    });

    it('resets both buffers and the reveal on a new run', async () => {
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => {
        bus.stage?.(stage('cover_letter', 'start', 4));
        bus.letterDelta?.('old letter');
      });
      expect(result.current.letterDraft).toBe('old letter');

      await startRun(result, { includeCoverLetter: true });
      expect(result.current.letterDraft).toBe('');
      expect(result.current.draft).toBe('');

      // The reveal reset too — a fresh letter delta stays hidden until the
      // new run's own cover_letter stage starts.
      act(() => bus.letterDelta?.('new letter'));
      expect(result.current.letterDraft).toBe('');
    });
  });

  // ── The trap this hook exists to avoid ────────────────────────────────────
  //
  // `chat_stream`'s finish() marks the umbrella job completed the moment the
  // draft's last delta lands — several stages before the run ends. Any code
  // that reads "the stream finished" (or "the last stage finished") as "the run
  // finished" shows an unvalidated, unrepaired draft as final. Delete the
  // status-driven terminal detection in the hook and this test fails; make
  // `stageToEvent` terminal on a `finish` and it fails too.
  describe('the draft stream is not the completion signal', () => {
    it('stays busy after every stage finishes AND the stream reports done', () => {
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => {
        STAGES.forEach((name, index) => {
          bus.stage?.(stage(name, 'start', index));
          bus.stage?.(stage(name, 'finish', index));
        });
        // The `done` frame of the display-only draft stream.
        const done: AiStreamChunk = { jobId: JOB_ID, delta: '', done: true };
        bus.delta?.(done.delta);
      });

      expect(result.current.busy).toBe(true);
      expect(result.current.state).not.toBe('done');
      expect(result.current.state).not.toBe('needsReview');
      // Still polling — which is the only reason a boundary stop is ever noticed.
      expect(bus.live).toBe(true);
    });

    it('finishes only once the run RECORD reports a terminal status', async () => {
      const { result, rerender } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => bus.stage?.(stage('repair', 'finish', 5)));
      expect(result.current.busy).toBe(true);

      bus.detail = detail('needsReview');
      rerender();

      await waitFor(() => expect(result.current.state).toBe('needsReview'));
      expect(result.current.busy).toBe(false);
      // `needsReview` is NOT success — the document exists but carries findings.
      expect(result.current.state).not.toBe('done');
      expect(result.current.detail?.resumeText).toBe('final document');
    });

    /**
     * The same discovery is the only thing that can tell the posting's run LIST
     * its run just ended — nothing was clicked, so none of the three
     * action-driven invalidators fires, and `runsForJob` has no poll of its
     * own. Left alone the list renders this run as "Running" indefinitely.
     *
     * The posting comes off the RECORD (`detail.jobUrl`), not from the caller:
     * this hook is not given a posting url at all, and the row is the authority
     * on which one the run belongs to.
     */
    it('tells the posting run list to refresh, keyed on the record’s own jobUrl', async () => {
      const { result, rerender } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      expect(refreshRunsMock).toHaveBeenLastCalledWith(undefined, undefined);

      bus.detail = detail('needsReview');
      rerender();
      await waitFor(() => expect(result.current.state).toBe('needsReview'));
      expect(refreshRunsMock).toHaveBeenLastCalledWith('https://example.test/job', 'needsReview');
    });

    it('notices a boundary stop that emitted no terminal stage event at all', async () => {
      // A cancel / deadline stop returns Err from `RunHooks::before`, so the
      // stage it refused to start never emits — the record is the only witness.
      const { result, rerender } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => bus.stage?.(stage('validate', 'finish', 4)));
      bus.detail = detail('cancelled');
      rerender();
      await waitFor(() => expect(result.current.state).toBe('cancelled'));
    });
  });

  // ── reset()'s own stale-ref window ──────────────────────────────────────
  //
  // Same shape as the `start()` race above (see "arrives after a clean
  // reconcile read, before the render commits"): the `pipeline:stage` and
  // `job.failed` listeners are still mounted on the run/job that was just
  // reset — nothing unsubscribes them — so a late event can fire in the gap
  // before `reset()`'s `setState` calls commit. `reset()` closes that gap by
  // assigning `runIdRef`/`jobIdRef`/`busyRef` SYNCHRONOUSLY, inside the same
  // call, instead of leaving them to the next render body — a late event
  // arriving in that gap must be dropped by the (fresh) ref guard, not read
  // against the old run/job's id.
  describe('reset()', () => {
    it('drops a stage AND a job.failed event for the just-reset run/job that arrive before the render commits', async () => {
      const { result, rerender } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      bus.detail = detail('needsReview');
      rerender();
      await waitFor(() => expect(result.current.state).toBe('needsReview'));

      // A missing registration must fail loudly here, not be silently
      // skipped by an optional call below — that silence is exactly what
      // would let this test pass green without ever exercising the guard
      // it exists to catch.
      if (!bus.stage) throw new Error('pipeline:stage listener was not registered');
      if (!bus.job) throw new Error('job event listener was not registered');
      const stageListener = bus.stage;
      const jobListener = bus.job;

      act(() => {
        result.current.reset();
        // Both fire in the SAME synchronous scope as reset() — before this
        // act() lets the pending RESET/setState calls commit — exactly the
        // ordering a real listener callback can race into. Together they
        // exercise all three refs reset() writes synchronously: runIdRef
        // (the stage event) and jobIdRef + busyRef (the job.failed event).
        stageListener(stage('analyze_job', 'start', 0));
        jobListener(jobFailed(JOB_ID, 'late failure for the just-reset run'));
      });

      expect(result.current.state).toBe('idle');
      expect(result.current.stage).toBeNull();
      expect(result.current.error).toBeNull();
    });
  });

  it('cancels through the umbrella job id and waits for the record to confirm', () => {
    const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
    act(() => bus.stage?.(stage('draft', 'start', 3)));
    act(() => result.current.cancel());
    expect(cancelJobMock.mutate).toHaveBeenCalledWith(JOB_ID);
    // The backend decides — a run that finished a millisecond earlier finished.
    expect(result.current.state).toBe('drafting');
  });

  it('reconnects to a run that was already finished when the panel remounted', async () => {
    bus.detail = detail('completed');
    const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
    await waitFor(() => expect(result.current.state).toBe('done'));
    expect(result.current.runId).toBe(RUN_ID);
  });
});
