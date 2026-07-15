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

import {
  ANSWER_FILL_GLOBAL,
  ANSWER_REPLACE_GLOBAL,
  fillAnswerField,
  replaceFilledField,
} from './answer-fill';

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

describe('ANSWER_REPLACE_GLOBAL', () => {
  it('is the fixed key background.ts duplicates as a local literal', () => {
    // Pinned so a rename here can never silently desync from background.ts's
    // duplicated literal (same discipline as ANSWER_FILL_GLOBAL's own pin).
    expect(ANSWER_REPLACE_GLOBAL).toBe('__ajhRunAnswerReplace');
  });
});

describe('replaceFilledField — happy path (Accept and Restore both go through this)', () => {
  it('replaces the exact labelled text input and dispatches input/change', () => {
    setForm(
      `<label for="q1">Why this role?</label><input id="q1" type="text" value="Because I love it." />`
    );
    let inputFired = false;
    let changeFired = false;
    const el = document.getElementById('q1') as HTMLInputElement;
    el.addEventListener('input', () => {
      inputFired = true;
    });
    el.addEventListener('change', () => {
      changeFired = true;
    });

    const result = replaceFilledField(
      document,
      'Why this role?',
      0,
      1,
      'Rewritten answer.',
      'Because I love it.'
    );

    expect(result).toEqual({ filled: true });
    expect(el.value).toBe('Rewritten answer.');
    expect(inputFired).toBe(true);
    expect(changeFired).toBe(true);
  });

  it('replaces a filled textarea', () => {
    setForm(`<label for="cl">Cover letter</label><textarea id="cl">Original text.</textarea>`);
    const result = replaceFilledField(
      document,
      'Cover letter',
      0,
      1,
      'Rewritten text.',
      'Original text.'
    );
    expect(result).toEqual({ filled: true });
    expect((document.getElementById('cl') as HTMLTextAreaElement).value).toBe('Rewritten text.');
  });

  it('disambiguates same-labelled fields by occurrence index — never replaces the wrong one', () => {
    setForm(`
      <label for="q1">Comments</label><input id="q1" type="text" value="First" />
      <label for="q2">Comments</label><input id="q2" type="text" value="Second" />
    `);
    replaceFilledField(document, 'Comments', 1, 2, 'Rewritten second', 'Second');
    expect((document.getElementById('q1') as HTMLInputElement).value).toBe('First');
    expect((document.getElementById('q2') as HTMLInputElement).value).toBe('Rewritten second');
  });

  it('restores the frozen original the SAME way Accept writes the rewrite — it is just a different `text`', () => {
    setForm(
      `<label for="q1">Why this role?</label><input id="q1" type="text" value="Original." />`
    );
    replaceFilledField(document, 'Why this role?', 0, 1, 'Rewritten.', 'Original.');
    expect((document.getElementById('q1') as HTMLInputElement).value).toBe('Rewritten.');

    // Restore original: the SAME function, the frozen pre-rewrite text — the
    // caller must pass the UPDATED expected value ("Rewritten.", what the
    // field holds now), not the original, mirroring how popup.ts tracks
    // `rewriteTarget.expectedValue` across a successful Accept.
    replaceFilledField(document, 'Why this role?', 0, 1, 'Original.', 'Rewritten.');
    expect((document.getElementById('q1') as HTMLInputElement).value).toBe('Original.');
  });
});

describe('replaceFilledField — refuses (never clobbers) when the field changed since the pick', () => {
  it('refuses when the field holds a DIFFERENT non-empty value than expected — a manual edit since the pick', () => {
    setForm(
      `<label for="q1">Why this role?</label><input id="q1" type="text" value="A manual edit the user made." />`
    );

    // The popup still believes the field holds the ORIGINAL text it picked —
    // it has no idea the user retyped it since.
    const result = replaceFilledField(
      document,
      'Why this role?',
      0,
      1,
      'A rewritten draft.',
      'Because I love it.'
    );

    expect(result.filled).toBe(false);
    expect(result.error).toMatch(/changed since you picked it/i);
    // The user's manual edit is NEVER overwritten.
    expect((document.getElementById('q1') as HTMLInputElement).value).toBe(
      'A manual edit the user made.'
    );
  });

  it('accepts when the CURRENT value matches expected exactly (the common, unmodified case)', () => {
    setForm(`<label for="q1">Why this role?</label><input id="q1" type="text" value="Kept." />`);
    const result = replaceFilledField(document, 'Why this role?', 0, 1, 'Rewritten.', 'Kept.');
    expect(result).toEqual({ filled: true });
    expect((document.getElementById('q1') as HTMLInputElement).value).toBe('Rewritten.');
  });

  it('tolerates only whitespace differences (the value is captured/compared trimmed, like the scan itself)', () => {
    setForm(
      `<label for="q1">Why this role?</label><input id="q1" type="text" value="  Kept.  " />`
    );
    const result = replaceFilledField(document, 'Why this role?', 0, 1, 'Rewritten.', 'Kept.');
    expect(result).toEqual({ filled: true });
  });
});

describe('replaceFilledField — fail-safe on any mutation since the pick', () => {
  it('returns a not-found error when the field was cleared in the meantime', () => {
    setForm(`<label for="q1">Why this role?</label><input id="q1" type="text" value="Kept." />`);
    (document.getElementById('q1') as HTMLInputElement).value = '';

    const result = replaceFilledField(
      document,
      'Why this role?',
      0,
      1,
      'A different answer',
      'Kept.'
    );

    expect(result.filled).toBe(false);
    expect(result.error).toMatch(/page may have changed/i);
    expect((document.getElementById('q1') as HTMLInputElement).value).toBe('');
  });

  it('returns a not-found error for a <select> — rewrite mode never targets one', () => {
    setForm(`
      <label for="yrs">Years of experience</label>
      <select id="yrs">
        <option value="0">Choose one</option>
        <option value="1" selected>5-10 years</option>
      </select>
    `);
    const result = replaceFilledField(
      document,
      'Years of experience',
      0,
      1,
      'Twenty years',
      '5-10 years'
    );
    expect(result.filled).toBe(false);
  });

  it('fails safe (never replaces) when a same-labelled field is inserted EARLIER in DOM order since the pick', () => {
    setForm(`<label for="q1">Comments</label><input id="q1" type="text" value="Kept." />`);

    const wrapper = document.querySelector('form')!;
    wrapper.insertAdjacentHTML(
      'afterbegin',
      `<label for="q0">Comments</label><input id="q0" type="text" value="New." />`
    );

    const result = replaceFilledField(document, 'Comments', 0, 1, 'An answer', 'Kept.');

    expect(result.filled).toBe(false);
    expect((document.getElementById('q0') as HTMLInputElement).value).toBe('New.');
    expect((document.getElementById('q1') as HTMLInputElement).value).toBe('Kept.');
  });
});
