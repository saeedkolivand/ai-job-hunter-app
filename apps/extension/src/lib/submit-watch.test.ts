/**
 * Unit tests for the gesture-armed submit watcher
 * (apps/extension/src/lib/submit-watch.ts).
 *
 * jsdom is provided by the vitest environment (vitest.config.ts). Mirrors
 * answers-capture/collect-answers.test.ts's style: build a real form in the shared `document`,
 * arm the REAL watcher, dispatch real DOM events, and assert what it posted.
 *
 * Visibility is asserted via computed style ONLY (jsdom always reports
 * getBoundingClientRect/offsetWidth as zero — see field-signal.isHidden).
 */

import { afterEach, describe, expect, it, vi } from 'vitest';

import { armSubmitWatch } from './submit-watch';

/** The fields that make a `<form>` read as a real application form rather than
 *  a search box / newsletter signup (see `looksLikeApplicationForm`). */
const APPLICATION_FIELDS = `
  <input name="first_name" />
  <input name="email" type="email" />
  <input name="resume" type="file" />
`;

/** An application form with one answered free-text question. */
const answerForm = (submitLabel = 'Submit application'): string => `
  <form id="f">
    ${APPLICATION_FIELDS}
    <label for="q">Why this role?</label>
    <textarea id="q">Because I love it.</textarea>
    <button type="submit">${submitLabel}</button>
  </form>
`;
const ANSWERS = [{ question: 'Why this role?', answer: 'Because I love it.' }];

const NEWSLETTER = `
  <form id="newsletter">
    <label for="nlEmail">Newsletter email</label>
    <input id="nlEmail" name="newsletter_email" value="me@example.com" />
  </form>
`;

/** Put `html` in the body and arm the REAL watcher against a `post` spy. */
function arm(html: string, options?: Parameters<typeof armSubmitWatch>[2]) {
  document.body.innerHTML = html;
  const post = vi.fn();
  armSubmitWatch(document, post, options);
  return post;
}

const submit = (id = 'f'): void => {
  document
    .getElementById(id)!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
};

const click = (selector = 'button'): void => {
  document
    .querySelector(selector)!
    .dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
};

afterEach(() => {
  document.body.innerHTML = '';
});

describe('armSubmitWatch — real form submit', () => {
  it('posts once on a real form submit', () => {
    const post = arm(
      `<form id="f">${APPLICATION_FIELDS}<button type="submit">Submit application</button></form>`
    );
    submit();

    expect(post).toHaveBeenCalledTimes(1);
    // The current page URL is posted (jsdom's default location).
    expect(typeof post.mock.calls[0]?.[0]).toBe('string');
  });

  it('does NOT post on a search / newsletter form submit', () => {
    // The listener sees EVERY form on the page and reports only location.href,
    // so an unscoped submit listener auto-marked the application "applied" when
    // the user pressed Enter in the site's search box.
    const post = arm(`
      <form id="search"><input type="search" name="q" /><button type="submit">Search</button></form>
      <form id="news"><input type="email" name="email" /><button type="submit">Subscribe</button></form>
    `);
    submit('search');
    submit('news');

    expect(post).not.toHaveBeenCalled();
  });

  it('OBSERVES ONLY — never preventDefault on the submit', () => {
    arm(`<form id="f">${APPLICATION_FIELDS}<button type="submit">Apply</button></form>`);

    const evt = new Event('submit', { bubbles: true, cancelable: true });
    const notCancelled = document.getElementById('f')!.dispatchEvent(evt);

    expect(evt.defaultPrevented).toBe(false);
    expect(notCancelled).toBe(true);
  });
});

describe('armSubmitWatch — apply-style click heuristic', () => {
  it.each([
    [
      'an apply-style submit button inside the application form',
      `<form>${APPLICATION_FIELDS}<button type="submit">Apply now</button></form>`,
      'button',
    ],
    [
      'a role="button" apply control (Easy-Apply / SPA, no native submit)',
      `<div role="button">Submit application</div>`,
      '[role="button"]',
    ],
    [
      'an input[type=submit] whose value matches',
      `<input type="submit" value="Finish" />`,
      'input',
    ],
    [
      'a click that lands on a child element of the control',
      `<form>${APPLICATION_FIELDS}<button type="submit"><span>Apply</span> now</button></form>`,
      'span',
    ],
  ])('posts on a click of %s', (_name, html, selector) => {
    const post = arm(html);
    click(selector);

    expect(post).toHaveBeenCalledTimes(1);
  });

  it.each([
    [
      // On a job-listing page this control OPENS the application (often a modal);
      // nothing has been submitted, so the app must not be marked applied.
      'a bare "Apply now" that is not inside an application form',
      `<button type="submit">Apply now</button><div role="button">Apply</div>`,
    ],
    [
      // Formless, so only the click heuristic is in play — a submit button inside
      // a form implicitly submits it, which is a separate (and legitimate) signal.
      'a non-apply submit button (e.g. "Save draft")',
      `<button type="submit">Save draft</button>`,
    ],
    [
      // The text matches, so `isHidden` is the only thing keeping this quiet.
      'a hidden (display:none) apply button — computed-style only',
      `<button type="submit" style="display:none">Submit application</button>`,
    ],
  ])('does NOT fire on %s', (_name, html) => {
    const post = arm(html);
    for (const control of document.querySelectorAll('button, [role="button"]')) {
      control.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    }

    expect(post).not.toHaveBeenCalled();
  });
});

