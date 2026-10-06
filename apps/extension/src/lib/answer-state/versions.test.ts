/**
 * Version history, Accept eligibility, the character counter and the
 * unchanged-rewrite check of the answer state (`answer-state.ts`): the counter
 * counts the text on screen, and a row with nowhere to write cannot be accepted.
 */

import { describe, expect, it } from 'vitest';

import {
  type AnswerRow,
  appendVersion,
  canAccept,
  counterText,
  isOverLimit,
  isUnchangedRewrite,
  normalizeAnswerText,
  rewriteBaseText,
  selectedText,
} from '../answer-state';
import { fieldOf, rowOf } from './test-support';

describe('versions', () => {
  const row = (over: Partial<AnswerRow> = {}): AnswerRow =>
    rowOf({
      field: fieldOf({ kind: 'filled', currentText: 'On the page.', originalText: 'On the page.' }),
      status: 'filled',
      ...over,
    });

  it('labels versions in order and selects the new one', () => {
    const one = appendVersion([row()], 'r', 'first', 'draft');
    const two = appendVersion(one, 'r', 'second', 'rewrite');

    expect(two[0]?.versions.map((v) => v.label)).toEqual(['v1', 'v2']);
    expect(two[0]?.selected).toBe(1);
  });

  it('clears a previous error when a version lands', () => {
    const errored = [row({ error: 'AI drafting is off.' })];
    expect(appendVersion(errored, 'r', 'ok now', 'draft')[0]?.error).toBeUndefined();
  });

  it('clears a previous notice when a version lands', () => {
    const noticed = [row({ notice: 'That came back the same.' })];
    expect(
      appendVersion(noticed, 'r', 'a genuinely new draft', 'draft')[0]?.notice
    ).toBeUndefined();
  });

  it('reshapes the LATEST version even while an older one is on screen', () => {
    // Restore is how you go back; a chip is how you go forward. Reshaping the
    // SELECTED version would silently discard v2 the moment someone looked at
    // v1.
    const two = appendVersion(
      appendVersion([row()], 'r', 'v1 text', 'draft'),
      'r',
      'v2 text',
      'rewrite'
    );
    const viewingV1 = two.map((r) => ({ ...r, selected: 0 }));

    expect(selectedText(viewingV1[0]!)).toBe('v1 text');
    expect(rewriteBaseText(viewingV1[0]!)).toBe('v2 text');
  });

  it('falls back to the page text before anything has been drafted', () => {
    expect(selectedText(row())).toBe('On the page.');
    expect(rewriteBaseText(row())).toBe('On the page.');
  });
});

describe('canAccept', () => {
  const drafted = (field: AnswerRow['field']): AnswerRow =>
    rowOf({
      field,
      status: 'drafted',
      versions: [{ label: 'v1', text: 'A draft.', kind: 'draft' }],
      selected: 0,
    });

  it('refuses a draft for a question that is not on the page', () => {
    expect(canAccept(drafted(null), false)).toBe(false);
  });

  it('refuses after a navigation, even with a field reference in hand', () => {
    expect(canAccept(drafted(fieldOf()), false)).toBe(true);
    expect(canAccept(drafted(fieldOf()), true)).toBe(false);
  });

  it('refuses to write whitespace over an existing answer', () => {
    const row = drafted(
      fieldOf({ kind: 'filled', currentText: 'Real answer.', originalText: 'Real answer.' })
    );
    expect(
      canAccept({ ...row, versions: [{ label: 'v1', text: '   ', kind: 'draft' }] }, false)
    ).toBe(false);
  });
});

describe('the character counter', () => {
  const withLimit = (maxChars?: number): AnswerRow =>
    rowOf({ field: fieldOf(maxChars === undefined ? {} : { maxChars }) });

  it('counts the text it is given, not the version metadata', () => {
    expect(counterText(withLimit(300), 'abcd')).toBe('4 / 300 characters');
  });

  it('shows nothing at all when the field declares no limit', () => {
    expect(counterText(withLimit(), 'abcd')).toBeNull();
    expect(isOverLimit(withLimit(), 'x'.repeat(10_000))).toBe(false);
  });

  it('is over only when it is actually over, not at the limit', () => {
    expect(isOverLimit(withLimit(4), 'abcd')).toBe(false);
    expect(isOverLimit(withLimit(4), 'abcde')).toBe(true);
  });
});

describe('isUnchangedRewrite / normalizeAnswerText', () => {
  const BASE = 'Led the migration and shipped the new payment service with the team.';

  it.each([
    ['is true for a verbatim echo', BASE, [BASE], true],
    [
      'is true when only a trailing comma or whitespace run differs',
      BASE,
      [`${BASE},`, '\n  ' + BASE.replace(/ /gu, '  ') + '\n'],
      true,
    ],
    // `+` is Unicode category Sm (a symbol, not punctuation) — both this
    // helper and the desktop twin (`normalizeRewriteText`, apps/desktop) strip
    // only trailing `\p{P}`, so the symbol survives the comparison here too.
    [
      'is NOT unchanged when a trailing SYMBOL changes the meaning (e.g. "20+" vs "20")',
      'Grew the team to 20+',
      ['Grew the team to 20'],
      false,
    ],
    [
      'is false for a genuinely different result',
      BASE,
      ['Led the migration; shipped payments.'],
      false,
    ],
    ['is false for an empty previous version — nothing to compare against', '   ', [''], false],
  ])('%s', (_name, previous, nexts, expected) => {
    for (const next of nexts) expect(isUnchangedRewrite(previous, next)).toBe(expected);
  });

  it('normalizeAnswerText collapses whitespace and strips only trailing punctuation', () => {
    expect(normalizeAnswerText('  a   b !!  ')).toBe('a b');
    // `+` is a symbol, not punctuation — it survives the strip.
    expect(normalizeAnswerText('a 20+')).toBe('a 20+');
  });
});
