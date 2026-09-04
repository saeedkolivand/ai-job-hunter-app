/**
 * Pure rewrite helpers — the three guards the inline rewrite popover leans on.
 *
 * Every case here comes from a measured live run (2026-09-03), not from
 * imagination: the model returned the selection back with one added comma, it
 * overshot "under 200 characters" about half the time and missed "at most 40
 * words" every time, and a Dutch span under an English document meta came back
 * in English in 12 of 18 runs.
 */
import { describe, expect, it } from 'vitest';

import {
  buildOvershootInstruction,
  deriveRewriteLocale,
  exceedsRewriteLimit,
  isUnchangedRewrite,
  measureRewriteLength,
  normalizeRewriteText,
  parseRewriteLimit,
} from './rewrite';

// ─── Unchanged-result predicate (C2) ─────────────────────────────────────────

describe('isUnchangedRewrite', () => {
  const SELECTION = 'Led the migration and shipped the new payment service with the team.';

  it('is true for a verbatim echo of the selection', () => {
    expect(isUnchangedRewrite(SELECTION, SELECTION)).toBe(true);
  });

  it('is true when the model differs only by ONE added trailing comma', () => {
    // The measured no-op: three "Shorten" runs came back differing from the
    // input by a single punctuation mark, so a bare `===` after trim misses it.
    expect(isUnchangedRewrite(SELECTION, `${SELECTION},`)).toBe(true);
  });

  it('is true when only whitespace RUNS differ', () => {
    const spaced = SELECTION.replace(/ /gu, '  ');
    expect(isUnchangedRewrite(SELECTION, `\n  ${spaced}\n`)).toBe(true);
  });

  it('is false for a genuinely shortened result', () => {
    expect(isUnchangedRewrite(SELECTION, 'Led the migration; shipped payments.')).toBe(false);
  });

  it('is false when the case changed — "make this all caps" is a real rewrite', () => {
    expect(isUnchangedRewrite(SELECTION, SELECTION.toUpperCase())).toBe(false);
  });

  it('is false for an empty selection (nothing to compare against)', () => {
    expect(isUnchangedRewrite('   ', '')).toBe(false);
  });

  it('normalizes whitespace runs and trailing punctuation only', () => {
    expect(normalizeRewriteText('  a   b !!  ')).toBe('a b');
    // Leading/interior punctuation survives — it can carry meaning.
    expect(normalizeRewriteText('"a, b"')).toBe('"a, b');
  });

  it('is false when the result drops a trailing SYMBOL that carries meaning', () => {
    // "+" is a symbol (\p{S}), not punctuation (\p{P}) — dropping it changes
    // "at least $2M" to exactly $2M, so it must NOT be treated as unchanged.
    expect(isUnchangedRewrite('Grew ARR to $2M+', 'Grew ARR to $2M')).toBe(false);
    expect(normalizeRewriteText('Grew ARR to $2M+')).toBe('Grew ARR to $2M+');
  });
});

// ─── Numeric limit parsing (C4) ──────────────────────────────────────────────

