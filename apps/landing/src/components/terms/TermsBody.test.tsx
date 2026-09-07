// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { LAST_UPDATED, TermsBody } from './TermsBody';

afterEach(() => {
  cleanup();
});

// The seven in-page anchor targets the `<h2 id>` `#` self-links jump to — must
// each exist exactly once, mirroring components/privacy/PrivacyBody.test.tsx.
const ANCHOR_IDS = [
  'as-is',
  'your-output',
  'third-parties',
  'your-data',
  'no-account',
  'changes',
  'contact',
];

describe('TermsBody', () => {
  it('wraps main.wrap in a display:contents root div', () => {
    const { container } = render(<TermsBody />);
    const root = container.firstElementChild;
    expect(root?.tagName).toBe('DIV');
    expect((root as HTMLElement | null)?.style.display).toBe('contents');

    const main = container.querySelector('main.wrap');
    expect(main).not.toBeNull();
    expect(main?.parentElement).toBe(root);
  });

  it('renders every in-page anchor target id exactly once', () => {
    const { container } = render(<TermsBody />);
    for (const id of ANCHOR_IDS) {
      expect(container.querySelectorAll(`#${id}`)).toHaveLength(1);
    }
  });

  // Reverse direction of the check above, derived from the DOM rather than a
  // second hand-maintained list: a typo'd `#` self-link renders a dead anchor
  // without ever failing the "ids exist" check alone.
  it('every h2 anchor self-link resolves to a rendered heading id', () => {
    const { container } = render(<TermsBody />);
    const headingIds = Array.from(container.querySelectorAll('h2[id]')).map((h2) => h2.id);
    const anchorTargets = Array.from(container.querySelectorAll('h2 a.anchor[href^="#"]')).map(
      (a) => a.getAttribute('href')?.slice(1)
    );

    expect(anchorTargets).toHaveLength(headingIds.length);
    expect(new Set(anchorTargets)).toEqual(new Set(headingIds));
  });

  it('renders exactly one h1 and the top back-link', () => {
    const { container } = render(<TermsBody />);
    expect(container.querySelectorAll('h1')).toHaveLength(1);
    expect(container.querySelector('h1')?.textContent).toBe('Terms of Use');

    const back = container.querySelector('a.top-back');
    expect(back?.getAttribute('href')).toBe('/');
  });

  it('links the privacy policy and a contact mailto, and wires the footer with "terms" as plain text', () => {
    const { container } = render(<TermsBody />);
    expect(container.querySelector('main.wrap > p a[href="/privacy"]')).not.toBeNull();
    expect(container.querySelector('a[href="mailto:contact@aijobhunter.app"]')).not.toBeNull();

    const footLinks = container.querySelector('.foot-links');
    expect(footLinks?.textContent).toContain('terms');

    const hrefs = Array.from(footLinks?.querySelectorAll('a') ?? []).map((a) =>
      a.getAttribute('href')
    );
    expect(hrefs).not.toContain('/terms');
    expect(hrefs).toContain('/privacy');
  });

  // Regression guard on the two load-bearing legal claims. Both are what makes
  // this page worth publishing at all: the licence's own warranty/liability
  // disclaimer (Apache-2.0 §§7-8, which the page promises not to narrow), and
  // the statement that the app never submits an application by itself — the
  // sentence the whole "you are responsible for what you send" section rests
  // on. A copy edit that drops either changes what the page promises, so it
  // has to fail here rather than ship quietly.
  it('keeps the licence disclaimer and the "never submits on its own" claim', () => {
    const { container } = render(<TermsBody />);
    const text = container.textContent ?? '';

    expect(text).toContain('Apache License 2.0');
    expect(text).toContain('without warranty of any kind');
    expect(text).toContain('never submits an application on its own');
  });

  // Mirrors PrivacyBody.test.tsx: the date is one exported constant, so a
  // future edit cannot reintroduce a hardcoded date that drifts from it.
  it('renders the Last updated date from the shared constant, in the expected form', () => {
    const { container } = render(<TermsBody />);
    const updated = container.querySelector('.updated')?.textContent ?? '';

    expect(updated).toBe(`Last updated: ${LAST_UPDATED}`);
    expect(LAST_UPDATED, `"${LAST_UPDATED}" is not in "D Month YYYY" form`).toMatch(
      /^\d{1,2} (January|February|March|April|May|June|July|August|September|October|November|December) \d{4}$/
    );
  });
});
