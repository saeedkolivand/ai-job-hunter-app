import { describe, expect, it } from 'vitest';

import { isCommittedSearchActive } from './hybrid-search-display';

describe('isCommittedSearchActive', () => {
  it('is active once a search has settled and the box still holds the exact committed text', () => {
    expect(isCommittedSearchActive('results', 'rust engineer', 'rust engineer')).toBe(true);
  });

  it('goes idle the instant the typed text diverges from the committed query', () => {
    expect(isCommittedSearchActive('results', 'rust engineer', 'rust engineer remote')).toBe(false);
  });

  it('is never active while idle, even if the (empty) committed query happens to match', () => {
    expect(isCommittedSearchActive('idle', '', '')).toBe(false);
  });

  it('is never active for an empty/whitespace-only filter, regardless of machine state', () => {
    expect(isCommittedSearchActive('results', '', '')).toBe(false);
    expect(isCommittedSearchActive('results', '  ', '   ')).toBe(false);
  });

  it('trims the current filter text before comparing', () => {
    expect(isCommittedSearchActive('noResults', 'engineer', '  engineer  ')).toBe(true);
  });

  /**
   * Pins the accepted caching behavior (MEDIUM review finding, corpus
   * staleness): this predicate takes NO signal about the postings corpus at
   * all — by construction, retyping the exact committed query reactivates
   * whatever result is already cached, regardless of a scrape that added
   * (or removed) postings in between. If this predicate's signature ever
   * grows a corpus/generation parameter, this test should be the one that
   * forces a conscious update to `hybrid-search-display.ts`'s doc comment.
   */
  it('reactivates the SAME cached answer for the same text no matter how many times it is asked (no corpus signal)', () => {
    const askedOnceBeforeAScrape = isCommittedSearchActive('results', 'engineer', 'engineer');
    const askedAgainAfterAScrape = isCommittedSearchActive('results', 'engineer', 'engineer');
    expect(askedOnceBeforeAScrape).toBe(true);
    expect(askedAgainAfterAScrape).toBe(true);
    expect(askedOnceBeforeAScrape).toBe(askedAgainAfterAScrape);
  });
});
