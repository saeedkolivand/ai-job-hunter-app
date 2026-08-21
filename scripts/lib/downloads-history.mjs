// The two pieces of real logic behind the downloads series, kept here rather
// than inside build-repo-charts.mjs so they can be tested: that script fetches
// the GitHub API at import time, so nothing in it is reachable from a unit test.

import { readFileSync } from 'node:fs';

/**
 * Read a JSON array, or throw.
 *
 * Deliberately unforgiving, because the downloads series is the one input that
 * cannot be recomputed: GitHub has no historical download endpoint, and the
 * publish target is a PARENTLESS commit that is force-pushed, so the branch
 * keeps no previous copy. A swallowed parse error would quietly reduce the
 * series to seed-plus-today and then overwrite the only surviving record of
 * every reading since the seed — unrecoverably.
 *
 * Failing the run instead leaves the last good file untouched on `badges`.
 * A genuinely absent `--prev-history` is handled by the caller, not here.
 */
export function readJsonArray(path, label) {
  const parsed = JSON.parse(readFileSync(path, 'utf8'));
  if (!Array.isArray(parsed)) {
    throw new TypeError(`${label} must be a JSON array (${path})`);
  }
  return parsed;
}

/**
 * Merge point lists by date, highest reading per date wins, sorted ascending.
 *
 * Highest-wins rather than last-wins because an installer download count can
 * only grow: if two sources disagree for one date, the larger reading is the
 * later — and therefore better — observation. It also makes the merge
 * order-independent, so re-running with the seed in a different position cannot
 * change the output.
 */
export function mergePoints(...lists) {
  const byDate = new Map();
  for (const list of lists) {
    for (const p of list ?? []) {
      if (!p?.date || typeof p.value !== 'number') continue;
      byDate.set(p.date, Math.max(byDate.get(p.date) ?? 0, p.value));
    }
  }
  return [...byDate.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([date, value]) => ({ date, value }));
}
