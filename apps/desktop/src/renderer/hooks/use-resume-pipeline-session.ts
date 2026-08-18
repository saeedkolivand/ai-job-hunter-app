import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import type { JobEvent, PipelineStageEvent } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';
import type { ResumePipelineRunRequest } from '@ajh/shared/schemas';
import { useTranslation } from '@ajh/translations';

import { useMachine } from '@/hooks/use-machine';
import { errorDetail } from '@/lib/error-class';
import {
  type PipelineStageProgress,
  resumePipelineMachine,
  type ResumePipelineState,
  stageToEvent,
  statusToEvent,
} from '@/lib/machines/resume-pipeline.machine';
import { stoppedSuffix } from '@/lib/stopped-reason';
import { fetchJob, useCancelJob, useJobEvents } from '@/services/use-jobs';
import {
  usePipelineDraftStream,
  usePipelineRun,
  usePipelineStageEvents,
  useRefreshRunsForJobOnTerminal,
  useStartResumePipelineRun,
} from '@/services/use-resume-pipeline';

export type { PipelineStageProgress };

export interface ResumePipelineSession {
  state: ResumePipelineState;
  /** The run is still doing work — nothing final to show yet. */
  busy: boolean;
  runId: string | null;
  /** The umbrella job id: what `cancel` targets and what the draft stream keys on. */
  jobId: string | null;
  stage: PipelineStageProgress | null;
  /**
   * The draft stage's streamed text, for DISPLAY ONLY. Not the run's result —
   * `detail.resumeText` is, and it can differ by up to two repair rounds.
   *
   * At MAX depth the same channel carries the progressive assembly: each
   * finished section's text is appended as its stage produces it, so this builds
   * up section by section instead of token by token. Display-only either way.
   *
   * **Stops growing once the `cover_letter` stage starts** — see
   * {@link ResumePipelineSession.letterDraft}.
   */
  draft: string;
  /**
   * The `cover_letter` stage's streamed text, for DISPLAY ONLY — the same
   * `ai:stream` channel as {@link ResumePipelineSession.draft} carries BOTH
   * documents in sequence (draft, then the letter), so this hook splits the
   * one stream at the `cover_letter` stage's `start` event rather than mashing
   * the letter's tokens onto the end of the résumé buffer. Empty for a run
   * that never includes a letter (`cover_letter` still runs but writes
   * nothing) and for a reconnected run (the stream is not replayed).
   */
  letterDraft: string;
  /**
   * Reasoning/thinking tokens from EITHER streamed stage, for DISPLAY ONLY —
   * not split by document (a reasoning bubble reads fine as one continuous
   * transcript; only the finished text needs to land in the right pane).
   */
  thinking: string;
  /** The run record: the authority on status, report, metrics and document. */
  detail: PipelineRunDetail | null;
  /**
   * A start failure, or the stopped reason of a run that ended badly — the
   * live `job.failed` message when one arrived, otherwise (a remount that
   * missed it) the persisted record's own reason. See
   * `persistedFailureReason`'s doc comment for the fallback's limits.
   */
  error: string | null;
  starting: boolean;
  start: (req: ResumePipelineRunRequest) => Promise<string | null>;
  cancel: () => void;
  reset: () => void;
}

/**
 * `job.failed`'s structured payload for a per-call timeout — see
 * `hooks::timeout_failure_data` on the Rust side, the ONLY producer of this
 * shape. `unknown`-safe: every OTHER `job.failed` (scrape, ai.generate,
 * autopilot, …) still sends a plain string, and `handleJobEvent` below must
 * keep treating those exactly as before.
 */
function timeoutFailureDetail(data: unknown): { stage: string; seconds: number } | null {
  if (typeof data !== 'object' || data === null || (data as { kind?: unknown }).kind !== 'timeout')
    return null;
  const { stage, seconds } = data as { stage?: unknown; seconds?: unknown };
  return typeof stage === 'string' && typeof seconds === 'number' ? { stage, seconds } : null;
}

