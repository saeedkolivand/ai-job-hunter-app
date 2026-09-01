import type { PostingsSearchState } from '@/features/jobs/hooks/usePostingsSearch';

/**
 * Whether a committed hybrid search should currently govern the visible
 * list — gated on the CURRENTLY-TYPED filter text still matching the text
 * the search was committed with (`JobsPage`). Editing the box after a
 * search settles falls back to instant substring filtering; retyping the
 * exact committed query reactivates it, purely derived, no effect needed.
 *
 * Deliberately does NOT invalidate on anything else — in particular, on
 * whether the postings corpus (`allPostings`) has changed since the search
 * was committed. A fresh scrape (or a "Show more" append) that adds new
 * postings while a matching search is displayed does NOT force a re-search:
 * retyping the exact committed query re-shows the CACHED ranked result
 * (`usePostingsSearch`'s `result`, re-intersected against the CURRENT
 * eligible set every render — see `JobsPage`'s `rankedFiltered` — so a hit
 * that's since become ineligible still drops out), not a fresh one. A
 * newly-scraped posting cannot appear in that cached result until the user
 * explicitly re-submits (Enter / the Search button).
 *
 * This is a deliberate simplification, not an oversight: driving staleness
 * off "has the corpus changed since commit" would need a generation/version
 * marker on `allPostings`, and doing that safely — without silently
 * discarding a user's ranked view mid-read every time a background scrape
 * stream trickles in one more item — is a real UX design decision, not a
 * mechanical fix. Left for a follow-up if it proves to matter in practice;
 * until then this function's contract (text-match only) is the documented,
 * tested behavior.
 */
export function isCommittedSearchActive(
  machineState: PostingsSearchState,
  committedQuery: string,
  currentFilterText: string
): boolean {
  const trimmed = currentFilterText.trim();
  return machineState !== 'idle' && committedQuery === trimmed && trimmed.length > 0;
}
