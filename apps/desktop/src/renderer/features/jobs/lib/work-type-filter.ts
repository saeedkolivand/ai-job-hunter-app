import type { WorkTypeOption } from '@ajh/shared';

import type { Posting } from '../types';

/**
 * View-only work-type filter predicate (Jobs-page). `workType` is read
 * verbatim from what the backend already attached at scrape-time — this is
 * NOT a classifier, and must never become one (a TS mirror of the Rust
 * classifier would be exactly the drift risk `canonical-job-key.ts` exists to
 * defend against for cluster keys).
 *
 * A posting with no declared `workType` is undeclared, not a mismatch, and is
 * always kept — mirrors the backend's keep-unknowns policy so the on-screen
 * filter never contradicts what the scrape engine already decided.
 */
export function matchesWorkTypeFilter(
  posting: Pick<Posting, 'workType'>,
  wanted: readonly WorkTypeOption[]
): boolean {
  if (wanted.length === 0) return true;
  if (!posting.workType) return true;
  return wanted.includes(posting.workType);
}
