import { useMemo, useState } from 'react';

import type { ActivityItem, JobRecord } from '@/features/monitoring/types';
import { fetchJob, useJobEvents } from '@/services';

interface JobEvent {
  type:
    | 'job.queued'
    | 'job.started'
    | 'job.progress'
    | 'job.stream'
    | 'job.completed'
    | 'job.failed'
    | 'job.cancelled';
  jobId: string;
  data?: unknown;
  ts: number;
}

export function useActivityFeed(allJobs: JobRecord[], kindLabelMap: Record<string, string>) {
  const [liveActivity, setLiveActivity] = useState<ActivityItem[]>([]);
  const [clearedBefore, setClearedBefore] = useState<number>(0);

  // Historical activity from completed/failed jobs
  const historicalActivity = useMemo(() => {
    return allJobs
      .filter((j) => j.status === 'completed' || j.status === 'failed' || j.status === 'cancelled')
      .filter((j) => (j.finishedAt ?? j.updatedAt ?? j.createdAt) > clearedBefore)
      .sort(
        (a, b) =>
          (b.finishedAt ?? b.updatedAt ?? b.createdAt) -
          (a.finishedAt ?? a.updatedAt ?? a.createdAt)
      )
      .slice(0, 40)
      .map((j) => {
        const verb = j.status === 'completed' ? '✓' : j.status === 'failed' ? '✕' : '⊘';
        const tone: ActivityItem['tone'] =
          j.status !== 'completed'
            ? 'amber'
            : j.kind?.startsWith('scrape')
              ? 'violet'
              : j.kind?.startsWith('ai')
                ? 'indigo'
                : 'emerald';
        return {
          id: j.id,
          time: j.finishedAt ?? j.updatedAt ?? j.createdAt,
          text: `${verb} ${kindLabelMap[j.kind] ?? j.kind}`,
          tone,
        };
      });
  }, [allJobs, kindLabelMap, clearedBefore]);

  // Merge live (top) with historical (deduped)
  const activity = useMemo(() => {
    // A live item's id is `${event.jobId}-${event.ts}` and a job id is itself
    // `job-<uuid>`, so `split('-')[0]` was ALWAYS the literal "job": the set held
    // one useless entry, no historical id ever matched it, and every completed
    // job rendered twice (different React keys, so nothing else caught it).
    // Strip only the trailing `-<ts>` to recover the real job id.
    const liveJobIds = new Set(
      liveActivity.map((a) => {
        const cut = a.id.lastIndexOf('-');
        return cut > 0 ? a.id.slice(0, cut) : a.id;
      })
    );
    return [...liveActivity, ...historicalActivity.filter((a) => !liveJobIds.has(a.id))].slice(
      0,
      40
    );
  }, [liveActivity, historicalActivity]);

  // Subscribe to job events — prepend fresh events to live list
  useJobEvents((ev: unknown) => {
    const event = ev as JobEvent;
    void (async () => {
      const job = (await fetchJob(event.jobId)) as JobRecord | null;
      const kindLabel = (job?.kind && kindLabelMap[job.kind]) ?? 'Job';
      const tone: ActivityItem['tone'] =
        event.type === 'job.completed'
          ? job?.kind?.startsWith('scrape')
            ? 'violet'
            : job?.kind?.startsWith('ai')
              ? 'indigo'
              : 'emerald'
          : event.type === 'job.failed' || event.type === 'job.cancelled'
            ? 'amber'
            : 'blue';

      if (['job.completed', 'job.failed', 'job.cancelled', 'job.started'].includes(event.type)) {
        const verb =
          event.type === 'job.completed'
            ? '✓'
            : event.type === 'job.failed'
              ? '✕'
              : event.type === 'job.cancelled'
                ? '⊘'
                : '▸';
        setLiveActivity((prev) =>
          [
            {
              id: `${event.jobId}-${event.ts}`,
              time: event.ts,
              text: `${verb} ${kindLabel}`,
              tone,
            },
            ...prev,
          ].slice(0, 40)
        );
      }
    })();
  });

  const clearAll = () => {
    setLiveActivity([]);
    setClearedBefore(Date.now());
  };

  return { activity, liveActivity, setLiveActivity, clearAll };
}
