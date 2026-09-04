/**
 * The shared answer state's ROW MODEL (`answer-state.ts`).
 *
 * These pin the decisions that are easy to break silently, not the shape of
 * the interfaces: that a rescan does not throw away drafted work, that the two
 * candidate sets keep separate correlation namespaces, that a row with nowhere
 * to write cannot be accepted, and that the counter counts the text on screen
 * rather than anything the model claimed about it.
 */

import { describe, expect, it } from 'vitest';

import {
  addFreeRow,
  type AnswerRow,
  type AnswerScan,
  appendVersion,
  buildRows,
  canAccept,
  counterText,
  isOverLimit,
  isUnchangedRewrite,
  normalizeAnswerText,
  rewriteBaseText,
  selectedText,
} from './answer-state';

const NO_SAVED = new Map<string, { answer: string; source?: string }>();

const scan = (over: Partial<AnswerScan> = {}): AnswerScan => ({
  questions: [],
  filled: [],
  ...over,
});

describe('buildRows', () => {
  it('gives the two candidate sets separate rows even for the identical question text', () => {
    // `collectQuestions` and `collectFilledFields` index INDEPENDENTLY, so
    // occurrence 0 of "Why us?" means a different field in each. Collapsing
    // them onto one row would make an Accept correlate through the wrong
    // locator and write into a field the scan never saw.
    const rows = buildRows(
      scan({
        questions: [{ question: 'Why us?', index: 0 }],
        filled: [{ question: 'Why us?', index: 0, answer: 'Because.' }],
      }),
      NO_SAVED
    );

    expect(rows).toHaveLength(2);
    expect(rows.map((r) => r.field?.kind)).toEqual(['empty', 'filled']);
    expect(new Set(rows.map((r) => r.id)).size).toBe(2);
  });

  it('records the same-question count so a later Accept can fail safe', () => {
    const rows = buildRows(
      scan({
        questions: [
          { question: 'Notice period', index: 0 },
          { question: 'Notice period', index: 1 },
        ],
      }),
      NO_SAVED
    );

    expect(rows.map((r) => r.field?.count)).toEqual([2, 2]);
  });

  it('carries drafted versions through a rescan', () => {
    const first = buildRows(scan({ questions: [{ question: 'Why us?', index: 0 }] }), NO_SAVED);
    const drafted = appendVersion(first, first[0]!.id, 'A draft.', 'draft');

    // The field flipped from empty to filled — a rescan sees it in the OTHER
    // candidate set, so the empty row disappears and must survive as free text.
    const rescanned = buildRows(
      scan({ filled: [{ question: 'Why us?', index: 0, answer: 'A draft.' }] }),
      NO_SAVED,
      drafted
    );

    const carried = rescanned.find((r) => r.versions.length > 0);
    expect(carried?.versions[0]?.text).toBe('A draft.');
  });

  it('drops a vanished scanned row that carried no work, and keeps one that did', () => {
    const previous: AnswerRow[] = [
      {
        id: 'empty:0:Gone',
        question: 'Gone',
        field: null,
        status: 'empty',
        versions: [],
        selected: -1,
      },
      {
        id: 'empty:0:Kept',
        question: 'Kept',
        field: { kind: 'empty', index: 0, count: 1, currentText: '', originalText: '' },
        status: 'drafted',
        versions: [{ label: 'v1', text: 'Worth keeping.', kind: 'draft' }],
        selected: 0,
      },
    ];

    const rows = buildRows(scan(), NO_SAVED, previous);

    // Mutation guard: relax the survives-a-rescan rule to "keep everything"
    // and THIS line fails — an empty scanned row would come back as a question
    // the page no longer has.
    expect(rows.map((r) => r.question)).toEqual(['Kept']);
    // The survivor is kept as a free-text row: there is nothing on the page to
    // accept into any more, and pretending otherwise is what decision 4 forbids.
    expect(rows[0]?.field).toBeNull();
  });

  it('marks a question a past application can answer, with its source', () => {
    const rows = buildRows(
      scan({ questions: [{ question: 'Why us?', index: 0 }] }),
      new Map([['Why us?', { answer: 'Because.', source: 'Frontend Dev at Acme' }]])
    );

    expect(rows[0]?.status).toBe('saved-available');
    expect(rows[0]?.savedSource).toBe('Frontend Dev at Acme');
  });

  it('reads the field limit off the scan and leaves it absent when the page declares none', () => {
    const rows = buildRows(
      scan({
        questions: [
          { question: 'Capped', index: 0, maxChars: 300 },
          { question: 'Uncapped', index: 0 },
        ],
      }),
      NO_SAVED
    );

    expect(rows[0]?.field?.maxChars).toBe(300);
    expect(rows[1]?.field?.maxChars).toBeUndefined();
  });

  it('carries a row notice through a rescan, same as it does an error', () => {
    const first = buildRows(scan({ questions: [{ question: 'Why us?', index: 0 }] }), NO_SAVED);
    const noticed = first.map((r) => ({ ...r, notice: 'That came back the same.' }));

    const rescanned = buildRows(
      scan({ questions: [{ question: 'Why us?', index: 0 }] }),
      NO_SAVED,
      noticed
    );

    expect(rescanned[0]?.notice).toBe('That came back the same.');
  });
});

