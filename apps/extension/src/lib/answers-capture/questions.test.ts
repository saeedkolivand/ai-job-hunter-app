/**
 * Unit tests for the "questions mode" collector (`collectQuestions`, answers.suggest)
 * and its fill-target re-scan (`locateQuestionField`) in
 * apps/extension/src/lib/answers-capture.ts.
 */

import { afterEach, describe, expect, it } from 'vitest';

import { collectQuestions, locateQuestionField } from '../answers-capture';
import { field, resetDocument, setForm } from './test-support';

afterEach(resetDocument);

const questionsOf = (html: string) => {
  setForm(html);
  return collectQuestions(document);
};

const YEARS_SELECT = (selected: string) => `
  <label for="yrs">Years of experience</label>
  <select id="yrs">${selected}<option value="1">5-10 years</option></select>`;

describe('collectQuestions — scans EMPTY candidate fields, the mirror of collectAnswers', () => {
  it.each([
    [
      'scans an empty, labelled text input at index 0',
      field('q1', 'Why this role?'),
      [{ question: 'Why this role?', index: 0 }],
    ],
    [
      'skips a FILLED field — the mirror of collectAnswers skipping empty ones',
      field('q1', 'Why this role?', 'Already answered'),
      [],
    ],
    [
      'assigns increasing occurrence indices to fields sharing the exact same label',
      `${field('q1', 'Comments')}<label for="q2">Comments</label><textarea id="q2"></textarea>`,
      [
        { question: 'Comments', index: 0 },
        { question: 'Comments', index: 1 },
      ],
    ],
    [
      'applies the SAME visibility/denylist/identity gates as collectAnswers',
      `
      <div style="display:none">${field('hp', 'Trap')}</div>
      ${field('ssn', 'SSN')}
      ${field('fn', 'Full Name')}
      ${field('q', 'Why this role?')}`,
      [{ question: 'Why this role?', index: 0 }],
    ],
    [
      // #1218's actual bug was here: the Answers tab fed a bare "X" box (and any
      // "Twitter handle" box) to the AI as a question. Identity fields must never
      // appear — while genuine empty questions around them still do.
      'excludes empty X / Twitter identity fields from the question list (#1218)',
      `
      ${field('x', 'X')}
      ${field('tw', 'Twitter handle')}
      ${field('more', 'Tell us more')}
      ${field('why', 'Why this role?')}`,
      [
        { question: 'Tell us more', index: 0 },
        { question: 'Why this role?', index: 0 },
      ],
    ],
    [
      // Tightened `x` guard: prose-x in "Mac OS X experience" is a question, not
      // an identity handle, so it must keep appearing in the Answers-tab list.
      'still scans a genuine empty question whose text contains a standalone "x"',
      field('osxq', 'Mac OS X experience'),
      [{ question: 'Mac OS X experience', index: 0 }],
    ],
    [
      'scans an empty select whose selected option has an empty value (placeholder)',
      YEARS_SELECT(`<option value="" selected>Choose one</option>`),
      [{ question: 'Years of experience', index: 0 }],
    ],
    [
      'skips a select that already has a meaningful selection',
      `
      <label for="yrs">Years of experience</label>
      <select id="yrs"><option value="0">Choose one</option><option value="1" selected>5-10 years</option></select>`,
      [],
    ],
    [
      'leaves a readonly field out of the QUESTIONS scan too',
      `
      <label for="ro">Posting id</label><input id="ro" type="text" readonly />
      ${field('open', 'Why this role?')}`,
      [{ question: 'Why this role?', index: 0 }],
    ],
  ])('%s', (_name, html, expected) => {
    expect(questionsOf(html)).toEqual(expected);
  });
});

describe('locateQuestionField — re-scans the CURRENT empty candidates by (question, index, expectedCount)', () => {
  it('locates the exact element a matching scan would have produced (unchanged page still fills)', () => {
    setForm(field('q1', 'Why this role?'));
    const el = locateQuestionField(document, 'Why this role?', 0, 1);
    expect(el?.id).toBe('q1');
  });

  it('disambiguates same-labelled fields by occurrence index', () => {
    setForm(`${field('q1', 'Comments')}${field('q2', 'Comments')}`);
    expect(locateQuestionField(document, 'Comments', 0, 2)?.id).toBe('q1');
    expect(locateQuestionField(document, 'Comments', 1, 2)?.id).toBe('q2');
  });

  it('fails safe (returns null) when the field was filled since the scan', () => {
    setForm(field('q1', 'Why this role?'));
    expect(locateQuestionField(document, 'Why this role?', 0, 1)).not.toBeNull();
    // The user (or the page) filled it in the meantime.
    (document.getElementById('q1') as HTMLInputElement).value = 'Already answered';
    expect(locateQuestionField(document, 'Why this role?', 0, 1)).toBeNull();
  });

  it('fails safe (returns null) when the occurrence no longer exists', () => {
    setForm(field('q1', 'Comments'));
    expect(locateQuestionField(document, 'Comments', 1, 1)).toBeNull();
  });

  it('fails safe (returns null) for a question that was never scanned', () => {
    setForm(field('q1', 'Why this role?'));
    expect(locateQuestionField(document, 'A different question?', 0, 1)).toBeNull();
  });

  it('fails safe (returns null) when the CURRENT occurrence count no longer matches the scan-time count, even though the requested index still resolves to SOME element', () => {
    // Scan time: exactly one "Comments" field, at index 0.
    setForm(field('q1', 'Comments'));
    expect(locateQuestionField(document, 'Comments', 0, 1)?.id).toBe('q1');

    // A new same-labelled field is inserted EARLIER in DOM order before the
    // fill click — the requested index (0) still "resolves" to an element,
    // but it is now the WRONG one (never the one the scan saw).
    document.querySelector('form')!.insertAdjacentHTML('afterbegin', field('q0', 'Comments'));

    expect(locateQuestionField(document, 'Comments', 0, 1)).toBeNull();
  });
});
