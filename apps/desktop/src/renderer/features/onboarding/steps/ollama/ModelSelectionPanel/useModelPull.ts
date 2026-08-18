import { useCallback, useEffect, useRef, useState } from 'react';

import {
  calculateDownloadSpeed,
  calculateTimeRemaining,
  formatDownloadSpeed,
  formatTimeRemaining,
} from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { fetchJob, useJobEvents, useJobQueue, usePullModel } from '@/services';

type PullState = 'idle' | 'pulling' | 'done' | 'error';

interface Params {
  selectedModel: string;
  onDownloadComplete?: () => void;
}

/** Pulls an Ollama model and tracks download progress/speed/ETA from job events. */
export function useModelPull({ selectedModel, onDownloadComplete }: Params) {
  const { t } = useTranslation();
  const notify = useNotification();
  const pullModel = usePullModel();
  const jobQueue = useJobQueue();

  const [pullState, setPullState] = useState<PullState>('idle');
  const [pullProgress, setPullProgress] = useState(0);
  const [pullJobId, setPullJobId] = useState<string | null>(null);
  const [downloadSpeed, setDownloadSpeed] = useState('');
  const [timeRemaining, setTimeRemaining] = useState('');
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [totalBytes, setTotalBytes] = useState(0);
  const prevBytesRef = useRef(0);
  const prevTimeRef = useRef(0);
  const lastSpeedUpdateRef = useRef(0);
  const lastTimeUpdateRef = useRef(0);

  // The reattach effect's reconcile read below needs the CURRENT `pullJobId`
  // from inside an async `.then()`, where a closure only has the value from
  // whichever render scheduled it — a ref synced every render is the
  // standard way to read "now" from there (same discipline as
  // `useResumePipelineSession`'s `jobIdRef`).
  const pullJobIdRef = useRef(pullJobId);
  pullJobIdRef.current = pullJobId;
  // True until this hook instance unmounts. Deliberately its OWN effect with
  // an empty dep array — tying it to the reattach effect below would flip it
  // false the instant that effect adopts a job (its deps include `pullJobId`,
  // which the adoption itself changes), cancelling that effect's own
  // reconcile read before it can resolve.
  const mountedRef = useRef(true);
  useEffect(
    () => () => {
      mountedRef.current = false;
    },
    []
  );
  // Job ids this hook instance has already settled (via the reconcile read
  // below or a live terminal event) — see the reattach effect for why this
  // is needed to stop it re-adopting the same job forever.
  const settledJobIdsRef = useRef<Set<string>>(new Set());

  /** Clear the transient per-download tracking (job id, speed, ETA, byte counters). */
  const resetTracking = useCallback(() => {
    // Synchronous, same reasoning as the adopt/handlePull assignments below:
    // a `setState` call does not update the ref for anything that reads it
    // before the next render commits.
    pullJobIdRef.current = null;
    setPullJobId(null);
    setDownloadSpeed('');
    setTimeRemaining('');
    setDownloadedBytes(0);
    setTotalBytes(0);
    prevBytesRef.current = 0;
    prevTimeRef.current = 0;
    lastSpeedUpdateRef.current = 0;
    lastTimeUpdateRef.current = 0;
  }, []);

  // Both take the settling job's id so they can record it as settled — see
  // `settledJobIdsRef` above.
  const finishOk = useCallback(
    (jobId: string) => {
      settledJobIdsRef.current.add(jobId);
      setPullProgress(100);
      setPullState('done');
      resetTracking();
      notify.success({ message: t('onboarding.ai.downloaded', { model: selectedModel }) });
      onDownloadComplete?.();
    },
    [resetTracking, notify, t, selectedModel, onDownloadComplete]
  );

  const finishFailed = useCallback(
    (jobId: string) => {
      settledJobIdsRef.current.add(jobId);
      setPullState('error');
      resetTracking();
      notify.error({ message: t('onboarding.ai.downloadFailed') });
    },
    [resetTracking, notify, t]
  );

  const handlePull = async () => {
    setPullState('pulling');
    setPullProgress(0);
    try {
      const result = await pullModel.mutateAsync(selectedModel);
      pullJobIdRef.current = result.jobId;
      setPullJobId(result.jobId);
    } catch (err) {
      setPullState('error');
      notify.error({ message: err instanceof Error ? err.message : 'Download failed.' });
    }
  };

  // Re-attach to a pull already running in the backend job registry — `pullJobId`
  // lives only in this hook's own state, so ANY unmount of this panel (not just
  // navigating away: switching to the Cloud/CLI tab and back, or Back/Forward
  // through the wizard) loses it forever and no later job.stream/job.completed
  // can match again. `ai_pull_model` is exclusive, so at most one can be running
  // app-wide — reattaching to whichever the registry still reports means this
  // panel resumes tracking the SAME job instead of quietly going deaf.
  useEffect(() => {
    if (pullJobId) return;
    const active = jobQueue.data?.find(
      (job) =>
        job.kind === 'ai.pull_model' &&
        !settledJobIdsRef.current.has(job.id) &&
        (job.status === 'running' || job.status === 'streaming' || job.status === 'queued')
    );
    if (!active) return;

    // Synchronous — `pullJobIdRef` must read as "now watching this job"
    // before the microtask queue below (and `useJobEvents`, if a real event
    // is already pending) gets a turn. A `setState` call alone does not: it
    // only updates the ref on the NEXT render commit, and an already-resolved
    // mock promise (a real IPC response resolves the same way) can unwind
    // entirely through microtasks without ever yielding the macrotask turn
    // React's scheduler needs — same discipline as `useResumePipelineSession`.
    pullJobIdRef.current = active.id;
    setPullJobId(active.id);
    setPullState('pulling');

    // The registry snapshot above can already be stale by the time this
    // commits: `ai.pull_model` fires its ONE job.completed/job.failed the
    // instant the pull ends, and `useJobEvents` below drops any event whose
    // jobId doesn't match a `pullJobId` that isn't set yet — `jobQueue.data`
    // takes a full IPC round trip to resolve, so a terminal event landing in
    // that gap is gone for good; no second one is coming to correct it.
    // Re-read this job's OWN current status right after adopting it so the
    // panel settles regardless of whether that race actually happened,
    // instead of depending on catching the event at the right moment.
    void fetchJob(active.id)
      .then((job) => {
        if (!mountedRef.current || pullJobIdRef.current !== active.id) return;
        if (job?.status === 'completed') {
          finishOk(active.id);
        } else if (job?.status === 'failed') {
          finishFailed(active.id);
        }
      })
      .catch((err) => {
        console.error('[modelPull] reconcile read failed', { jobId: active.id, err });
      });
  }, [jobQueue.data, pullJobId, finishOk, finishFailed]);

  useJobEvents((event) => {
    if (event.type === 'job.stream' && event.jobId === pullJobIdRef.current) {
      const data = event.data as {
        status?: string;
        p?: number;
        completed?: number;
        total?: number;
      };
      if (typeof data?.p === 'number') {
        setPullProgress(data.p * 100);
      }

      if (typeof data?.completed === 'number') {
        setDownloadedBytes(data.completed);
      }
      if (typeof data?.total === 'number' && data.total > 0) {
        setTotalBytes(data.total);
      }

      if (typeof data?.completed === 'number' && typeof data?.total === 'number') {
        const now = Date.now();
        const bytes = data.completed;
        const prevBytes = prevBytesRef.current;
        const prevTime = prevTimeRef.current;

        if (prevTime > 0 && bytes > prevBytes) {
          const bytesPerSecond = calculateDownloadSpeed(bytes, prevBytes, now, prevTime);

          if (bytesPerSecond > 0) {
            if (now - lastSpeedUpdateRef.current > 500) {
              setDownloadSpeed(formatDownloadSpeed(bytesPerSecond));
              lastSpeedUpdateRef.current = now;
            }

            if (totalBytes > 0 && downloadedBytes > 0 && downloadedBytes < totalBytes) {
              if (now - lastTimeUpdateRef.current > 500) {
                const remainingSeconds = calculateTimeRemaining(
                  totalBytes,
                  downloadedBytes,
                  bytesPerSecond
                );
                setTimeRemaining(formatTimeRemaining(remainingSeconds));
                lastTimeUpdateRef.current = now;
              }
            }
          }
        }

        prevBytesRef.current = bytes;
        prevTimeRef.current = now;
      }

      if (data?.status === 'success') {
        finishOk(event.jobId);
      }
    } else if (event.type === 'job.completed' && event.jobId === pullJobIdRef.current) {
      finishOk(event.jobId);
    } else if (event.type === 'job.failed' && event.jobId === pullJobIdRef.current) {
      finishFailed(event.jobId);
    }
  });

  return {
    pullState,
    pullProgress,
    downloadSpeed,
    timeRemaining,
    downloadedBytes,
    totalBytes,
    handlePull,
  };
}
