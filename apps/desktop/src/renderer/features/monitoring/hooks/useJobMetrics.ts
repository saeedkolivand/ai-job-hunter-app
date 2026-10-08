import { useMemo } from 'react';

import type { JobRecord } from '@/features/monitoring/types';

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
  const failedCount = useMemo(() => allJobs.filter((j) => j.status === 'failed').length, [allJobs]);
  // A user-cancelled job is neither a success nor a failure, so it stays out of
  // the success-rate denominator.
  const cancelledCount = useMemo(
    () => allJobs.filter((j) => j.status === 'cancelled').length,
    [allJobs]
  );

  const total = completedCount + failedCount;
  const successRate = total ? Math.round((completedCount / total) * 100) : 100;
  const counters = {
    completed: completedCount,
    running: activeJobs.length,
    failed: failedCount,
    cancelled: cancelledCount,
  };

  return { activeJobs, counters, successRate };
}
