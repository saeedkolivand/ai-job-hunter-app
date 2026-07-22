/**
 * Unit tests for the gesture-armed submit watcher
 * (apps/extension/src/lib/submit-watch.ts).
 *
 * jsdom is provided by the vitest environment (vitest.config.ts). Mirrors
 * answers-capture.test.ts's style: build a real form in the shared `document`,
 * arm the REAL watcher, dispatch real DOM events, and assert what it posted.
 *
 * Visibility is asserted via computed style ONLY (jsdom always reports
 * getBoundingClientRect/offsetWidth as zero — see field-signal.isHidden).
 */

import { afterEach, describe, expect, it, vi } from 'vitest';

import { armSubmitWatch } from './submit-watch';

function setBody(html: string): void {
  document.body.innerHTML = html;
}

/** The fields that make a `<form>` read as a real application form rather than
 *  a search box / newsletter signup (see `looksLikeApplicationForm`). */
const APPLICATION_FIELDS = `
  <input name="first_name" />
  <input name="email" type="email" />
  <input name="resume" type="file" />
`;

afterEach(() => {
  document.body.innerHTML = '';
});

describe('armSubmitWatch — real form submit', () => {
  it('posts once on a real form submit', () => {
    setBody(
      `<form id="f">${APPLICATION_FIELDS}<button type="submit">Submit application</button></form>`
    );
    const post = vi.fn();
    armSubmitWatch(document, post);

    const form = document.getElementById('f') as HTMLFormElement;
    form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));

    expect(post).toHaveBeenCalledTimes(1);
    // The current page URL is posted (jsdom's default location).
    expect(typeof post.mock.calls[0]?.[0]).toBe('string');
  });

  it('does NOT post on a search / newsletter form submit', () => {
    // The listener sees EVERY form on the page and reports only location.href,
    // so an unscoped submit listener auto-marked the application "applied" when
    // the user pressed Enter in the site's search box.
    setBody(`
      <form id="search"><input type="search" name="q" /><button type="submit">Search</button></form>
      <form id="news"><input type="email" name="email" /><button type="submit">Subscribe</button></form>
    `);
    const post = vi.fn();
    armSubmitWatch(document, post);

    for (const id of ['search', 'news']) {
      (document.getElementById(id) as HTMLFormElement).dispatchEvent(
        new Event('submit', { bubbles: true, cancelable: true })
      );
    }

    expect(post).not.toHaveBeenCalled();
  });

  it('OBSERVES ONLY — never preventDefault on the submit', () => {
    setBody(`<form id="f">${APPLICATION_FIELDS}<button type="submit">Apply</button></form>`);
    armSubmitWatch(document, vi.fn());

    const form = document.getElementById('f') as HTMLFormElement;
    const evt = new Event('submit', { bubbles: true, cancelable: true });
    const notCancelled = form.dispatchEvent(evt);

    expect(evt.defaultPrevented).toBe(false);
    expect(notCancelled).toBe(true);
  });
});

describe('armSubmitWatch — apply-style click heuristic', () => {
  it('posts on a click of an apply-style submit button inside the application form', () => {
    setBody(`<form>${APPLICATION_FIELDS}<button type="submit">Apply now</button></form>`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    document
      .querySelector('button')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));

    expect(post).toHaveBeenCalledTimes(1);
  });

  it('does NOT post on a bare "Apply now" that is not inside an application form', () => {
    // On a job-listing page this control OPENS the application (often a modal);
    // nothing has been submitted, so the app must not be marked applied.
    setBody(`<button type="submit">Apply now</button><div role="button">Apply</div>`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    document.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    document
      .querySelector('[role="button"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }));

    expect(post).not.toHaveBeenCalled();
  });

  it('posts on a role="button" apply control (Easy-Apply / SPA, no native submit)', () => {
    setBody(`<div role="button">Submit application</div>`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    document
      .querySelector('[role="button"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }));

    expect(post).toHaveBeenCalledTimes(1);
  });

  it('posts on an input[type=submit] whose value matches', () => {
    setBody(`<input type="submit" value="Finish" />`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    document.querySelector('input')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));

    expect(post).toHaveBeenCalledTimes(1);
  });

  it('resolves the control when the click lands on a child element', () => {
    setBody(
      `<form>${APPLICATION_FIELDS}<button type="submit"><span>Apply</span> now</button></form>`
    );
    const post = vi.fn();
    armSubmitWatch(document, post);

    document.querySelector('span')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));

    expect(post).toHaveBeenCalledTimes(1);
  });

  it('does NOT fire on a non-apply submit button (e.g. "Save draft")', () => {
    // Formless, so only the click heuristic is in play — a submit button inside
    // a form implicitly submits it, which is a separate (and legitimate) signal.
    setBody(`<button type="submit">Save draft</button>`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    document.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));

    expect(post).not.toHaveBeenCalled();
  });

  it('does NOT fire on a hidden (display:none) apply button — computed-style only', () => {
    // The text matches, so `isHidden` is the only thing keeping this quiet.
    setBody(`<button type="submit" style="display:none">Submit application</button>`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    document.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));

    expect(post).not.toHaveBeenCalled();
  });
});

