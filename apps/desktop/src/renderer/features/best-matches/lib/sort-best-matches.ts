import type { AutopilotBestMatch } from '@ajh/shared';

export const BEST_MATCHES_SORTS = ['score', 'newest', 'salary'] as const;
export type BestMatchesSortBy = (typeof BEST_MATCHES_SORTS)[number];

const byUrl = (x: AutopilotBestMatch, y: AutopilotBestMatch) =>
  x.url < y.url ? -1 : x.url > y.url ? 1 : 0;

/**
 * `postedAt` desc, dated band first, undated band trailing, `url` tiebreak.
 * Mirrors `sortFoundJobsByDate`'s exact contract
 * (`AutopilotCard/index.tsx:196-211`): deliberately NO `foundAt` (discovery
 * time) fallback for undated rows — a just-scraped stale posting must not
 * jump above a genuinely recent one just because we found it more recently.
 * Never mutates `matches`.
 */
export function sortByNewest(matches: AutopilotBestMatch[]): AutopilotBestMatch[] {
  return [...matches].sort((a, b) => {
    if (typeof a.postedAt !== 'number' || typeof b.postedAt !== 'number') {
      if (typeof a.postedAt === 'number') return -1; // a dated, b undated
      if (typeof b.postedAt === 'number') return 1; // b dated, a undated
      return byUrl(a, b); // both undated
    }
    return b.postedAt - a.postedAt || byUrl(a, b);
  });
}

/**
 * `salaryMax ?? salaryMin` desc, same dated/undated-style banding as
 * `sortByNewest` for rows that carry no salary at all (most boards don't
 * report one — Adzuna is the only one that does), `url` tiebreak. Never
 * mutates `matches`.
 */
export function sortBySalary(matches: AutopilotBestMatch[]): AutopilotBestMatch[] {
  const salaryOf = (m: AutopilotBestMatch) => m.salaryMax ?? m.salaryMin;
  return [...matches].sort((a, b) => {
    const sa = salaryOf(a);
    const sb = salaryOf(b);
    if (typeof sa !== 'number' || typeof sb !== 'number') {
      if (typeof sa === 'number') return -1;
      if (typeof sb === 'number') return 1;
      return byUrl(a, b);
    }
    return sb - sa || byUrl(a, b);
  });
}

/**
 * `'score'` (default) returns `matches` UNTOUCHED — the backend already
 * sorts by tier desc, score desc, key asc, and re-sorting it here would just
 * be a slower no-op with a chance to get the tiebreak wrong. Never mutates
 * its input in any branch.
 */
export function sortBestMatches(
  matches: AutopilotBestMatch[],
  sortBy: BestMatchesSortBy
): AutopilotBestMatch[] {
  if (sortBy === 'newest') return sortByNewest(matches);
  if (sortBy === 'salary') return sortBySalary(matches);
  return matches;
}
