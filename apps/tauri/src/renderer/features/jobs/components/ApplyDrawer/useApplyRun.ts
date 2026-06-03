import { useRef, useState } from 'react';

import { useApplyJob, useCancelJob, useJobEvents } from '@/services';

import type { ApplyStep, Posting } from './types';

const APPLIABLE = new Set(['linkedin', 'indeed', 'greenhouse', 'workday', 'xing', 'glassdoor']);

interface StartOptions {
  coverLetter: string;
  autoSubmit: boolean;
}

/** Drives an auto-apply job: starts it, tracks step/progress events, and the outcome. */
export function useApplyRun(posting: Posting) {
  const [running, setRunning] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  const [steps, setSteps] = useState<ApplyStep[]>([]);
  const [outcome, setOutcome] = useState<{ ok: boolean; submitted: boolean; note?: string } | null>(
    null
  );
  const jobRef = useRef<string | null>(null);
  const applyJob = useApplyJob();
  const cancelJobMutation = useCancelJob();

  jobRef.current = jobId;

  useJobEvents((raw: unknown) => {
    const ev = raw as { type: string; jobId: string; data?: unknown; ts: number };
    if (ev.jobId !== jobRef.current) return;
    if (ev.type === 'job.stream') {
      const data = ev.data as ApplyStep;
      setSteps((prev) =>
        [
          ...prev,
          {
            ts: ev.ts,
            stage: data.stage,
            ok: 'ok' in data ? data.ok : true,
            ...('note' in data && data.note ? { note: data.note } : {}),
            kind: data.kind,
            ...(data.kind === 'progress' ? { p: data.p } : {}),
          },
        ].slice(-40)
      );
    } else if (ev.type === 'job.completed') {
      const r = ev.data as { ok: boolean; submitted: boolean; note?: string };
      setOutcome(r);
      setRunning(false);
    } else if (ev.type === 'job.failed' || ev.type === 'job.cancelled') {
      setOutcome({ ok: false, submitted: false, note: String(ev.data ?? 'failed') });
      setRunning(false);
    }
  });

  const start = async ({ coverLetter, autoSubmit }: StartOptions) => {
    setSteps([]);
    setOutcome(null);
    setRunning(true);
    try {
      const res = await applyJob.mutateAsync({
        board: posting.source,
        url: posting.url,
        ...(coverLetter.trim() ? { coverLetter: coverLetter.trim() } : {}),
        autoSubmit,
      });
      jobRef.current = res.jobId;
      setJobId(res.jobId);
    } catch (err) {
      setOutcome({
        ok: false,
        submitted: false,
        note: err instanceof Error ? err.message : String(err),
      });
      setRunning(false);
    }
  };

  const cancel = async () => {
    if (jobId) await cancelJobMutation.mutateAsync(jobId);
  };

  const canApply = APPLIABLE.has(posting.source);

  return { steps, outcome, running, start, cancel, canApply };
}
