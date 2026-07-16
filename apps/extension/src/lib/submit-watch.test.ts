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

afterEach(() => {
  document.body.innerHTML = '';
});

describe('armSubmitWatch — real form submit', () => {
  it('posts once on a real form submit', () => {
    setBody(`<form id="f"><button type="submit">Submit application</button></form>`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    const form = document.getElementById('f') as HTMLFormElement;
    form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));

    expect(post).toHaveBeenCalledTimes(1);
    // The current page URL is posted (jsdom's default location).
    expect(typeof post.mock.calls[0]?.[0]).toBe('string');
  });

  it('OBSERVES ONLY — never preventDefault on the submit', () => {
    setBody(`<form id="f"><button type="submit">Apply</button></form>`);
    armSubmitWatch(document, vi.fn());

    const form = document.getElementById('f') as HTMLFormElement;
    const evt = new Event('submit', { bubbles: true, cancelable: true });
    const notCancelled = form.dispatchEvent(evt);

    expect(evt.defaultPrevented).toBe(false);
    expect(notCancelled).toBe(true);
  });
});

describe('armSubmitWatch — apply-style click heuristic', () => {
  it('posts on a click of an apply-style submit button', () => {
    setBody(`<button type="submit">Apply now</button>`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    document
      .querySelector('button')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));

    expect(post).toHaveBeenCalledTimes(1);
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
    setBody(`<button type="submit"><span>Apply</span> now</button>`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    document.querySelector('span')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));

    expect(post).toHaveBeenCalledTimes(1);
  });

  it('does NOT fire on a non-apply submit button (e.g. "Save draft")', () => {
    setBody(`<button type="submit">Save draft</button>`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    document.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));

    expect(post).not.toHaveBeenCalled();
  });

  it('does NOT fire on a hidden (display:none) apply button — computed-style only', () => {
    setBody(`<button type="submit" style="display:none">Apply</button>`);
    const post = vi.fn();
    armSubmitWatch(document, post);

    document.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));

    expect(post).not.toHaveBeenCalled();
  });
});

describe('armSubmitWatch — fire-once guard', () => {
  it('posts AT MOST ONCE when the apply click AND its submit both fire', () => {
    setBody(`<form id="f"><button type="submit">Apply</button></form>`);
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