describe('armSubmitWatch — hidden résumé file input (#786 follow-up)', () => {
  // A styled upload widget hides the native <input type=file> (display:none).
  // Only a résumé/CV-flavored hidden file input is decisive; a bare hidden
  // upload on a non-application form stays below the 3-field bar, so the module
  // keeps under-reporting rather than over-reporting. Visibility is
  // computed-style only (isHidden).
  it.each([
    [
      'recognizes a form whose résumé file input is hidden behind a custom upload button',
      `<input name="first_name" /><input type="file" name="resume" style="display:none" />`,
      1,
    ],
    [
      'still ignores a hidden NON-résumé file input with too few visible fields',
      `<input name="q" /><input type="file" style="display:none" />`,
      0,
    ],
  ])('%s', (_name, fields, calls) => {
    const post = arm(
      `<form id="f">${fields}<button type="submit">Submit application</button></form>`
    );
    submit();

    expect(post).toHaveBeenCalledTimes(calls);
  });
});

describe('armSubmitWatch — non-application forms (#786 lows)', () => {
  it('does NOT treat a checkbox/radio-only form as an application form', () => {
    // A filter / cookie-consent / survey widget is built only from checkboxes or
    // radios; a real application form clears the bar on its text/email/résumé
    // fields, so these are excluded from the fillable-field count.
    const post = arm(`
      <form id="f">
        <input type="checkbox" name="a" />
        <input type="checkbox" name="b" />
        <input type="radio" name="c" value="1" />
        <input type="radio" name="c" value="2" />
        <button type="submit">Submit application</button>
      </form>
    `);
    submit();

    expect(post).not.toHaveBeenCalled();
  });

  // Real browsers carry the pressed button as the SubmitEvent's `submitter`;
  // a draft-save submit must not auto-advance the application to `applied`.
  it.each([
    ['does NOT fire when a "Save draft" button submits the application form', 'draft', 0],
    ['still fires when the real submit button sends the application form', 'send', 1],
  ])('%s', (_name, submitterId, calls) => {
    const post = arm(`
      <form id="f">
        ${APPLICATION_FIELDS}
        <button id="draft" type="submit">Save draft</button>
        <button id="send" type="submit">Submit application</button>
      </form>
    `);
    document.getElementById('f')!.dispatchEvent(
      new SubmitEvent('submit', {
        submitter: document.getElementById(submitterId),
        bubbles: true,
        cancelable: true,
      })
    );

    expect(post).toHaveBeenCalledTimes(calls);
  });
});

describe('armSubmitWatch — fire-once guard', () => {
  it('posts AT MOST ONCE when the apply click AND its submit both fire', () => {
    const post = arm(
      `<form id="f">${APPLICATION_FIELDS}<button type="submit">Apply</button></form>`
    );
    // A real click on the apply button, then the submit it triggers.
    click();
    submit();

    expect(post).toHaveBeenCalledTimes(1);
  });
});

describe('armSubmitWatch — save-answers-on-submit capture (PR4)', () => {
  it('does NOT capture answers when captureAnswers is absent (default false)', () => {
    const post = arm(answerForm());
    submit();

    expect(post).toHaveBeenCalledTimes(1);
    expect(post.mock.calls[0]?.[1]).toBeUndefined();
  });

  it('captures the currently-filled answers SYNCHRONOUSLY when armed with captureAnswers:true', () => {
    const post = arm(answerForm(), { captureAnswers: true });
    submit();

    expect(post).toHaveBeenCalledTimes(1);
    expect(post.mock.calls[0]?.[0]).toEqual(expect.any(String));
    expect(post.mock.calls[0]?.[1]).toEqual(ANSWERS);
  });

  it('omits answers (rather than an empty array) when armed but nothing is filled — present means "something to save"', () => {
    const post = arm(
      `<form id="f">${APPLICATION_FIELDS}<button type="submit">Submit application</button></form>`,
      { captureAnswers: true }
    );
    submit();

    expect(post.mock.calls[0]?.[1]).toBeUndefined();
  });
});

describe('armSubmitWatch — capture is scoped to the submitted application form (PR-1209)', () => {
  it('does NOT include an unrelated filled form’s fields in the captured answers', () => {
    const post = arm(answerForm() + NEWSLETTER, { captureAnswers: true });
    submit();

    expect(post.mock.calls[0]?.[1]).toEqual(ANSWERS);
  });

  it('click-only detection resolves the associated form and scopes capture to it', () => {
    const post = arm(answerForm('Apply now'), { captureAnswers: true });
    click();

    expect(post.mock.calls[0]?.[1]).toEqual(ANSWERS);
  });

  it('click-only detection with no resolvable form captures NOTHING rather than the whole document', () => {
    const post = arm(`<div role="button">Submit application</div>${NEWSLETTER}`, {
      captureAnswers: true,
    });
    click('[role="button"]');

    expect(post).toHaveBeenCalledTimes(1);
    expect(post.mock.calls[0]?.[1]).toBeUndefined();
  });
});

describe('armSubmitWatch — captureAnswers is read at FIRE TIME, not arm time (PR-1209)', () => {
  it.each([
    [
      'captures when the getter flips ON between arming and firing (no re-arm needed)',
      false,
      ANSWERS,
    ],
    [
      'does NOT capture when the getter flips OFF between arming and firing (no re-arm needed)',
      true,
      undefined,
    ],
  ])('%s', (_name, armedWith, expected) => {
    let capture = armedWith;
    const post = arm(answerForm(), { captureAnswers: () => capture });

    capture = !armedWith; // e.g. the desktop-enforced opt-in flipped mid-frame
    submit();

    expect(post.mock.calls[0]?.[1]).toEqual(expected);
  });
});
