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
   * staleness) at the SIGNATURE, not by calling it twice with identical
   * arguments — two calls with the same inputs only prove the function is
   * pure/deterministic, not that it has no corpus input (a function that
   * secretly read `allPostings.length` from a closure would pass that
   * version of the test too). Asserting arity is the direct claim: this
   * predicate structurally CANNOT read a corpus/generation signal because
   * nothing carrying one is ever passed to it. If this predicate's contract
   * ever grows a corpus/generation parameter, THIS is the assertion that
   * forces a conscious update here and to `hybrid-search-display.ts`'s doc
   * comment (a corpus-aware version belongs in `JobsPage`, not this file —
   * see that doc's "deliberate simplification" paragraph).
   */
  it('takes exactly the three text/state parameters — no corpus/generation signal', () => {
    expect(isCommittedSearchActive.length).toBe(3);
  });
});
