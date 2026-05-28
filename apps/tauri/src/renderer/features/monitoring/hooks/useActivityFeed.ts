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

  // Historical activity from completed/failed jobs
  const historicalActivity = useMemo(() => {
    return allJobs
      .filter((j) => j.status === 'completed' || j.status === 'failed' || j.status === 'cancelled')
      .sort((a, b) => (b.finishedAt ?? b.updatedAt) - (a.finishedAt ?? a.updatedAt))
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
          time: j.finishedAt ?? j.updatedAt,
          text: `${verb} ${kindLabelMap[j.kind] ?? j.kind}`,
          tone,
        };
      });
  }, [allJobs, kindLabelMap]);

  // Merge live (top) with historical (deduped)
  const activity = useMemo(() => {
    const liveJobIds = new Set(liveActivity.map((a) => a.id.split('-')[0]));
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

  return { activity, liveActivity, setLiveActivity };
}
