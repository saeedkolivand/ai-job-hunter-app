/**
 * Unit tests for the single-field answer filler
 * (apps/extension/src/lib/answer-fill.ts).
 *
 * jsdom is provided by the vitest environment declared in vitest.config.ts.
 * Mirrors answers-capture.test.ts's style: build a real form in `document`,
 * run the REAL implementation, and assert both the fill outcome and the
 * actual DOM value written.
 */

import { afterEach, describe, expect, it } from 'vitest';

import { ANSWER_FILL_GLOBAL, fillAnswerField } from './answer-fill';

function setForm(html: string): void {
  document.body.innerHTML = `<form>${html}</form>`;
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('ANSWER_FILL_GLOBAL', () => {
  it('is the fixed key background.ts duplicates as a local literal', () => {
    // Pinned so a rename here can never silently desync from background.ts's
    // duplicated literal (same discipline as autofill.test.ts's AUTOFILL_GLOBAL pin).
    expect(ANSWER_FILL_GLOBAL).toBe('__ajhRunAnswerFill');
  });
});

describe('fillAnswerField — happy path', () => {
  it('fills the exact labelled text input and dispatches input/change', () => {
    setForm(`<label for="q1">Why this role?</label><input id="q1" type="text" value="" />`);
    let inputFired = false;
    let changeFired = false;
    const el = document.getElementById('q1') as HTMLInputElement;
    el.addEventListener('input', () => {
      inputFired = true;
    });
    el.addEventListener('change', () => {
      changeFired = true;
    });

    const result = fillAnswerField(document, 'Why this role?', 0, 1, 'Because I love it.');

    expect(result).toEqual({ filled: true });
    expect(el.value).toBe('Because I love it.');
    expect(inputFired).toBe(true);
    expect(changeFired).toBe(true);
  });

  it('fills a labelled empty textarea', () => {
    setForm(`<label for="cl">Cover letter</label><textarea id="cl"></textarea>`);
    const result = fillAnswerField(document, 'Cover letter', 0, 1, 'I would love to join.');
    expect(result).toEqual({ filled: true });
    expect((document.getElementById('cl') as HTMLTextAreaElement).value).toBe(
      'I would love to join.'
    );
  });

  it('selects the <select> option whose visible text matches the answer (case/whitespace-insensitive)', () => {
    setForm(`
      <label for="yrs">Years of experience</label>
      <select id="yrs">
        <option value="" selected>Choose one</option>
        <option value="1">5-10 years</option>
      </select>
    `);
    const result = fillAnswerField(document, 'Years of experience', 0, 1, '  5-10 YEARS  ');
    expect(result).toEqual({ filled: true });
    expect((document.getElementById('yrs') as HTMLSelectElement).value).toBe('1');
  });

  it('disambiguates same-labelled fields by occurrence index — never fills the wrong one', () => {
    setForm(`
      <label for="q1">Comments</label><input id="q1" type="text" value="" />
      <label for="q2">Comments</label><input id="q2" type="text" value="" />
    `);
    fillAnswerField(document, 'Comments', 1, 2, 'Second field answer');
    expect((document.getElementById('q1') as HTMLInputElement).value).toBe('');
    expect((document.getElementById('q2') as HTMLInputElement).value).toBe('Second field answer');
  });
});

describe('fillAnswerField — fail-safe on any mutation since the scan', () => {
  it('returns a not-found error when the field was already filled in the meantime', () => {
    setForm(`<label for="q1">Why this role?</label><input id="q1" type="text" value="" />`);
    (document.getElementById('q1') as HTMLInputElement).value = 'Already answered';

    const result = fillAnswerField(document, 'Why this role?', 0, 1, 'A different answer');

    expect(result.filled).toBe(false);
    expect(result.error).toMatch(/page may have changed/i);
    // Never overwrites the user's own value.
    expect((document.getElementById('q1') as HTMLInputElement).value).toBe('Already answered');
  });

  it('returns a not-found error when the occurrence no longer exists', () => {
    setForm(`<label for="q1">Comments</label><input id="q1" type="text" value="" />`);
    const result = fillAnswerField(document, 'Comments', 1, 1, 'An answer');
    expect(result.filled).toBe(false);
  });

  it('returns a not-found error for a question that has no scanned field at all', () => {
    setForm(`<label for="q1">Why this role?</label><input id="q1" type="text" value="" />`);
    const result = fillAnswerField(document, 'A different question?', 0, 1, 'An answer');
    expect(result.filled).toBe(false);
  });

  it('returns a distinct error when a <select> has no option matching the answer text', () => {
    setForm(`
      <label for="yrs">Years of experience</label>
      <select id="yrs">
        <option value="" selected>Choose one</option>
        <option value="1">5-10 years</option>
      </select>
    `);
    const result = fillAnswerField(document, 'Years of experience', 0, 1, 'Twenty years');
    expect(result.filled).toBe(false);
    expect(result.error).toMatch(/options/i);
    // The select must be left untouched, not coerced to a wrong option.
    expect((document.getElementById('yrs') as HTMLSelectElement).value).toBe('');
  });

  it('fails safe (never fills) when a same-labelled field is inserted EARLIER in DOM order since the scan', () => {
    // Scan time: exactly one "Comments" field (occurrence 0, count 1).
    setForm(`<label for="q1">Comments</label><input id="q1" type="text" value="" />`);

    // A new same-labelled field is inserted before it — the requested index
    // (0) still resolves to SOME element, but it is now the wrong one.
    const wrapper = document.querySelector('form')!;
    wrapper.insertAdjacentHTML(
      'afterbegin',
      `<label for="q0">Comments</label><input id="q0" type="text" value="" />`
    );

    const result = fillAnswerField(document, 'Comments', 0, 1, 'An answer');

    expect(result.filled).toBe(false);
    // Neither field was touched — the fail-safe never guesses.
    expect((document.getElementById('q0') as HTMLInputElement).value).toBe('');
    expect((document.getElementById('q1') as HTMLInputElement).value).toBe('');
  });
});
