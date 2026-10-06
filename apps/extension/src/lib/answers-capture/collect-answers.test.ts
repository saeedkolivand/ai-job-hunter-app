/**
 * Unit tests for `collectAnswers` / `hasAnswerCapturableFields`
 * (apps/extension/src/lib/answers-capture.ts) — the "save my answers" collector.
 *
 * jsdom is provided by the vitest environment declared in vitest.config.ts.
 * Mirrors autofill/fills.test.ts's style: build a real form in `document`, run the
 * REAL implementation, and assert which fields were captured.
 */

import { afterEach, describe, expect, it } from 'vitest';

import { collectAnswers, hasAnswerCapturableFields } from '../answers-capture';
import { field, resetDocument, setForm } from './test-support';

afterEach(resetDocument);

const answersOf = (html: string) => {
  setForm(html);
  return collectAnswers(document);
};

const WHY = { question: 'Why this role?', answer: 'Because I love it.' };
const YEARS_SELECT = (options: string) => `
  <label for="yrs">Years of experience</label>
  <select id="yrs">${options}</select>`;

describe('collectAnswers — what gets captured', () => {
  it.each([
    [
      'pairs a filled text input with its <label for> text',
      field('q1', 'Why this role?', 'Because I love it.'),
      [WHY],
    ],
    [
      'captures a filled, labelled textarea',
      `<label for="cl">Cover letter</label><textarea id="cl">I would love to join.</textarea>`,
      [{ question: 'Cover letter', answer: 'I would love to join.' }],
    ],
    [
      "captures the selected option's displayed text",
      YEARS_SELECT(
        `<option value="0">Choose one</option><option value="1" selected>5-10 years</option>`
      ),
      [{ question: 'Years of experience', answer: '5-10 years' }],
    ],
    [
      'still captures a genuine application question',
      field('q', 'Why do you want to work here?', 'Because I love it.'),
      [{ question: 'Why do you want to work here?', answer: 'Because I love it.' }],
    ],
    [
      // The twitter row's `x` matches only when the whole signal is x tokens OR
      // an x immediately paired with a handle-ish qualifier, so a prose
      // standalone-x inside an otherwise-named question is NOT identities and
      // stays capturable (#1218 follow-up — `\bx\b` would have dropped it).
      'still captures a genuine question whose text contains a standalone "x" — "Mac OS X experience"',
      field('osx', 'Mac OS X experience', '5 years daily'),
      [{ question: 'Mac OS X experience', answer: '5 years daily' }],
    ],
    [
      'still captures an autocomplete="off" textarea labelled as a genuine question',
      `<label for="q">Why this role?</label><textarea id="q" autocomplete="off">Because I love it.</textarea>`,
      [WHY],
    ],
    [
      // Mirrors autofill's `isCandidateField` gate — fill and capture must agree
      // on what counts as a real, user-editable field.
      'skips a readonly textarea (boilerplate) and a disabled input',
      `
      <label for="terms">Terms</label><textarea id="terms" readonly>Standard terms apply.</textarea>
      ${field('ref', 'Requisition', 'REQ-1', 'disabled')}
      ${field('q', 'Why this role?', 'Because I love it.')}`,
      [WHY],
    ],
    [
      'still captures a normal visible sibling alongside a hidden honeypot field',
      `<div style="display:none">${field('hp', 'Trap', 'bot')}</div>${field('q', 'Question', 'A')}`,
      [{ question: 'Question', answer: 'A' }],
    ],
  ])('%s', (_name, html, expected) => {
    expect(answersOf(html)).toEqual(expected);
  });

  it('pairs a filled field with its wrapping <label> text', () => {
    const result = answersOf(
      `<label>Why this role? <input type="text" value="Because I love it." /></label>`
    );
    expect(result).toHaveLength(1);
    expect(result[0]?.answer).toBe('Because I love it.');
    expect(result[0]?.question).toContain('Why this role?');
  });

  it('captures multiple filled fields, each paired with its own label', () => {
    const result = answersOf(
      `${field('q1', 'Why this role?', 'A')}${field('q2', 'Notice period?', '2 weeks')}`
    );
    expect(result).toContainEqual({ question: 'Why this role?', answer: 'A' });
    expect(result).toContainEqual({ question: 'Notice period?', answer: '2 weeks' });
  });
});

