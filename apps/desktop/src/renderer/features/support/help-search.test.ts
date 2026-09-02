/**
 * Help search — unit tests for `matchesHelpQuery`.
 *
 * Covers:
 *  1. word-AND: every token must be present, order-independent.
 *  2. a token absent from the text fails the whole query.
 *  3. case-insensitive on both the query and the text.
 *  4. an empty query matches everything (untouched search box).
 *  5. a whitespace-only query matches everything.
 *  6. repeated whitespace between tokens is ignored.
 *  7. matching is substring-based, not word-boundary-based.
 *  8. diacritic folding, in BOTH directions and on BOTH sides: an
 *     ASCII-keyboard query finds accented copy (`prufen` → `prüfen`,
 *     `resume` → `résumé`) and the accented spelling still finds itself.
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

  it('folds diacritics so an umlaut-free query matches German copy', () => {
    const DE = 'Wie prüfen Sie meinen Lebenslauf?';
    // The whole point: a keyboard with no umlaut key still reaches the entry.
    expect(matchesHelpQuery('prufen', DE)).toBe(true);
    // …and typing the accented spelling keeps working.
    expect(matchesHelpQuery('prüfen', DE)).toBe(true);
    // Folding must not turn every query into a match.
    expect(matchesHelpQuery('anschreiben', DE)).toBe(false);
  });

  it('folds diacritics in the text too, so `resume` finds `résumé`', () => {
    const ACCENTED = 'Wie exportiere ich mein Résumé?';
    expect(matchesHelpQuery('resume', ACCENTED)).toBe(true);
    expect(matchesHelpQuery('résumé', ACCENTED)).toBe(true);
    // Symmetric: the accented query also finds unaccented text.
    expect(matchesHelpQuery('résumé', 'Export your resume as a PDF')).toBe(true);
  });

  it('folds a decomposed spelling identically to a precomposed one', () => {
    // The same word in two Unicode encodings, derived at runtime rather than
    // typed: written as literals the two lines look identical, and an editor
    // (or any tool) normalizing the file would collapse them into the same
    // bytes and quietly void the assertion.
    const precomposed = 'prüfen'.normalize('NFC');
    const decomposed = 'prüfen'.normalize('NFD');
    expect(precomposed).not.toBe(decomposed);
    expect(matchesHelpQuery(precomposed, decomposed)).toBe(true);
    expect(matchesHelpQuery(decomposed, precomposed)).toBe(true);
  });
});
