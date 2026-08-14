import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import type { AiStreamChunk, JobEvent, PipelineStageEvent } from '@ajh/shared';
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
const refreshRunsMock = vi.hoisted(() => vi.fn());
const bus = vi.hoisted(() => ({
  stage: null as ((e: PipelineStageEvent) => void) | null,
  delta: null as ((d: string) => void) | null,
  thinking: null as ((d: string) => void) | null,
  job: null as ((e: JobEvent) => void) | null,
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
  usePipelineDraftStream: (
    _jobId: string | null,
    onDelta?: (d: string) => void,
    onThinking?: (d: string) => void
  ) => {
    bus.delta = onDelta ?? null;
    bus.thinking = onThinking ?? null;
  },
  useRefreshRunsForJobOnTerminal: refreshRunsMock,
}));

vi.mock('@/services/use-jobs', () => ({
  useCancelJob: () => cancelJobMock,
  useJobEvents: (handler?: (e: JobEvent) => void) => {
    bus.job = handler ?? null;
  },
}));

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
const STAGES = [
  'analyze_job',
  'match_evidence',
  'strategy',
  'draft',
  'cover_letter',
  'validate',
  'repair',
  'humanize',
];

beforeEach(() => {
  vi.clearAllMocks();
  bus.stage = null;
  bus.delta = null;
  bus.thinking = null;
  bus.job = null;
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
        targetLanguage: 'en',
        topRequirements: [],
        coverLetterText: '',
        includeCoverLetter: false,
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

  // The trap `usePipelineDraftStream`'s doc comment calls out: both the draft
  // AND the cover_letter stage stream through the SAME `ai:stream` jobId, so
  // without a split every letter token would land on the end of the résumé
  // buffer.
  describe('the letter stream split', () => {
    it('routes deltas before cover_letter starts to `draft`', () => {
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => {
        bus.stage?.(stage('draft', 'start', 3));
        bus.delta?.('resume text');
      });
      expect(result.current.draft).toBe('resume text');
      expect(result.current.letterDraft).toBe('');
    });

    it('routes deltas from the cover_letter stage start onward to `letterDraft`, leaving `draft` frozen', () => {
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => {
        bus.stage?.(stage('draft', 'start', 3));
        bus.delta?.('resume text');
        bus.stage?.(stage('draft', 'finish', 3));
        bus.stage?.(stage('cover_letter', 'start', 4));
        bus.delta?.('Dear hiring team,');
      });
      expect(result.current.draft).toBe('resume text');
      expect(result.current.letterDraft).toBe('Dear hiring team,');
    });

    it('resets both buffers and the split flag on a new run', async () => {
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => {
        bus.stage?.(stage('cover_letter', 'start', 4));
        bus.delta?.('old letter');
      });
      expect(result.current.letterDraft).toBe('old letter');

      await act(async () => {
        await result.current.start({
          resumeId: 'doc-1',
          jobId: 'posting-1',
          jobUrl: '',
          targetLanguage: 'en',
          topRequirements: [],
          coverLetterText: '',
          includeCoverLetter: true,
        });
      });
      expect(result.current.letterDraft).toBe('');
      expect(result.current.draft).toBe('');

      // The split flag reset too — a fresh delta with no stage event yet
      // goes back to the résumé buffer, not the previous run's letter one.
      act(() => bus.delta?.('new resume text'));
      expect(result.current.draft).toBe('new resume text');
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
    const jobFailed = (jobId: string, data?: unknown): JobEvent => ({
      type: 'job.failed',
      jobId,
      ...(data !== undefined ? { data } : {}),
      ts: 1,
    });

    it('ends the session on the umbrella job failure when no record exists', async () => {
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      const { result } = renderHook(() => useResumePipelineSession(RUN_ID, JOB_ID));
      act(() => bus.job?.(jobFailed(JOB_ID, 'job not found in cache: posting-1')));

      await waitFor(() => expect(result.current.state).toBe('error'));
      expect(result.current.busy).toBe(false);
      expect(result.current.error).toContain('job not found in cache');
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
    await act(async () => {
      await result.current.start({
        resumeId: 'doc-9',
        jobId: 'posting-1',
        jobUrl: '',
        targetLanguage: 'en',
        topRequirements: [],
        coverLetterText: '',
        includeCoverLetter: false,
      });
    });
    expect(result.current.state).toBe('error');
    expect(result.current.error).toContain('doc-9');
  });
});
