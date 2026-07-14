/**
 * Unit tests for the answers-capture collector
 * (apps/extension/src/lib/answers-capture.ts).
 *
 * jsdom is provided by the vitest environment declared in vitest.config.ts.
 * Mirrors autofill.test.ts's style: build a real form in `document`, run the
 * REAL implementation, and assert which fields were captured.
 */

import { afterEach, describe, expect, it } from 'vitest';

import { collectAnswers, collectQuestions, locateQuestionField } from './answers-capture';

function setForm(html: string): void {
  document.body.innerHTML = `<form>${html}</form>`;
}

afterEach(() => {
  document.body.innerHTML = '';
  document.head.querySelectorAll('style[data-ajh-test]').forEach((s) => s.remove());
});

describe('collectAnswers — label pairing', () => {
  it('pairs a filled text input with its <label for> text', () => {
    setForm(
      `<label for="q1">Why this role?</label><input id="q1" type="text" value="Because I love it." />`
    );
    expect(collectAnswers(document)).toEqual([
      { question: 'Why this role?', answer: 'Because I love it.' },
    ]);
  });

  it('pairs a filled field with its wrapping <label> text', () => {
    setForm(`<label>Why this role? <input type="text" value="Because I love it." /></label>`);
    const result = collectAnswers(document);
    expect(result).toHaveLength(1);
    expect(result[0]?.answer).toBe('Because I love it.');
    expect(result[0]?.question).toContain('Why this role?');
  });

  it('captures multiple filled fields, each paired with its own label', () => {
    setForm(`
      <label for="q1">Why this role?</label><input id="q1" type="text" value="A" />
      <label for="q2">Notice period?</label><input id="q2" type="text" value="2 weeks" />
    `);
    const result = collectAnswers(document);
    expect(result).toContainEqual({ question: 'Why this role?', answer: 'A' });
    expect(result).toContainEqual({ question: 'Notice period?', answer: '2 weeks' });
  });
});

describe('collectAnswers — textarea', () => {
  it('captures a filled, labelled textarea', () => {
    setForm(
      `<label for="cl">Cover letter</label><textarea id="cl">I would love to join.</textarea>`
    );
    expect(collectAnswers(document)).toEqual([
      { question: 'Cover letter', answer: 'I would love to join.' },
    ]);
  });

  it('skips an empty/whitespace-only textarea', () => {
    setForm(`<label for="cl">Cover letter</label><textarea id="cl">   </textarea>`);
    expect(collectAnswers(document)).toEqual([]);
  });
});

describe('collectAnswers — select captures the visible option text, not the value', () => {
  it("captures the selected option's displayed text", () => {
    setForm(`
      <label for="yrs">Years of experience</label>
      <select id="yrs">
        <option value="0">Choose one</option>
        <option value="1" selected>5-10 years</option>
      </select>
    `);
    expect(collectAnswers(document)).toEqual([
      { question: 'Years of experience', answer: '5-10 years' },
    ]);
  });

  it('skips a select with nothing meaningfully selected (blank option text)', () => {
    setForm(`
      <label for="yrs">Years of experience</label>
      <select id="yrs">
        <option value="" selected></option>
        <option value="1">5-10 years</option>
      </select>
    `);
    expect(collectAnswers(document)).toEqual([]);
  });

  it('skips a select whose selected option has an empty value even when its text is non-blank (placeholder)', () => {
    setForm(`
      <label for="yrs">Years of experience</label>
      <select id="yrs">
        <option value="" selected>Choose one</option>
        <option value="1">5-10 years</option>
      </select>
    `);
    expect(collectAnswers(document)).toEqual([]);
  });
});

describe('collectAnswers — skips password/hidden/file/checkbox/radio inputs', () => {
  it('never captures a password, hidden, file, checkbox, or radio input', () => {
    setForm(`
      <label for="pw">Password</label><input id="pw" type="password" value="secret" />
      <label for="hid">Hidden</label><input id="hid" type="hidden" value="x" />
      <label for="file">Resume</label><input id="file" type="file" />
      <label for="chk">Agree</label><input id="chk" type="checkbox" checked />
      <label for="rad">Choice</label><input id="rad" type="radio" checked value="a" />
    `);
    expect(collectAnswers(document)).toEqual([]);
  });
});

describe('collectAnswers — skips unlabeled and blank/whitespace fields', () => {
  it('skips a filled field with no associated label text', () => {
    setForm(`<input id="nolabel" type="text" value="orphan answer" />`);
    expect(collectAnswers(document)).toEqual([]);
  });

  it('skips an empty text input', () => {
    setForm(`<label for="q">Question</label><input id="q" type="text" value="" />`);
    expect(collectAnswers(document)).toEqual([]);
  });

  it('skips a whitespace-only text input', () => {
    setForm(`<label for="q">Question</label><input id="q" type="text" value="   " />`);
    expect(collectAnswers(document)).toEqual([]);
  });
});