describe('addFreeRow', () => {
  it('is idempotent, so a repeated context-menu click does not stack duplicates', () => {
    const once = addFreeRow([], '  Why us?  ');
    const twice = addFreeRow(once, 'Why us?');

    expect(once).toHaveLength(1);
    expect(twice).toHaveLength(1);
    expect(twice[0]?.question).toBe('Why us?');
  });

  it('ignores an empty selection', () => {
    expect(addFreeRow([], '   ')).toEqual([]);
  });

  it('survives a rescan that finds nothing, even before it has been drafted', () => {
    // A question the user typed is theirs. A rescan of a page that never had a
    // field for it must not quietly delete it.
    const typed = addFreeRow([], 'A question the scan missed');
    expect(buildRows(scan(), NO_SAVED, typed).map((r) => r.question)).toEqual([
      'A question the scan missed',
    ]);
  });
});

describe('versions', () => {
  const row = (over: Partial<AnswerRow> = {}): AnswerRow => ({
    id: 'r',
    question: 'Q',
    field: {
      kind: 'filled',
      index: 0,
      count: 1,
      currentText: 'On the page.',
      originalText: 'On the page.',
    },
    status: 'filled',
    versions: [],
    selected: -1,
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
  const drafted = (field: AnswerRow['field']): AnswerRow => ({
    id: 'r',
    question: 'Q',
    field,
    status: 'drafted',
    versions: [{ label: 'v1', text: 'A draft.', kind: 'draft' }],
    selected: 0,
  });

  it('refuses a draft for a question that is not on the page', () => {
    expect(canAccept(drafted(null), false)).toBe(false);
  });

  it('refuses after a navigation, even with a field reference in hand', () => {
    const field = { kind: 'empty' as const, index: 0, count: 1, currentText: '', originalText: '' };
    expect(canAccept(drafted(field), false)).toBe(true);
    expect(canAccept(drafted(field), true)).toBe(false);
  });

  it('refuses to write whitespace over an existing answer', () => {
    const row = drafted({
      kind: 'filled',
      index: 0,
      count: 1,
      currentText: 'Real answer.',
      originalText: 'Real answer.',
    });
    expect(
      canAccept({ ...row, versions: [{ label: 'v1', text: '   ', kind: 'draft' }] }, false)
    ).toBe(false);
  });
});

describe('the character counter', () => {
  const withLimit = (maxChars?: number): AnswerRow => ({
    id: 'r',
    question: 'Q',
    field: {
      kind: 'empty',
      index: 0,
      count: 1,
      currentText: '',
      originalText: '',
      ...(maxChars === undefined ? {} : { maxChars }),
    },
    status: 'empty',
    versions: [],
    selected: -1,
  });

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

  it('is true for a verbatim echo', () => {
    expect(isUnchangedRewrite(BASE, BASE)).toBe(true);
  });

  it('is true when only a trailing comma or whitespace run differs', () => {
    expect(isUnchangedRewrite(BASE, `${BASE},`)).toBe(true);
    expect(isUnchangedRewrite(BASE, `\n  ${BASE.replace(/ /gu, '  ')}\n`)).toBe(true);
  });

  it('is NOT unchanged when a trailing SYMBOL changes the meaning (e.g. "20+" vs "20")', () => {
    // `+` is Unicode category Sm (a symbol, not punctuation) — the desktop
    // twin's `\p{P}\p{S}` strip drops it and would call these unchanged; this
    // helper strips only `\p{P}`, so the symbol survives the comparison.
    expect(isUnchangedRewrite('Grew the team to 20+', 'Grew the team to 20')).toBe(false);
  });

  it('is false for a genuinely different result', () => {
    expect(isUnchangedRewrite(BASE, 'Led the migration; shipped payments.')).toBe(false);
  });

  it('is false for an empty previous version — nothing to compare against', () => {
    expect(isUnchangedRewrite('   ', '')).toBe(false);
  });

  it('normalizeAnswerText collapses whitespace and strips only trailing punctuation', () => {
    expect(normalizeAnswerText('  a   b !!  ')).toBe('a b');
    // `+` is a symbol, not punctuation — it survives the strip.
    expect(normalizeAnswerText('a 20+')).toBe('a 20+');
  });
});
