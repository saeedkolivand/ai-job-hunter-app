/**
 * Unit tests for the "rewrite mode" collector (`collectFilledFields`, extension
 * PR 11) and its rewrite-target re-scan (`locateFilledField`) in
 * apps/extension/src/lib/answers-capture.ts.
 */

import { afterEach, describe, expect, it } from 'vitest';

import { collectFilledFields, locateFilledField } from '../answers-capture';
import { field, resetDocument, setForm } from './test-support';

afterEach(resetDocument);

const filledOf = (html: string) => {
  setForm(html);
  return collectFilledFields(document);
};

const WHY = { question: 'Why this role?', index: 0, answer: 'Because I love it.' };
const YEARS_SELECT = `
  <label for="yrs">Years of experience</label>
  <select id="yrs"><option value="0">Choose one</option><option value="1" selected>5-10 years</option></select>`;

describe('collectFilledFields — scans FILLED candidate fields, the mirror of collectQuestions', () => {
  it.each([
    [
      'scans a filled, labelled text input at index 0 with its current answer',
      field('q1', 'Why this role?', 'Because I love it.'),
      [WHY],
    ],
    [
      'skips an EMPTY field — the mirror of collectQuestions skipping filled ones',
      field('q1', 'Why this role?'),
      [],
    ],
    [
      'assigns increasing occurrence indices to filled fields sharing the exact same label',
      `${field('q1', 'Comments', 'First')}<label for="q2">Comments</label><textarea id="q2">Second</textarea>`,
      [
        { question: 'Comments', index: 0, answer: 'First' },
        { question: 'Comments', index: 1, answer: 'Second' },
      ],
    ],
    [
      'applies the SAME visibility/denylist/identity gates as collectAnswers',
      `
      <div style="display:none">${field('hp', 'Trap', 'x')}</div>
      ${field('ssn', 'SSN', '123-45-6789')}
      ${field('fn', 'Full Name', 'Jane Doe')}
      ${field('q', 'Why this role?', 'Because I love it.')}`,
      [WHY],
    ],
    [
      'NEVER scans a <select> — a rewritten free-text answer cannot map onto fixed options',
      YEARS_SELECT,
      [],
    ],
  ])('%s', (_name, html, expected) => {
    expect(filledOf(html)).toEqual(expected);
  });
});

// The rewrite path (`collectFilledFields` → `locateFilledField`) is the one that
// would hand readonly boilerplate back as an editable "answer".
it('leaves a readonly field out of the REWRITE scan, and unlocatable for rewriting', () => {
  setForm(`
    <label for="ro2">Standard terms</label><textarea id="ro2" readonly>Boilerplate text.</textarea>
    ${field('ans', 'Why this role?', 'Because I love it.')}
  `);
  expect(collectFilledFields(document)).toEqual([WHY]);
  expect(locateFilledField(document, 'Standard terms', 0, 1)).toBeNull();
});

describe('locateFilledField — re-scans the CURRENT filled candidates by (question, index, expectedCount)', () => {
  it('locates the exact element a matching scan would have produced (unchanged page still replaces)', () => {
    setForm(field('q1', 'Why this role?', 'Because I love it.'));
    const el = locateFilledField(document, 'Why this role?', 0, 1);
    expect(el?.id).toBe('q1');
  });

  it('disambiguates same-labelled fields by occurrence index', () => {
    setForm(`${field('q1', 'Comments', 'A')}${field('q2', 'Comments', 'B')}`);
    expect(locateFilledField(document, 'Comments', 0, 2)?.id).toBe('q1');
    expect(locateFilledField(document, 'Comments', 1, 2)?.id).toBe('q2');
  });

  it('fails safe (returns null) when the field was cleared since the scan', () => {
    setForm(field('q1', 'Why this role?', 'Because I love it.'));
    expect(locateFilledField(document, 'Why this role?', 0, 1)).not.toBeNull();
    (document.getElementById('q1') as HTMLInputElement).value = '';
    expect(locateFilledField(document, 'Why this role?', 0, 1)).toBeNull();
  });

  it('fails safe (returns null) when the CURRENT occurrence count no longer matches the scan-time count', () => {
    setForm(field('q1', 'Comments', 'A'));
    expect(locateFilledField(document, 'Comments', 0, 1)?.id).toBe('q1');

    document.querySelector('form')!.insertAdjacentHTML('afterbegin', field('q0', 'Comments', 'B'));

    expect(locateFilledField(document, 'Comments', 0, 1)).toBeNull();
  });

  it('never locates a <select>, even if one shares the exact question text', () => {
    setForm(YEARS_SELECT);
    expect(locateFilledField(document, 'Years of experience', 0, 1)).toBeNull();
  });
});