describe('collectAnswers — the AMBIGUOUS/sensitive denylist (shared with autofill.ts)', () => {
  it('skips ambiguous/sensitive labels (referrer, ssn, company, confirm)', () => {
    setForm(`
      <label for="ref">Referrer name</label><input id="ref" type="text" value="Jane" />
      <label for="ssn">SSN</label><input id="ssn" type="text" value="123-45-6789" />
      <label for="co">Company you work for now</label><input id="co" type="text" value="Acme" />
      <label for="cf">Confirm answer</label><input id="cf" type="text" value="yes" />
    `);
    expect(collectAnswers(document)).toEqual([]);
  });

  it('does not capture a filled "Driver\'s license number" field', () => {
    setForm(
      `<label for="dl">Driver's license number</label><input id="dl" type="text" value="D1234567" />`
    );
    expect(collectAnswers(document)).toEqual([]);
  });
});

describe('collectAnswers — excludes identity fields (contact-profile data, not answers)', () => {
  it('does not capture a filled "Full Name" text field', () => {
    setForm(`<label for="fn">Full Name</label><input id="fn" type="text" value="Jane Doe" />`);
    expect(collectAnswers(document)).toEqual([]);
  });

  it('does not capture a filled "LinkedIn" text field', () => {
    setForm(
      `<label for="li">LinkedIn</label><input id="li" type="text" value="https://linkedin.com/in/jane" />`
    );
    expect(collectAnswers(document)).toEqual([]);
  });

  it('still captures a genuine application question', () => {
    setForm(
      `<label for="q">Why do you want to work here?</label><input id="q" type="text" value="Because I love it." />`
    );
    expect(collectAnswers(document)).toEqual([
      { question: 'Why do you want to work here?', answer: 'Because I love it.' },
    ]);
  });
});

describe('collectAnswers — autocomplete-aware identity exclusion (shared with autofill.ts Tier 1)', () => {
  it('does not capture an autocomplete="name" field even under a quirky, non-identity-looking label', () => {
    setForm(
      `<label for="fn">Your details</label><input id="fn" type="text" autocomplete="name" value="Jane Doe" />`
    );
    expect(collectAnswers(document)).toEqual([]);
  });

  it('still captures an autocomplete="off" textarea labelled as a genuine question', () => {
    setForm(
      `<label for="q">Why this role?</label><textarea id="q" autocomplete="off">Because I love it.</textarea>`
    );
    expect(collectAnswers(document)).toEqual([
      { question: 'Why this role?', answer: 'Because I love it.' },
    ]);
  });
});

describe('collectAnswers — visibility (computed-style-only, jsdom-safe)', () => {
  it('skips a field hidden by an ancestor display:none', () => {
    setForm(
      `<div style="display:none"><label for="q">Question</label><input id="q" type="text" value="A" /></div>`
    );
    expect(collectAnswers(document)).toEqual([]);
  });

  it('skips a field hidden by an ancestor CSS class (honeypot), not just inline style', () => {
    const style = document.createElement('style');
    style.setAttribute('data-ajh-test', '');
    style.textContent = '.ajh-visually-hidden { display: none; }';
    document.head.appendChild(style);

    setForm(
      `<div class="ajh-visually-hidden"><label for="q">Question</label><input id="q" type="text" value="A" /></div>`
    );
    expect(collectAnswers(document)).toEqual([]);
  });

  it('still captures a normal visible sibling alongside a hidden honeypot field', () => {
    setForm(`
      <div style="display:none"><label for="hp">Trap</label><input id="hp" type="text" value="bot" /></div>
      <label for="q">Question</label><input id="q" type="text" value="A" />
    `);
    expect(collectAnswers(document)).toEqual([{ question: 'Question', answer: 'A' }]);
  });
});

// ── collectQuestions — the "questions mode" collector (answers.suggest) ────────

