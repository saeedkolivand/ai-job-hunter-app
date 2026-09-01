import { createMachine } from '@/lib/machine';

/**
 * Hybrid postings search (renderer half — see `commands::hybrid_search`,
 * Rust, for the lexical/dense/fusion/rerank pipeline this reports on).
 *
 * States:
 *   idle       → no committed search; the instant substring filter governs the list
 *   searching  → `scrape:hybridSearch` in flight
 *   results    → outcome 'ok' with at least one hit
 *   noResults  → outcome 'ok' with zero hits — a DIFFERENT situation from
 *                "nothing scraped yet" and must read as one (see JobsResults)
 *   stale      → outcome 'staleCorpus' (a re-scrape cleared the corpus mid-search)
 *   error      → the mutation itself rejected (network/IPC failure)
 *
 * `outcome: 'cancelled'` never reaches this machine. By construction it only
 * happens to a search `usePostingsSearch` itself superseded (a newer search
 * cancels the previous one's `queryId` before firing), so it is silently
 * discarded there rather than driving a transition — that is what makes it
 * distinct from a genuine error.
 *
 * Valid transitions (happy path):
 *   idle → searching → results | noResults
 *
 * Recovery: SUBMIT is valid from every non-searching state (re-issuing the
 * same query after `stale`/`error` is just another search), and CLEAR always
 * returns to idle (the user dismisses the search, e.g. via "Clear search").
 */
export type PostingsSearchState =
  'idle' | 'searching' | 'results' | 'noResults' | 'stale' | 'error';

export type PostingsSearchEvent =
  'SUBMIT' | 'SETTLED_RESULTS' | 'SETTLED_EMPTY' | 'SETTLED_STALE' | 'FAILED' | 'CLEAR';

export const postingsSearchMachine = createMachine<PostingsSearchState, PostingsSearchEvent>({
  transitions: {
    idle: { SUBMIT: 'searching' },
    searching: {
      SETTLED_RESULTS: 'results',
      SETTLED_EMPTY: 'noResults',
      SETTLED_STALE: 'stale',
      FAILED: 'error',
      CLEAR: 'idle',
    },
    results: { SUBMIT: 'searching', CLEAR: 'idle' },
    noResults: { SUBMIT: 'searching', CLEAR: 'idle' },
    stale: { SUBMIT: 'searching', CLEAR: 'idle' },
    error: { SUBMIT: 'searching', CLEAR: 'idle' },
  },
  busyStates: ['searching'],
  errorStates: ['error'],
});
