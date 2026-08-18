import { describe, expect, it } from 'vitest';

import fixture from '../fixtures/section-lexicon.json';
import { SECTION_LEXICON } from './index';

describe('section-lexicon fixture ↔ SECTION_LEXICON', () => {
  // The fixture is a flattened copy of SECTION_LEXICON, read by Rust's
  // `documents::evidence::lexicon_parity` sweep — Rust cannot parse the TS
  // source, so the copy is unavoidable. This is the guard that keeps it from
  // becoming a stale copy: a term added to SECTION_LEXICON without
  // regenerating the fixture fails here, which is what stops the Rust sweep
  // from silently checking yesterday's vocabulary and reporting green.
  it('matches the live lexicon exactly, in order', () => {
    const derived = SECTION_LEXICON.flatMap((entry) =>
      entry.terms.map((term) => ({ section: entry.name, term }))
    );
    expect(fixture).toEqual(derived);
  });
});