describe('collectAnswers — what is NOT an answer', () => {
  it.each([
    [
      'an empty/whitespace-only textarea',
      `<label for="cl">Cover letter</label><textarea id="cl">   </textarea>`,
    ],
    [
      'a field disabled by an ancestor <fieldset disabled>',
      `<fieldset disabled>${field('d', 'Notice period', '2 weeks')}</fieldset>`,
    ],
    [
      'a select with nothing meaningfully selected (blank option text)',
      YEARS_SELECT(`<option value="" selected></option><option value="1">5-10 years</option>`),
    ],
    [
      'a select whose selected option has an empty value even when its text is non-blank (placeholder)',
      YEARS_SELECT(
        `<option value="" selected>Choose one</option><option value="1">5-10 years</option>`
      ),
    ],
    [
      'a password, hidden, file, checkbox, or radio input',
      `
      <label for="pw">Password</label><input id="pw" type="password" value="secret" />
      <label for="hid">Hidden</label><input id="hid" type="hidden" value="x" />
      <label for="file">Resume</label><input id="file" type="file" />
      <label for="chk">Agree</label><input id="chk" type="checkbox" checked />
      <label for="rad">Choice</label><input id="rad" type="radio" checked value="a" />`,
    ],
    [
      'a filled field with no associated label text',
      `<input id="nolabel" type="text" value="orphan answer" />`,
    ],
    ['an empty text input', field('q', 'Question')],
    ['a whitespace-only text input', field('q', 'Question', '   ')],
    [
      'ambiguous/sensitive labels (referrer, ssn, company, confirm)',
      `
      ${field('ref', 'Referrer name', 'Jane')}
      ${field('ssn', 'SSN', '123-45-6789')}
      ${field('co', 'Company you work for now', 'Acme')}
      ${field('cf', 'Confirm answer', 'yes')}`,
    ],
    [
      'a filled "Driver\'s license number" field',
      field('dl', "Driver's license number", 'D1234567'),
    ],
    ['a filled "Full Name" text field', field('fn', 'Full Name', 'Jane Doe')],
    ['a filled "LinkedIn" text field', field('li', 'LinkedIn', 'https://linkedin.com/in/jane')],
    // A bare "X" box is a profile-handle field, not an essay question — and
    // #1218's bug was that its question text was fed to the AI as if it were
    // one.
    [
      'a filled "X / Twitter" identity field (#1218)',
      field('x', 'X / Twitter', 'https://x.com/jane'),
    ],
    // The longer-label shape: a field labelled "X handle" resolves to the
    // twitter identity row through the x+qualifier branch, so it is excluded
    // from capture exactly like the bare "X / Twitter" box above.
    ['a filled "X handle" identity field (#1218)', field('xh', 'X handle', 'https://x.com/jane')],
    // Capture and autofill share isAmbiguousSignal + matchNamedKey, so the
    // localized denylist/identity terms must behave identically on this side too.
    [
      'a filled German "Vorname" field (matchNamedKey → firstName identity)',
      field('vn', 'Vorname', 'Saeed'),
    ],
    [
      'a filled "Name des Ansprechpartners" / "Notfallkontakt" third-party field',
      `${field('ap', 'Name des Ansprechpartners', 'Jane Doe')}${field('nk', 'Notfallkontakt', '+49 111 222')}`,
    ],
    [
      'an autocomplete="name" field even under a quirky, non-identity-looking label',
      field('fn', 'Your details', 'Jane Doe', 'autocomplete="name"'),
    ],
    [
      'a field hidden by an ancestor display:none',
      `<div style="display:none">${field('q', 'Question', 'A')}</div>`,
    ],
  ])('skips %s', (_name, html) => {
    expect(answersOf(html)).toEqual([]);
  });

  it('skips a field hidden by an ancestor CSS class (honeypot), not just inline style', () => {
    const style = document.createElement('style');
    style.setAttribute('data-ajh-test', '');
    style.textContent = '.ajh-visually-hidden { display: none; }';
    document.head.appendChild(style);

    expect(
      answersOf(`<div class="ajh-visually-hidden">${field('q', 'Question', 'A')}</div>`)
    ).toEqual([]);
  });

  it('skips a disabled <select> (the :disabled half applies to selects too)', () => {
    setForm(`
      <label for="sel">Work authorization</label>
      <select id="sel" disabled><option value="yes" selected>Yes</option></select>
    `);
    expect(collectAnswers(document)).toEqual([]);
    expect(hasAnswerCapturableFields(document)).toBe(false);
  });
});

describe('collectAnswers — scoped to a specific form (PR-1209, submit-watch capture)', () => {
  it('passing an HTMLFormElement scopes the scan to that form only, excluding a second filled form', () => {
    document.body.innerHTML = `
      <form id="app">${field('q', 'Question', 'A')}</form>
      <form id="other">${field('q2', 'Other question', 'B')}</form>
    `;
    const appForm = document.getElementById('app') as HTMLFormElement;
    expect(collectAnswers(appForm)).toEqual([{ question: 'Question', answer: 'A' }]);
  });
});

describe("hasAnswerCapturableFields — the popup fields-probe's NARROWER signal (Answer-tools gating)", () => {
  it.each([
    [
      'false for a page with no form fields at all (a plain job listing)',
      `<p>Senior Rust Engineer at Acme Corp.</p>`,
      false,
    ],
    [
      'true when the page has an EMPTY, capturable candidate field',
      field('q1', 'Why this role?'),
      true,
    ],
    [
      'true when the page has a FILLED, capturable candidate field',
      field('q1', 'Why this role?', 'Because I love it.'),
      true,
    ],
    [
      'false when every candidate field is excluded (ambiguous/identity/hidden — same gates as Save/Suggest)',
      `
      ${field('pw', 'Password')}
      ${field('ln', 'LinkedIn URL', 'https://linkedin.com/in/x')}
      ${field('h', 'Honeypot', '', 'style="display:none"')}`,
      false,
    ],
    [
      // The narrow signal deliberately excludes these; hasAutofillableFields
      // (lib/autofill.ts) covers them for the Form group's wider union.
      'false for an IDENTITY-ONLY form (name/email/phone)',
      `
      ${field('name', 'Full name')}
      <label for="email">Email</label><input id="email" type="email" value="" />
      <label for="phone">Phone</label><input id="phone" type="tel" value="" />`,
      false,
    ],
  ])('returns %s', (_name, html, expected) => {
    setForm(html);
    expect(hasAnswerCapturableFields(document)).toBe(expected);
  });
});
