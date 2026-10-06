import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import { useResumePipelineSession } from '../use-resume-pipeline-session';
import { bus, fetchJobMock, startMock } from './test-mocks';

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
  START_ARGS,
  startRun,
} from './test-support';

describe('useResumePipelineSession — failures', () => {
  // ── The failure reason survives a remount the live listener missed ────────
  //
  // `job.failed` only ever reaches `setError` while a listener is mounted at
  // the moment it fires. A fresh mount that reconnects to an already-failed
  // run never saw that event, so without a fallback `error` stays `null`
  // forever even though the record says exactly why the run stopped.
  describe('the failure reason survives a remount that missed job.failed', () => {
    it('derives the reason from the persisted stoppedReason when no live event ever arrived', async () => {
      // Simulates a fresh mount reconnecting to a run that already failed
      // while unmounted — no `bus.job?.(...)` call anywhere in this test.
      bus.detail = detail('failed', 'run_timeout');
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));

      await waitFor(() => expect(result.current.state).toBe('error'));
      expect(result.current.error).toBeTruthy();
      expect(result.current.error).toContain('ran out of time');
    });

    it('prefers the live job.failed message over the persisted reason when both exist', async () => {
      const { result, rerender } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => bus.job?.(jobFailed(JOB_ID, 'the provider refused the request')));
      await waitFor(() => expect(result.current.state).toBe('error'));

      // The record catches up on a later poll with a DIFFERENT reason — the
      // live message, which arrived first-hand, must still win.
      bus.detail = detail('failed', 'timeout');
      rerender();
      expect(result.current.error).toBe('the provider refused the request');
    });

    it('says nothing for a failed run that recorded no reason at all', async () => {
      bus.detail = detail('failed', null);
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      await waitFor(() => expect(result.current.state).toBe('error'));
      expect(result.current.error).toBeNull();
    });
  });

  // ── A read that never succeeds must not read as "still working" ───────────
  //
  // The record's status is the ONLY completion signal, so a session that never
  // gets a first record can never leave a busy state. Discard `isError` here
  // and the machine spins forever on a request that already gave up — the exact
  // silent death these two tests pin.
  describe('a failing record read', () => {
    it('errors the session when NO record has ever landed', async () => {
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      bus.recordError = new Error('ipc channel closed');
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));

      await waitFor(() => expect(result.current.state).toBe('error'));
      expect(result.current.busy).toBe(false);
      expect(result.current.error).toContain('ipc channel closed');
      consoleError.mockRestore();
    });

    it('does NOT kill a live run over one dropped read', async () => {
      // A blip after a record has landed is a blip: the run is still going and
      // the query keeps polling. Ending it here would be the opposite mistake.
      bus.detail = detail('running');
      const { result, rerender } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => bus.stage?.(stage('draft', 'start', 3)));
      expect(result.current.state).toBe('drafting');

      bus.recordError = new Error('blip');
      rerender();

      await waitFor(() => expect(result.current.state).toBe('drafting'));
      expect(result.current.busy).toBe(true);
      expect(bus.live).toBe(true);
    });
  });

  // ── The failure the record can't report ───────────────────────────────────
  //
  // `resume_pipeline_run` returns its ids immediately and writes the
  // `pipeline_runs` row inside the spawned task, AFTER admission and after it
  // resolves the depth, the provider, the résumé and the cached posting. Each of
  // those failures calls `job_fail` with no row ever written, so `get(runId)`
  // answers `null` forever — a real answer, so the poll stops — and the status,
  // this hook's only completion signal, never exists. A failure of the FINAL
  // `upsert_run` leaves the same hole from the other end: a row stuck at
  // `running`. Drop the `job.failed` consumer and the session spins either way.
  describe('a run that failed without a terminal record', () => {
    it('ends the session on the umbrella job failure when no record exists', async () => {
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => bus.job?.(jobFailed(JOB_ID, 'job not found in cache: posting-1')));

      await waitFor(() => expect(result.current.state).toBe('error'));
      expect(result.current.busy).toBe(false);
      expect(result.current.error).toContain('job not found in cache');
      consoleError.mockRestore();
    });

    // The earliest instance of the same hole: `job.failed` can fire while
    // `mutateAsync` is still in flight, before `jobId` state exists at all —
    // so the live listener above drops it (`jobIdRef.current` is null). Only
    // `start`'s own `fetchJob` reconcile, run right after the ids land, can
    // ever notice. This reproduces the ACTUAL ordering (the event fires
    // inside the mutation, before it resolves) rather than asserting the
    // eventual state some other way.
    it('reconciles a job.failed event that raced ahead of start() resolving', async () => {
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      startMock.mutateAsync.mockImplementationOnce(async () => {
        bus.job?.(jobFailed(JOB_ID, 'agent_run queue is full'));
        return { runId: RUN_ID, jobId: JOB_ID };
      });
      fetchJobMock.mockResolvedValueOnce({ status: 'failed', error: 'agent_run queue is full' });

      const { result } = renderHook(() => useResumePipelineSession());
      await startRun(result);

      expect(fetchJobMock).toHaveBeenCalledWith(JOB_ID);
      expect(result.current.state).toBe('error');
      expect(result.current.busy).toBe(false);
      expect(result.current.error).toContain('agent_run queue is full');
      consoleError.mockRestore();
    });

    // A DIFFERENT ordering than the test above, and the residual window
    // CodeRabbit found in that fix: the reconcile read comes back CLEAN, and
    // only THEN does `job.failed` arrive. `jobIdRef`/`runIdRef` are written
    // in the render body, so they only pick up the new ids once React
    // actually re-renders — which a `setState` call does not do
    // synchronously. Firing the event in the same `act()` scope right after
    // `start()` resolves, before that scope gets to flush the pending
    // render, reproduces exactly the gap: without `start()` also assigning
    // both refs synchronously, this event would still see them null/stale
    // and be dropped, even though `start()` itself already found nothing
    // wrong.
    it('reconciles a job.failed event that arrives after a clean reconcile read, before the render commits', async () => {
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      fetchJobMock.mockResolvedValueOnce({ status: 'queued' });

      const { result } = renderHook(() => useResumePipelineSession());
      await act(async () => {
        await result.current.start(START_ARGS);
        bus.job?.(jobFailed(JOB_ID, 'writing the run row failed'));
      });

      expect(fetchJobMock).toHaveBeenCalledWith(JOB_ID);
      expect(result.current.state).toBe('error');
      expect(result.current.busy).toBe(false);
      expect(result.current.error).toContain('writing the run row failed');
      consoleError.mockRestore();
    });

    it('ignores a failure belonging to another job', () => {
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => bus.job?.(jobFailed('someone-elses-job', 'boom')));
      expect(result.current.state).toBe('queued');
    });

    // The other half of the hole: `execute` can fail ON its final `upsert_run`,
    // leaving a row that says `running` for good. Gating on "no record yet"
    // instead of "the machine is still busy" misses exactly this case.
    it('ends the session when the record is stuck at running', async () => {
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      bus.detail = detail('running');
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => bus.stage?.(stage('draft', 'start', 3)));
      expect(result.current.state).toBe('drafting');

      act(() => bus.job?.(jobFailed(JOB_ID, 'writing the run row failed')));
      await waitFor(() => expect(result.current.state).toBe('error'));
      expect(result.current.error).toContain('writing the run row failed');
      consoleError.mockRestore();
    });

    // A per-call timeout carries `{ kind: 'timeout', stage, seconds }` instead
    // of a plain string (see `hooks::timeout_failure_data` on the Rust side) —
    // the ONE `job.failed` shape this hook renders through `pipeline.timeout`
    // rather than a raw string, so the banner names a step the user recognizes
    // ("Matching your evidence") instead of the internal wire key
    // ("match_evidence") a German (or any) user has never seen.
    //
    // Mutation check: read `event.data` as a plain string unconditionally
    // (the pre-fix shape) and this fails — the raw key shows up verbatim.
    it('localizes a per-call timeout instead of splicing the raw stage key into prose', async () => {
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() =>
        bus.job?.(jobFailed(JOB_ID, { kind: 'timeout', stage: 'match_evidence', seconds: 302 }))
      );

      await waitFor(() => expect(result.current.state).toBe('error'));
      expect(result.current.error).toContain('Matching your evidence');
      expect(result.current.error).toContain('302');
      expect(result.current.error).not.toContain('match_evidence');
      consoleError.mockRestore();
    });

    // A stage this build has no `pipeline.stage.*` copy for (added server-side
    // after this renderer shipped) must still say SOMETHING rather than an
    // empty label — the same `defaultValue` fallback `useTailorPipeline`'s
    // `stageLabel` already relies on.
    it('falls back to the raw stage key when no pipeline.stage label exists for it', async () => {
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() =>
        bus.job?.(jobFailed(JOB_ID, { kind: 'timeout', stage: 'a_future_stage', seconds: 12 }))
      );

      await waitFor(() => expect(result.current.state).toBe('error'));
      expect(result.current.error).toContain('a_future_stage');
      consoleError.mockRestore();
    });

    // A run that already reached a terminal state is left alone — error text
    // included. The backend reports a deadline-stopped-but-saved run as complete
    // on the JOB while the row says `needsReview`, so letting a late job event
    // through would contradict a document the run's own row calls reviewable.
    it('leaves a terminal run alone, error text included', async () => {
      bus.detail = detail('needsReview');
      const { result, rerender } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      await waitFor(() => expect(result.current.state).toBe('needsReview'));

      act(() => bus.job?.(jobFailed(JOB_ID, 'late failure')));
      rerender();
      expect(result.current.state).toBe('needsReview');
      expect(result.current.error).toBeNull();
    });

    // `job.completed` fires the moment the draft's last delta lands — with
    // validation and up to two repair rounds still ahead.
    it('never treats the umbrella job COMPLETING as the run finishing', () => {
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => bus.stage?.(stage('draft', 'finish', 3)));
      act(() => bus.job?.({ type: 'job.completed', jobId: JOB_ID, ts: 1 }));
      expect(result.current.busy).toBe(true);
      expect(result.current.state).not.toBe('done');
    });
  });

  it('surfaces a start failure instead of leaving the panel spinning', async () => {
    startMock.mutateAsync.mockRejectedValueOnce(new Error('resume not found: doc-9'));
    const { result } = renderHook(() => useResumePipelineSession());
    await startRun(result, { resumeId: 'doc-9' });
    expect(result.current.state).toBe('error');
    expect(result.current.error).toContain('doc-9');
  });
});
