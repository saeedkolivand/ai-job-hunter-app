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
 * Returns 'none' when no nextActionAt, 'overdue' once the due DAY has passed,
 * or 'upcoming' while it is still today or later.
 */
export type NextActionState = 'none' | 'upcoming' | 'overdue';

export function nextActionLabel(nextActionAt?: number): NextActionState {
  if (!nextActionAt) return 'none';
  // Compared against the start of TODAY, not `Date.now()`. The field is a
  // `<input type="date">`, so it stores local midnight — against `now` a reminder
  // set for today reads "Overdue" from 00:01 onwards, i.e. a follow-up is
  // labelled late on the very day it is due. Overdue means the day has passed.
  return nextActionAt < startOfToday() ? 'overdue' : 'upcoming';
}

/** Local midnight of the current day, in epoch ms. */
function startOfToday(): number {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** Format staleness in days, e.g. '14d'. */
export function staleDays(updatedAt: number): number {
  return Math.floor((Date.now() - updatedAt) / (24 * 60 * 60 * 1000));
}