describe('armSubmitWatch — hidden résumé file input (#786 follow-up)', () => {
  it('recognizes a form whose résumé file input is hidden behind a custom upload button', () => {
    // A styled upload widget hides the native <input type=file> (display:none).
    // With only one OTHER visible field the form falls below the 3-field bar, so
    // without treating the hidden résumé input as decisive the real application
    // form would go unrecognized. Visibility is computed-style only (isHidden).
    setBody(`
      <form id="f">
        <input name="first_name" />
        <input type="file" name="resume" style="display:none" />
        <button type="submit">Submit application</button>
      </form>
    `);
    const post = vi.fn();
    armSubmitWatch(document, post);

    (document.getElementById('f') as HTMLFormElement).dispatchEvent(
      new Event('submit', { bubbles: true, cancelable: true })
    );

    expect(post).toHaveBeenCalledTimes(1);
  });

  it('still ignores a hidden NON-résumé file input with too few visible fields', () => {
    // The narrowing: only a résumé/CV-flavored hidden file input is decisive, so
    // a bare hidden upload on a non-application form stays below the bar and the
    // module keeps under-reporting rather than over-reporting.
    setBody(`
      <form id="f">
        <input name="q" />
        <input type="file" style="display:none" />
        <button type="submit">Submit application</button>
      </form>
    `);
    const post = vi.fn();
    armSubmitWatch(document, post);

    (document.getElementById('f') as HTMLFormElement).dispatchEvent(
      new Event('submit', { bubbles: true, cancelable: true })
    );

    expect(post).not.toHaveBeenCalled();
  });
});

describe('armSubmitWatch — non-application forms (#786 lows)', () => {
  it('does NOT treat a checkbox/radio-only form as an application form', () => {
    // A filter / cookie-consent / survey widget is built only from checkboxes or
    // radios; a real application form clears the bar on its text/email/résumé
    // fields, so these are excluded from the fillable-field count.
    setBody(`
      <form id="f">
        <input type="checkbox" name="a" />
        <input type="checkbox" name="b" />
        <input type="radio" name="c" value="1" />
        <input type="radio" name="c" value="2" />
        <button type="submit">Submit application</button>
      </form>
    `);
    const post = vi.fn();
    armSubmitWatch(document, post);

    (document.getElementById('f') as HTMLFormElement).dispatchEvent(
      new Event('submit', { bubbles: true, cancelable: true })
    );

    expect(post).not.toHaveBeenCalled();
  });

  it('does NOT fire when a "Save draft" button submits the application form', () => {
    // Real browsers carry the pressed button as the SubmitEvent's `submitter`;
    // a draft-save submit must not auto-advance the application to `applied`.
    setBody(`
      <form id="f">
        ${APPLICATION_FIELDS}
        <button id="draft" type="submit">Save draft</button>
        <button id="send" type="submit">Submit application</button>
      </form>
    `);
    const post = vi.fn();
    armSubmitWatch(document, post);

    (document.getElementById('f') as HTMLFormElement).dispatchEvent(
      new SubmitEvent('submit', {
        submitter: document.getElementById('draft'),
        bubbles: true,
        cancelable: true,
      })
    );

    expect(post).not.toHaveBeenCalled();
  });

  it('still fires when the real submit button sends the application form', () => {
    setBody(`
      <form id="f">
        ${APPLICATION_FIELDS}
        <button id="draft" type="submit">Save draft</button>
        <button id="send" type="submit">Submit application</button>
      </form>
    `);
    const post = vi.fn();
    armSubmitWatch(document, post);

    (document.getElementById('f') as HTMLFormElement).dispatchEvent(
      new SubmitEvent('submit', {
        submitter: document.getElementById('send'),
        bubbles: true,
        cancelable: true,
      })
    );

    expect(post).toHaveBeenCalledTimes(1);
  });
});

describe('armSubmitWatch — fire-once guard', () => {
  it('posts AT MOST ONCE when the apply click AND its submit both fire', () => {
    setBody(`<form id="f">${APPLICATION_FIELDS}<button type="submit">Apply</button></form>`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    const button = document.querySelector('button')!;
    const form = document.getElementById('f') as HTMLFormElement;
    // A real click on the apply button, then the submit it triggers.
    button.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));

    expect(post).toHaveBeenCalledTimes(1);
  });
});
