/**
 * Stale-detection threshold: 30 days in milliseconds.
 * A pursuit is 'stale' when updatedAt is older than this with no nextActionAt set.
 */
const STALE_THRESHOLD_MS = 30 * 24 * 60 * 60 * 1000;

/**
 * Returns true when an application has had no status update for >= STALE_THRESHOLD_MS.
 * Used to render the 'No reply · Nd' staleness badge on ApplicationRow.
 */
export function isStale(updatedAt: number, thresholdMs = STALE_THRESHOLD_MS): boolean {
  return Date.now() - updatedAt >= thresholdMs;
}

/**
 * Returns 'none' when no nextActionAt, 'overdue' when it is in the past,
 * or 'upcoming' when it is still in the future.
 */
export type NextActionState = 'none' | 'upcoming' | 'overdue';

export function nextActionLabel(nextActionAt?: number): NextActionState {
  if (!nextActionAt) return 'none';
  return nextActionAt < Date.now() ? 'overdue' : 'upcoming';
}

/** Format staleness in days, e.g. '14d'. */
export function staleDays(updatedAt: number): number {
  return Math.floor((Date.now() - updatedAt) / (24 * 60 * 60 * 1000));
}
