import { useMemo } from 'react';

import type { JobRecord } from '../types';

export function useJobMetrics(allJobs: JobRecord[]) {
  const activeJobs = useMemo(
    () =>
      allJobs.filter(
        (j) => j.status === 'queued' || j.status === 'running' || j.status === 'streaming'
      ),
    [allJobs]
  );
  const completedCount = useMemo(
    () => allJobs.filter((j) => j.status === 'completed').length,
    [allJobs]
  );
  const failedCount = useMemo(
    () => allJobs.filter((j) => j.status === 'failed' || j.status === 'cancelled').length,
    [allJobs]
  );

  const total = completedCount + failedCount;
  const successRate = total ? Math.round((completedCount / total) * 100) : 100;
  const counters = { completed: completedCount, running: activeJobs.length, failed: failedCount };

  return { activeJobs, counters, successRate };
}