describe('parseRewriteLimit', () => {
  it('parses a CHARACTER limit from free text', () => {
    expect(parseRewriteLimit('Make it under 200 characters')).toEqual({ unit: 'chars', max: 200 });
    expect(parseRewriteLimit('200 chars max')).toEqual({ unit: 'chars', max: 200 });
    expect(parseRewriteLimit('max 150 character')).toEqual({ unit: 'chars', max: 150 });
  });

  it('parses a WORD limit from free text', () => {
    expect(parseRewriteLimit('At most 40 words')).toEqual({ unit: 'words', max: 40 });
  });

  it('returns null when the instruction carries no numeric limit', () => {
    // The shipped presets: none of them names a number.
    expect(
      parseRewriteLimit('Cut this to about two thirds of its length, keeping every concrete fact.')
    ).toBeNull();
    expect(
      parseRewriteLimit('Rephrase this in different words while keeping the same meaning.')
    ).toBeNull();
  });

  it('with TWO numbers, characters win over words and the smallest number wins', () => {
    expect(parseRewriteLimit('Under 200 characters and at most 40 words')).toEqual({
      unit: 'chars',
      max: 200,
    });
    expect(parseRewriteLimit('Under 200 characters, ideally 150 characters')).toEqual({
      unit: 'chars',
      max: 150,
    });
  });

  it('ignores a number that is not a length limit', () => {
    expect(parseRewriteLimit('Rewrite this for a 200 person company')).toBeNull();
    expect(parseRewriteLimit('Mention the 2019 migration')).toBeNull();
  });

  it('ignores a MINIMUM, which is a floor and not a limit', () => {
    expect(parseRewriteLimit('At least 200 characters')).toBeNull();
    expect(parseRewriteLimit('No fewer than 40 words')).toBeNull();
  });

  it('parses the German unit words the German presets and free text use', () => {
    expect(parseRewriteLimit('Höchstens 200 Zeichen')).toEqual({ unit: 'chars', max: 200 });
    expect(parseRewriteLimit('Maximal 40 Wörter')).toEqual({ unit: 'words', max: 40 });
    expect(parseRewriteLimit('Mindestens 200 Zeichen')).toBeNull();
  });
});

describe('measureRewriteLength / exceedsRewriteLimit', () => {
  it('counts characters of the trimmed text', () => {
    expect(measureRewriteLength('  abcde  ', 'chars')).toBe(5);
  });

  it('counts words across any whitespace run, and an empty string is zero words', () => {
    expect(measureRewriteLength('one two\nthree   four', 'words')).toBe(4);
    expect(measureRewriteLength('   ', 'words')).toBe(0);
  });

  it('treats the limit as INCLUSIVE — exactly at the limit is inside it', () => {
    expect(exceedsRewriteLimit('abcde', { unit: 'chars', max: 5 })).toBe(false);
    expect(exceedsRewriteLimit('abcdef', { unit: 'chars', max: 5 })).toBe(true);
  });
});

describe('buildOvershootInstruction', () => {
  it('keeps the original instruction and appends the MEASURED overshoot', () => {
    const out = buildOvershootInstruction(
      'Make it under 200 characters',
      { unit: 'chars', max: 200 },
      211
    );
    expect(out).toContain('Make it under 200 characters');
    expect(out).toContain('211 characters');
    expect(out).toContain('the limit is 200 characters');
    // 211 - 200 = 11: the model is told the number, never asked to try harder.
    expect(out).toContain('cut at least 11 characters');
    expect(out).toContain('keep every fact');
  });

  it('names the WORD unit for a word limit', () => {
    const out = buildOvershootInstruction('At most 40 words', { unit: 'words', max: 40 }, 46);
    expect(out).toContain('46 words');
    expect(out).toContain('cut at least 6 words');
  });
});

// ─── Span language (C5) ──────────────────────────────────────────────────────

describe('deriveRewriteLocale', () => {
  const DUTCH_SPAN =
    'Ik heb het migratieproject geleid en samen met het team de nieuwe betaaldienst opgeleverd binnen zes maanden.';

  it('a Dutch span under an English document meta yields a DUTCH locale', () => {
    expect(deriveRewriteLocale(DUTCH_SPAN, 'en')).toBe('nl');
  });

  it('falls back to the document meta when the span is too short to detect', () => {
    expect(deriveRewriteLocale('Kort.', 'de')).toBe('de');
  });

  it('falls back to English when there is no document meta either', () => {
    expect(deriveRewriteLocale('Kort.')).toBe('en');
    expect(deriveRewriteLocale('Kort.', '')).toBe('en');
  });

  it('keeps the span language when it agrees with the document', () => {
    expect(
      deriveRewriteLocale(
        'I led the migration project and delivered the new payment service with the team.',
        'en'
      )
    ).toBe('en');
  });
});
