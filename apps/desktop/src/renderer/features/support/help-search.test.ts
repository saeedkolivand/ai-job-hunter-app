/**
 * Help search — unit tests for `matchesHelpQuery`.
 *
 * Covers:
 *  1. word-AND: every token must be present, order-independent.
 *  2. a token absent from the text fails the whole query.
 *  3. case-insensitive on both the query and the text.
 *  4. an empty query matches everything (untouched search box).
 *  5. a whitespace-only query matches everything (no empty token survives).
 *  6. repeated whitespace between tokens is ignored.
 *  7. matching is substring-based, not word-boundary-based.
 *  8. non-ASCII text (the de locale) lowercases and matches too.
 */
import { describe, expect, it } from 'vitest';

import { matchesHelpQuery } from '@/features/support/help-search';

/** Stands in for a `${q} ${a}` help entry. */
const TEXT = 'How do I export a document? Use Export and pick PDF, DOCX or TXT.';

describe('matchesHelpQuery', () => {
  it('matches when every token is present, in any order', () => {
    expect(matchesHelpQuery('export pdf', TEXT)).toBe(true);
    expect(matchesHelpQuery('pdf export', TEXT)).toBe(true);
  });

  it('fails when a single token is absent', () => {
    // "export" hits, "xml" does not — word-AND means the whole query fails.
    expect(matchesHelpQuery('export xml', TEXT)).toBe(false);
    expect(matchesHelpQuery('autopilot', TEXT)).toBe(false);
  });

  it('is case-insensitive on both sides', () => {
    expect(matchesHelpQuery('EXPORT DocX', TEXT)).toBe(true);
    expect(matchesHelpQuery('export', TEXT.toUpperCase())).toBe(true);
  });

  it('matches everything for an empty query', () => {
    expect(matchesHelpQuery('', TEXT)).toBe(true);
    expect(matchesHelpQuery('', '')).toBe(true);
  });

  it('matches everything for a whitespace-only query', () => {
    expect(matchesHelpQuery('   ', TEXT)).toBe(true);
    expect(matchesHelpQuery('\t \n ', TEXT)).toBe(true);
  });

  it('ignores repeated whitespace between tokens', () => {
    expect(matchesHelpQuery('  export   pdf  ', TEXT)).toBe(true);
  });

  it('matches a substring inside a word (no word-boundary requirement)', () => {
    expect(matchesHelpQuery('port', TEXT)).toBe(true);
  });

  it('matches localized text', () => {
    expect(matchesHelpQuery('LEBENSLAUF', 'Wie exportiere ich meinen Lebenslauf?')).toBe(true);
    expect(matchesHelpQuery('anschreiben', 'Wie exportiere ich meinen Lebenslauf?')).toBe(false);
  });
});
