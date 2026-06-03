import { useRef, useState } from 'react';

import {
  calculateDownloadSpeed,
  calculateTimeRemaining,
  formatDownloadSpeed,
  formatTimeRemaining,
} from '@ajh/shared';
import { useNotification } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useJobEvents, usePullModel } from '@/services';

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

  /** Clear the transient per-download tracking (job id, speed, ETA, byte counters). */
  const resetTracking = () => {
    setPullJobId(null);
    setDownloadSpeed('');
    setTimeRemaining('');
    setDownloadedBytes(0);
    setTotalBytes(0);
    prevBytesRef.current = 0;
    prevTimeRef.current = 0;
    lastSpeedUpdateRef.current = 0;
    lastTimeUpdateRef.current = 0;
  };

  const finishOk = () => {
    setPullProgress(100);
    setPullState('done');
    resetTracking();
    notify(t('onboarding.ai.downloaded', { model: selectedModel }), 'success');
    onDownloadComplete?.();
  };

  const handlePull = async () => {
    setPullState('pulling');
    setPullProgress(0);
    try {
      const result = await pullModel.mutateAsync(selectedModel);
      setPullJobId(result.jobId);
    } catch (err) {
      setPullState('error');
      notify(err instanceof Error ? err.message : 'Download failed.', 'error');
    }
  };

  useJobEvents((event) => {
    if (event.type === 'job.stream' && event.jobId === pullJobId) {
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
        finishOk();
      }
    } else if (event.type === 'job.completed' && event.jobId === pullJobId) {
      finishOk();
    } else if (event.type === 'job.failed' && event.jobId === pullJobId) {
      setPullState('error');
      resetTracking();
      notify(t('onboarding.ai.downloadFailed'), 'error');
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