/**
 * Session state for one staged ("quality depth") résumé run.
 *
 * Owns four things a panel would otherwise re-derive: the coarse machine state,
 * the live stage counter, the display-only draft stream, and the run RECORD —
 * which is the only reliable completion signal (see
 * `resume-pipeline.machine`'s module doc). A run that was cancelled or hit its
 * deadline at a stage boundary produces no terminal stage event at all, so
 * terminal detection here reads `detail.status` and nothing else.
 *
 * @param initialRunId Reconnect target. A panel that remounts mid-run (the
 *   modal was closed and reopened, the route changed) passes the run id it
 *   remembers — the hook re-reads the record, replays its persisted stage trail
 *   to recover the counter, and resumes polling if the run is still going. The
 *   draft text is NOT recoverable that way (it was never persisted as a
 *   stream), so a reconnected run shows the finished document instead.
 */
export function useResumePipelineSession(
  initialRunId?: string | null,
  initialJobId?: string | null
): ResumePipelineSession {
  // A reconnect starts BUSY, not idle. Two things depend on it: `idle` accepts
  // only `START` (so a stage event arriving for the run we just re-attached to
  // would be dropped on the floor), and the record poll is gated on `busy` —
  // the one mechanism that notices a run which ended at a stage boundary and
  // therefore emitted no terminal event. A run that turns out to be finished
  // leaves `queued` on its first fetched record, which costs one render.
  const { t } = useTranslation();
  const [state, send] = useMachine(resumePipelineMachine, initialRunId ? 'queued' : 'idle');
  const [runId, setRunId] = useState<string | null>(initialRunId ?? null);
  const [jobId, setJobId] = useState<string | null>(initialJobId ?? null);
  const [stage, setStage] = useState<PipelineStageProgress | null>(null);
  const [draft, setDraft] = useState('');
  const [letterDraft, setLetterDraft] = useState('');
  const [thinking, setThinking] = useState('');
  const [error, setError] = useState<string | null>(null);
  // Flips once, at the `cover_letter` stage's `start` event — see
  // `ResumePipelineSession.letterDraft`. A ref because it must be readable
  // from `appendDraft` without re-subscribing the stream on every stage event.
  const writingLetterRef = useRef(false);

  const startRun = useStartResumePipelineRun();
  const cancelJob = useCancelJob();

  const busy = resumePipelineMachine.busyStates?.includes(state) ?? false;
  const {
    data: detail = null,
    isError: recordFailed,
    error: recordError,
  } = usePipelineRun(runId, busy);

  // `runId` is state, so the subscription handlers below need the CURRENT one
  // without re-subscribing on every change (a re-subscribe drops events in the
  // gap). Same discipline as `useNotificationEvents`' setter ref.
  const runIdRef = useRef(runId);
  runIdRef.current = runId;

  const handleStage = useCallback(
    (event: PipelineStageEvent) => {
      // Every in-flight run broadcasts on this channel — take only our own.
      if (!runIdRef.current || event.runId !== runIdRef.current) return;
      setStage({
        stage: event.stage,
        phase: event.phase,
        index: event.index,
        total: event.total,
        attempt: event.attempt,
        ...(event.issueCount != null ? { issueCount: event.issueCount } : {}),
        ...(event.criticalCount != null ? { criticalCount: event.criticalCount } : {}),
      });
      // The one thing that decides which buffer new deltas land in — flips
      // ONCE, at the letter stage's start, and never back: nothing after it
      // (validate/repair/humanize) streams visibly, so there is nothing left
      // to misroute once it is set.
      if (event.stage === 'cover_letter' && event.phase === 'start') {
        writingLetterRef.current = true;
      }
      const next = stageToEvent(event.stage, event.phase);
      if (next) send(next);
    },
    [send]
  );
  usePipelineStageEvents(handleStage);

  const appendDraft = useCallback((delta: string) => {
    if (writingLetterRef.current) setLetterDraft((prev) => prev + delta);
    else setDraft((prev) => prev + delta);
  }, []);
  const appendThinking = useCallback((delta: string) => setThinking((prev) => prev + delta), []);
  usePipelineDraftStream(jobId, appendDraft, appendThinking);

  /**
   * The failures the run RECORD can never report, and the one job event that can.
   *
   * `resume_pipeline_run` returns its two ids immediately and writes the
   * `pipeline_runs` row inside the spawned task — AFTER admission and after it
   * resolves the depth, the provider, the résumé and the cached posting. Each of
   * those five can fail (a full queue, no configured provider, a deleted résumé,
   * a posting that is not in the live cache, an unknown depth string) and lands on
   * `job_fail` with NO row ever written; `get(runId)` then answers `null` — a
   * real answer, so the poll correctly stops — and `detail.status`, the only
   * completion signal this hook has, never exists. The same hole exists at the
   * other end: if the final `upsert_run` is what failed, the row is left saying
   * `running` forever. Either way the session spins on a run that has ended.
   *
   * So the umbrella job's `job.failed` is consumed WHILE THE MACHINE IS BUSY —
   * not "while no record exists", which misses the stuck-`running` row. It is
   * safe against the record because the backend's terminal arms are exclusive:
   * `job_fail` is emitted only when the row is (or would be) `failed`; a
   * cancelled run gets `job_cancel`, and a deadline-stopped run that still saved
   * its document gets `job_complete` precisely so the renderer does not discard
   * a document the row calls reviewable. A run that already reached a terminal
   * state is left alone, error text included. `job.completed` is never read here
   * for the reason the module doc gives: the draft stream fires it mid-run.
   *
   * This live path still misses the EARLIEST instance of the same failure: one
   * emitted before `start`'s `mutateAsync` resolves, while `jobIdRef` is still
   * null/stale. `start` reconciles that window itself with one `fetchJob` read
   * right after the ids land — and assigns `jobIdRef`/`runIdRef` synchronously
   * at that same point, rather than leaving them to the next render commit,
   * so a `job.failed` (or `pipeline:stage`) delivered between the reconcile
   * read and that commit is not read against a stale ref either.
   */
  const jobIdRef = useRef(jobId);
  jobIdRef.current = jobId;
  const busyRef = useRef(busy);
  busyRef.current = busy;
  const handleJobEvent = useCallback(
    (event: JobEvent) => {
      if (event.type !== 'job.failed') return;
      if (!jobIdRef.current || event.jobId !== jobIdRef.current) return;
      if (!busyRef.current) return;
      console.error('[resumePipeline] the umbrella job failed', { jobId: event.jobId });
      // A per-call timeout carries structured `{ stage, seconds }` instead of
      // the plain string every other `job.failed` sends — localize it through
      // `pipeline.timeout`, mapping the raw stage key through the SAME
      // `pipeline.stage.*` labels the step tracker already shows (never the
      // raw snake_case name a user has never seen). Every other failure keeps
      // reading `event.data` as the plain string it has always been.
      const timeout = timeoutFailureDetail(event.data);
      setError(
        timeout
          ? t('pipeline.timeout', {
              stage: t(`pipeline.stage.${timeout.stage}`, { defaultValue: timeout.stage }),
              seconds: timeout.seconds,
            })
          : typeof event.data === 'string' && event.data
            ? event.data
            : null
      );
      send('ERROR');
    },
    [send, t]
  );
  useJobEvents(handleJobEvent);

  // Terminal detection, and the ONLY place it happens: the record's status.
  // Not the last stage's `finish` (a boundary stop never reaches one), not the
  // umbrella job's `job.completed` (the draft stream fires that mid-run), not
  // `awaitAiStream` (same stream, same problem).
  const status = detail?.status;
  useEffect(() => {
    if (!status) return;
    const next = statusToEvent(status);
    if (next) send(next);
  }, [status, send]);

  /**
   * The failure reason for a run whose ONLY trace is the record — a remount
   * after a `job.failed` the live listener above never saw. `error` (the
   * `useState` above) is empty in exactly that case: it is written only by
   * `handleJobEvent`/`start`, both of which require a listener mounted at the
   * moment the event fired, which a fresh mount by definition was not.
   *
   * Reuses the SAME wire→label mapping `ResultsPanel`/`PipelineRunsList`
   * already use for this exact field, rather than inventing a second one —
   * it will never carry the live path's stage/seconds specificity (the row
   * persists `stoppedReason`, not the per-call timeout's duration), but it is
   * a real, localized reason instead of the silence a bare `error` leaves.
   * `null` on a `failed` run with no recorded reason (a provider error, a
   * store failure) — there's nothing to say — and on every non-`failed`
   * status, where `error` is not what renders the outcome.
   */
  const persistedFailureReason = useMemo(() => {
    if (status !== 'failed') return null;
    const suffix = stoppedSuffix(detail?.stoppedReason);
    return suffix ? t(`pipeline.stopped.${suffix}`) : null;
  }, [status, detail?.stoppedReason, t]);

  // …and the same discovery is the only thing that can tell the posting's run
  // LIST its run just ended. Nothing was clicked, so none of the three
  // action-driven invalidators fires, and the list has no poll of its own: left
  // alone it renders this run as "Running" indefinitely. The run record is also
  // the authority on which posting to refresh — `jobUrl` off the row, never a
  // caller-supplied one that could disagree with it.
  useRefreshRunsForJobOnTerminal(detail?.jobUrl, status);

  /**
   * The record read itself failed and NOTHING has been fetched yet.
   *
   * That combination is fatal to this session and has to be said out loud: the
   * status is the only completion signal, so a session that never gets a first
   * record has no way to ever leave a busy state — the panel spins forever on a
   * request that already gave up. Surfacing it puts the machine in `error`,
   * where the surface's existing retry (`start` again) and cancel affordances
   * live.
   *
   * Gated on `!detail` deliberately: once a record HAS landed, a later poll
   * failure is a blip, the run is still going, and react-query keeps polling —
   * killing a live run over one dropped read would be the opposite mistake.
   */
  useEffect(() => {
    if (!recordFailed || detail) return;
    console.error('[resumePipeline] reading the run record failed', {
      error: errorDetail(recordError),
    });
    setError(recordError instanceof Error ? recordError.message : String(recordError));
    send('ERROR');
  }, [recordFailed, detail, recordError, send]);

  // Replay a reconnected run's persisted stage trail so the counter is right
  // after a remount. Only while the machine is still at `idle`/`queued` — once
  // live events are arriving they are fresher than the trail.
  const events = detail?.events;
  const replayed = useRef<string | null>(null);
  useEffect(() => {
    if (!runId || !events?.length || replayed.current === runId) return;
    replayed.current = runId;
    const last = events[events.length - 1];
    if (!last) return;
    setStage(
      (current) =>
        current ?? {
          stage: last.stage,
          phase: last.phase,
          index: events.filter((e) => e.phase === 'start').length - 1,
          total: 0,
          attempt: 1,
        }
    );
    const next = stageToEvent(last.stage, last.phase);
    if (next) send(next);
  }, [runId, events, send]);

  const start = useCallback(
    async (req: ResumePipelineRunRequest) => {
      setError(null);
      setStage(null);
      setDraft('');
      setLetterDraft('');
      setThinking('');
      writingLetterRef.current = false;
      replayed.current = null;
      send('START');
      // `jobIdRef`/`runIdRef`/`busyRef` are all written in the render body
      // (see below), so the `setState` calls in this function only reach
      // them once React actually re-renders — which is NOT guaranteed to
      // happen before this async function's next `await` resumes. A promise
      // that resolves entirely through the microtask queue (every mocked
      // service call in this hook's tests; a real `invoke` can too, once its
      // one native round trip is done) never yields the macrotask turn
      // React's scheduler needs, so `start` can run start-to-finish without
      // a single commit landing in between. Any live listener reading one of
      // these refs in that window — `job.failed`/`pipeline:stage` via
      // `jobIdRef`/`runIdRef`, `job.failed`'s busy gate via `busyRef` — sees
      // it null/stale/false and drops the event, even after this function's
      // OWN reconcile read already came back clean. `busy` is knowable
      // synchronously right here: `START` always lands on a busy state (see
      // `resumePipelineMachine`), so there is nothing to compute.
      busyRef.current = true;
      try {
        const started = await startRun.mutateAsync(req);
        setRunId(started.runId);
        setJobId(started.jobId);
        // Same reasoning as above, for the ids: assign both refs here,
        // synchronously, the instant they are known, rather than leaving
        // them to the next render commit.
        jobIdRef.current = started.jobId;
        runIdRef.current = started.runId;
        // The umbrella job can also already be `failed` by the time this
        // promise resolves: `resume_pipeline_run` spawns its task and returns
        // the ids immediately, so an admission/validation failure inside that
        // task (a full queue, no configured provider, …) can reach `job_fail`
        // before this `await` returns — see that command's doc comment. And
        // no `pipeline_runs` row was ever written for that failure either, so
        // nothing else would ever end this session. One reconcile read
        // against the (cheap, in-memory) `JobTracker` closes that window too.
        const job = (await fetchJob(started.jobId)) as { status?: string; error?: string } | null;
        if (job?.status === 'failed') {
          console.error('[resumePipeline] the umbrella job had already failed', {
            jobId: started.jobId,
          });
          setError(typeof job.error === 'string' && job.error ? job.error : null);
          send('ERROR');
          return null;
        }
        return started.runId;
      } catch (err) {
        console.error('[resumePipeline] start failed', { error: errorDetail(err) });
        setError(err instanceof Error ? err.message : String(err));
        send('ERROR');
        return null;
      }
    },
    [send, startRun]
  );

  /**
   * Cancel through the ordinary job path — the umbrella id from `run`. It
   * aborts the draft stream immediately and every other stage at its next
   * boundary, so the machine is NOT flipped to `cancelled` here: the backend
   * decides (a run that finished a millisecond earlier finished), and the
   * record says so.
   */
  const cancel = useCallback(() => {
    if (!jobId) return;
    cancelJob.mutate(jobId);
  }, [cancelJob, jobId]);

  const reset = useCallback(() => {
    send('RESET');
    setRunId(null);
    setJobId(null);
    setStage(null);
    setDraft('');
    setLetterDraft('');
    setThinking('');
    writingLetterRef.current = false;
    setError(null);
    replayed.current = null;
    // Same reasoning as `start` (see the `jobIdRef` doc comment above): these
    // three are written only in the render body, so a `pipeline:stage`/
    // `job.failed` listener still mounted on the OLD run/job can fire in the
    // gap before this function's `setState` calls actually commit — reading
    // the stale ids/busy flag and misattributing the event to whatever
    // session starts next. Assign them synchronously, right here, instead of
    // leaving them to the next render.
    runIdRef.current = null;
    jobIdRef.current = null;
    busyRef.current = false;
  }, [send]);

  return {
    state,
    busy,
    runId,
    jobId,
    stage,
    draft,
    letterDraft,
    thinking,
    detail,
    // The live message wins when one landed; otherwise fall back to what the
    // record itself says — see `persistedFailureReason`'s doc comment.
    error: error ?? persistedFailureReason,
    starting: startRun.isPending,
    start,
    cancel,
    reset,
  };
}
