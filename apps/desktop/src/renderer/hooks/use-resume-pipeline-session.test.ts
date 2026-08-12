import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import type { AiStreamChunk, PipelineStageEvent } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';

import { useResumePipelineSession } from './use-resume-pipeline-session';

// ── Service-hook stubs ────────────────────────────────────────────────────
//
// The hook's whole job is to combine three sources (stage events, the draft
// stream, the run record) into one state, so the test drives all three by hand
// rather than through a mock IPC client.

const startMock = vi.hoisted(() => ({
  mutateAsync: vi.fn(),
  isPending: false,
}));
const cancelJobMock = vi.hoisted(() => ({ mutate: vi.fn() }));
const bus = vi.hoisted(() => ({
  stage: null as ((e: PipelineStageEvent) => void) | null,
  delta: null as ((d: string) => void) | null,
  detail: null as PipelineRunDetail | null,
  live: false,
  recordError: null as Error | null,
}));

vi.mock('@/services/use-resume-pipeline', () => ({
  useStartResumePipelineRun: () => startMock,
  usePipelineRun: (_runId: string | null, live: boolean) => {
    bus.live = live;
    return { data: bus.detail, isError: !!bus.recordError, error: bus.recordError };
  },
  usePipelineStageEvents: (handler?: (e: PipelineStageEvent) => void) => {
    bus.stage = handler ?? null;
  },
  usePipelineDraftStream: (_jobId: string | null, handler?: (d: string) => void) => {
    bus.delta = handler ?? null;
  },
}));

vi.mock('@/services/use-jobs', () => ({ useCancelJob: () => cancelJobMock }));

const RUN_ID = 'run-1';
const JOB_ID = 'job-1';

function stage(
  name: string,
  phase: PipelineStageEvent['phase'],
  index: number,
  runId = RUN_ID
): PipelineStageEvent {
  return { runId, jobId: JOB_ID, stage: name, phase, index, total: 6, attempt: 1 };
}

function detail(status: PipelineRunDetail['status']): PipelineRunDetail {
  return {
    runId: RUN_ID,
    jobUrl: 'https://example.test/job',
    kind: 'resume',
    depth: 'quality',
    status,
    startedAt: 1,
    metrics: {},
    events: [],
    report: null,
    resumeText: 'final document',
  };
}

/** Start a run and drive the stage stream through the whole pipeline. */
const STAGES = ['analyze_job', 'match_evidence', 'strategy', 'draft', 'validate', 'repair'];

beforeEach(() => {
  vi.clearAllMocks();
  bus.stage = null;
  bus.delta = null;
  bus.detail = null;
  bus.live = false;
  bus.recordError = null;
  startMock.mutateAsync.mockResolvedValue({ runId: RUN_ID, jobId: JOB_ID });
});

describe('useResumePipelineSession', () => {
  it('starts a run and records both ids', async () => {
    const { result } = renderHook(() => useResumePipelineSession());
    await act(async () => {
      await result.current.start({
        resumeId: 'doc-1',
        jobId: 'posting-1',
        jobUrl: '',
        depth: 'quality',
        targetLanguage: 'en',
        topRequirements: [],
        coverLetterText: '',
      });
    });
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

  it('surfaces a start failure instead of leaving the panel spinning', async () => {
    startMock.mutateAsync.mockRejectedValueOnce(new Error('resume not found: doc-9'));
    const { result } = renderHook(() => useResumePipelineSession());
    await act(async () => {
      await result.current.start({
        resumeId: 'doc-9',
        jobId: 'posting-1',
        jobUrl: '',
        depth: 'quality',
        targetLanguage: 'en',
        topRequirements: [],
        coverLetterText: '',
      });
    });
    expect(result.current.state).toBe('error');
    expect(result.current.error).toContain('doc-9');
  });
});