describe('collectQuestions — scans EMPTY candidate fields, the mirror of collectAnswers', () => {
  it('scans an empty, labelled text input at index 0', () => {
    setForm(`<label for="q1">Why this role?</label><input id="q1" type="text" value="" />`);
    expect(collectQuestions(document)).toEqual([{ question: 'Why this role?', index: 0 }]);
  });

  it('skips a FILLED field — the mirror of collectAnswers skipping empty ones', () => {
    setForm(
      `<label for="q1">Why this role?</label><input id="q1" type="text" value="Already answered" />`
    );
    expect(collectQuestions(document)).toEqual([]);
  });

  it('assigns increasing occurrence indices to fields sharing the exact same label', () => {
    setForm(`
      <label for="q1">Comments</label><input id="q1" type="text" value="" />
      <label for="q2">Comments</label><textarea id="q2"></textarea>
    `);
    expect(collectQuestions(document)).toEqual([
      { question: 'Comments', index: 0 },
      { question: 'Comments', index: 1 },
    ]);
  });

  it('applies the SAME visibility/denylist/identity gates as collectAnswers', () => {
    setForm(`
      <div style="display:none"><label for="hp">Trap</label><input id="hp" type="text" value="" /></div>
      <label for="ssn">SSN</label><input id="ssn" type="text" value="" />
      <label for="fn">Full Name</label><input id="fn" type="text" value="" />
      <label for="q">Why this role?</label><input id="q" type="text" value="" />
    `);
    expect(collectQuestions(document)).toEqual([{ question: 'Why this role?', index: 0 }]);
  });

  it('scans an empty select whose selected option has an empty value (placeholder)', () => {
    setForm(`
      <label for="yrs">Years of experience</label>
      <select id="yrs">
        <option value="" selected>Choose one</option>
        <option value="1">5-10 years</option>
      </select>
    `);
    expect(collectQuestions(document)).toEqual([{ question: 'Years of experience', index: 0 }]);
  });

  it('skips a select that already has a meaningful selection', () => {
    setForm(`
      <label for="yrs">Years of experience</label>
      <select id="yrs">
        <option value="0">Choose one</option>
        <option value="1" selected>5-10 years</option>
      </select>
    `);
    expect(collectQuestions(document)).toEqual([]);
  });
});

// ── locateQuestionField — the fill-target re-scan (fail-safe correlation) ──────

describe('locateQuestionField — re-scans the CURRENT empty candidates by (question, index, expectedCount)', () => {
  it('locates the exact element a matching scan would have produced (unchanged page still fills)', () => {
    setForm(`<label for="q1">Why this role?</label><input id="q1" type="text" value="" />`);
    const el = locateQuestionField(document, 'Why this role?', 0, 1);
    expect(el?.id).toBe('q1');
  });

  it('disambiguates same-labelled fields by occurrence index', () => {
    setForm(`
      <label for="q1">Comments</label><input id="q1" type="text" value="" />
      <label for="q2">Comments</label><input id="q2" type="text" value="" />
    `);
    expect(locateQuestionField(document, 'Comments', 0, 2)?.id).toBe('q1');
    expect(locateQuestionField(document, 'Comments', 1, 2)?.id).toBe('q2');
  });

  it('fails safe (returns null) when the field was filled since the scan', () => {
    setForm(`<label for="q1">Why this role?</label><input id="q1" type="text" value="" />`);
    expect(locateQuestionField(document, 'Why this role?', 0, 1)).not.toBeNull();
    // The user (or the page) filled it in the meantime.
    (document.getElementById('q1') as HTMLInputElement).value = 'Already answered';
    expect(locateQuestionField(document, 'Why this role?', 0, 1)).toBeNull();
  });

  it('fails safe (returns null) when the occurrence no longer exists', () => {
    setForm(`<label for="q1">Comments</label><input id="q1" type="text" value="" />`);
    expect(locateQuestionField(document, 'Comments', 1, 1)).toBeNull();
  });

  it('fails safe (returns null) for a question that was never scanned', () => {
    setForm(`<label for="q1">Why this role?</label><input id="q1" type="text" value="" />`);
    expect(locateQuestionField(document, 'A different question?', 0, 1)).toBeNull();
  });

  it('fails safe (returns null) when the CURRENT occurrence count no longer matches the scan-time count, even though the requested index still resolves to SOME element', () => {
    // Scan time: exactly one "Comments" field, at index 0.
    setForm(`<label for="q1">Comments</label><input id="q1" type="text" value="" />`);
    expect(locateQuestionField(document, 'Comments', 0, 1)?.id).toBe('q1');

    // A new same-labelled field is inserted EARLIER in DOM order before the
    // fill click — the requested index (0) still "resolves" to an element,
    // but it is now the WRONG one (never the one the scan saw).
    const wrapper = document.querySelector('form')!;
    wrapper.insertAdjacentHTML(
      'afterbegin',
      `<label for="q0">Comments</label><input id="q0" type="text" value="" />`
    );

    expect(locateQuestionField(document, 'Comments', 0, 1)).toBeNull();
  });
});
