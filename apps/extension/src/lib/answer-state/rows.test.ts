/**
 * The row model's `buildRows` / `addFreeRow` (`answer-state.ts`): what a rescan
 * keeps, migrates and drops. Pins the decisions that are easy to break silently
 * — that a rescan does not throw away drafted work and that the two candidate
 * sets keep separate correlation namespaces.
 */

import { describe, expect, it } from 'vitest';

import { addFreeRow, type AnswerRow, appendVersion, buildRows, canAccept } from '../answer-state';
import { draftedWhyUs, rowOf, rowsOf, WHY_US, WHY_US_FILLED } from './test-support';

const orphans = (rows: AnswerRow[]): AnswerRow[] => rows.filter((r) => r.field === null);

describe('buildRows', () => {
  it('gives the two candidate sets separate rows even for the identical question text', () => {
    // `collectQuestions` and `collectFilledFields` index INDEPENDENTLY, so
    // occurrence 0 of "Why us?" means a different field in each. Collapsing
    // them onto one row would make an Accept correlate through the wrong
    // locator and write into a field the scan never saw.
    const rows = rowsOf({
      questions: [WHY_US],
      filled: [{ ...WHY_US, answer: 'Because.' }],
    });

    expect(rows).toHaveLength(2);
    expect(rows.map((r) => r.field?.kind)).toEqual(['empty', 'filled']);
    expect(new Set(rows.map((r) => r.id)).size).toBe(2);
  });

  it('records the same-question count so a later Accept can fail safe', () => {
    const rows = rowsOf({
      questions: [
        { question: 'Notice period', index: 0 },
        { question: 'Notice period', index: 1 },
      ],
    });

    expect(rows.map((r) => r.field?.count)).toEqual([2, 2]);
  });

  it('migrates drafted versions onto the filled row a kind-flipped field rescans as', () => {
    // The field flipped from empty to filled — a rescan sees it in the OTHER
    // candidate set under a DIFFERENT row id (`empty:0:Why us?` ->
    // `filled:0:Why us?`). The (question,index) migration must move the
    // drafted versions onto the row that NOW owns the field, as a REAL field
    // row the user can still accept into — not strand them as free text.
    const rescanned = rowsOf({ filled: [WHY_US_FILLED] }, draftedWhyUs());

    expect(rescanned).toHaveLength(1);
    const migrated = rescanned[0]!;
    expect(migrated.id).toBe('filled:0:Why us?');
    expect(migrated.field?.kind).toBe('filled');
    expect(migrated.versions[0]?.text).toBe('A draft.');
    expect(canAccept(migrated, false)).toBe(true);
    // The migrated row must not ALSO re-emerge as a free-text orphan with the
    // same versions (that is the `claimed` set's job).
    expect(orphans(rescanned)).toHaveLength(0);
  });

  it('lets an exact-id carry-over win over migration onto a second row', () => {
    // One scan can yield BOTH `empty:0:Why us?` AND `filled:0:Why us?` (two
    // fields, identical question text — see the first test above). The prior
    // `empty` row is still reachable by its OWN id, so it must be carried
    // exactly once, onto the empty row, and the kind-flip migration must not
    // ALSO adopt it onto the filled row (duplicated versions on two rows).
    const rescanned = rowsOf({ questions: [WHY_US], filled: [WHY_US_FILLED] }, draftedWhyUs());

    const withVersions = rescanned.filter((r) => r.versions.length > 0);
    expect(withVersions).toHaveLength(1);
    expect(withVersions[0]?.id).toBe('empty:0:Why us?');
  });

  it('does not migrate when the prior row matches but holds no drafts', () => {
    // A versionless prior row has nothing to salvage — the field flips and the
    // scan finds it under a new id as a plain empty-of-versions row.
    const previous = [rowOf({ id: 'empty:0:Why us?', question: 'Why us?' })];

    const rescanned = rowsOf({ filled: [{ ...WHY_US, answer: 'Typed on the page.' }] }, previous);

    expect(rescanned).toHaveLength(1);
    expect(rescanned[0]?.field?.kind).toBe('filled');
    expect(rescanned[0]?.versions).toEqual([]);
    // And the versionless prior row is dropped, not kept as a free-text row.
    expect(orphans(rescanned)).toHaveLength(0);
  });

  it('never migrates a free-text row, which stays free text on its own', () => {
    const previous = [
      rowOf({
        id: 'free: Tell me about yourself',
        question: 'Tell me about yourself',
        field: null,
        status: 'drafted',
        versions: [{ label: 'v1', text: 'A manual draft.', kind: 'draft' }],
        selected: 0,
      }),
    ];

    const rescanned = rowsOf(
      { questions: [{ question: 'Tell me about yourself', index: 0 }] },
      previous
    );

    // The scanned row is fresh (no adoption — the prior is free text), and the
    // free-text row keeps its own versions as itself.
    expect(rescanned[0]?.versions).toEqual([]);
    expect(rescanned[1]?.id).toBe('free: Tell me about yourself');
    expect(rescanned[1]?.versions[0]?.text).toBe('A manual draft.');
  });

  it('adopts each flipped field once and free-texts nothing (`claimed` set)', () => {
    const first = rowsOf({ questions: [WHY_US, { question: 'Notice period', index: 0 }] });
    const drafted = appendVersion(
      appendVersion(first, first[0]!.id, 'Why us answer.', 'draft'),
      first[1]!.id,
      'Two weeks.',
      'draft'
    );

    const rescanned = rowsOf(
      {
        filled: [
          { ...WHY_US, answer: 'Why us answer.' },
          { question: 'Notice period', index: 0, answer: 'Two weeks.' },
        ],
      },
      drafted
    );

    expect(rescanned).toHaveLength(2);
    const migrated = rescanned.filter((r) => r.versions.length > 0);
    expect(migrated).toHaveLength(2);
    // Neither migrated row re-emerges as an orphan free-text duplicate.
    expect(orphans(rescanned)).toHaveLength(0);
  });

  it('drops a vanished scanned row that carried no work, and keeps one that did', () => {
    const previous = [
      rowOf({ id: 'empty:0:Gone', question: 'Gone', field: null }),
      rowOf({
        id: 'empty:0:Kept',
        question: 'Kept',
        status: 'drafted',
        versions: [{ label: 'v1', text: 'Worth keeping.', kind: 'draft' }],
        selected: 0,
      }),
    ];

    const rows = rowsOf({}, previous);

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
      { questions: [WHY_US], filled: [] },
      new Map([['Why us?', { answer: 'Because.', source: 'Frontend Dev at Acme' }]])
    );

    expect(rows[0]?.status).toBe('saved-available');
    expect(rows[0]?.savedSource).toBe('Frontend Dev at Acme');
  });

  it('reads the field limit off the scan and leaves it absent when the page declares none', () => {
    const rows = rowsOf({
      questions: [
        { question: 'Capped', index: 0, maxChars: 300 },
        { question: 'Uncapped', index: 0 },
      ],
    });

    expect(rows[0]?.field?.maxChars).toBe(300);
    expect(rows[1]?.field?.maxChars).toBeUndefined();
  });

  it('carries a row notice through a rescan, same as it does an error', () => {
    const first = rowsOf({ questions: [WHY_US] });
    const noticed = first.map((r) => ({ ...r, notice: 'That came back the same.' }));

    const rescanned = rowsOf({ questions: [WHY_US] }, noticed);

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
    expect(rowsOf({}, typed).map((r) => r.question)).toEqual(['A question the scan missed']);
  });
});
