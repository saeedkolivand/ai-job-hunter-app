import type { JobRecord } from '@/features/monitoring/types';

/** Number of hourly buckets in the sparkline — one per hour of the window. */
export const LAST_24H_BUCKETS = 24;

const HOUR_MS = 60 * 60 * 1000;

/**
 * Bucket completed jobs into the last 24 hours, oldest bucket first.
 *
 * Two things this deliberately does NOT do, because the previous version did
 * both: it does not include jobs from outside the window, and it does not bucket
 * by hour-OF-DAY. `new Date(ts).getHours()` collapses every completion into a
 * 0–23 slot regardless of date, so a job from three days ago at 3pm landed in
 * the same bar as today's 3pm — under a "Last 24 hours" label.
 *
 * `now` is a parameter so the caller owns the clock (and tests can pin it).
 */
export function binLast24Hours(jobs: JobRecord[], now: number): number[] {
  const bins = Array.from({ length: LAST_24H_BUCKETS }, () => 0);
  const windowStart = now - LAST_24H_BUCKETS * HOUR_MS;

  for (const job of jobs) {
    if (job.status !== 'completed') continue;
    const ts = job.finishedAt ?? job.updatedAt;
    if (ts <= windowStart || ts > now) continue;
    // Oldest bucket first, so the bars read left → right chronologically.
    const hoursAgo = Math.floor((now - ts) / HOUR_MS);
    const bin = LAST_24H_BUCKETS - 1 - Math.min(hoursAgo, LAST_24H_BUCKETS - 1);
    bins[bin] = (bins[bin] ?? 0) + 1;
  }

  return bins;
}
