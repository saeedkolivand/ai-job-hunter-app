import type { DedupMarkNotDuplicateRequest } from '../../schemas/index.js';

/**
 * Cross-board dedup (ADR-029): the renderer's only write into the clustering
 * feature. Clustering itself is recomputed in Rust at every ingest and is never
 * driven from here — the UI groups rows by the opaque `clusterId`/member keys
 * the backend attaches and, on a user "not a duplicate" action, echoes those
 * keys straight back through {@link DedupContract.markNotDuplicate}.
 */
export interface DedupContract {
  /**
   * Record a "not a duplicate" verdict: `memberKey` is split from each of
   * `otherKeys` (opaque canonical job keys taken from a cluster's members). The
   * pair tombstones persist, so the split survives every re-scrape. Pass
   * `autopilotId` when splitting within an autopilot found-jobs view so that
   * record's annotations are recomputed too.
   */
  markNotDuplicate(req: DedupMarkNotDuplicateRequest): Promise<{ success: boolean }>;
}

export const DEDUP_CHANNELS = {
  markNotDuplicate: 'dedup:markNotDuplicate',
} as const;
