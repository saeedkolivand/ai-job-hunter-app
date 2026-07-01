import type { Posting } from '../types';

/**
 * Merge live (streamed) and persisted postings into a single deduplicated list.
 *
 * Backend `postings` win on duplicate ids — they carry interactions and the
 * persisted full description. Streamed `livePostings` are only added when
 * they have no backend counterpart yet (mid-scrape, not yet persisted).
 */
export function mergePostings(postings: Posting[], livePostings: Posting[]): Posting[] {
  const byId = new Map<string, Posting>();
  for (const p of postings) byId.set(p.id, p);
  for (const p of livePostings) if (!byId.has(p.id)) byId.set(p.id, p);
  return [...byId.values()];
}
