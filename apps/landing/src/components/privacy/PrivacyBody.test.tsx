// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { PrivacyBody } from './PrivacyBody';
import { LAST_UPDATED } from './sections/Footer';

afterEach(() => {
  cleanup();
});

// The six in-page anchor targets `public/scripts/privacy-0.js` and the
// `<h2 id>` `#` self-links jump to (ADR 0018) — must each exist exactly once.
const ANCHOR_IDS = ['short', 'extension', 'desktop', 'control', 'changes', 'contact'];

describe('PrivacyBody', () => {
  it('wraps main.wrap in a display:contents root div', () => {
    const { container } = render(<PrivacyBody />);
    const root = container.firstElementChild;
    expect(root?.tagName).toBe('DIV');
    expect((root as HTMLElement | null)?.style.display).toBe('contents');

    const main = container.querySelector('main.wrap');
    expect(main).not.toBeNull();
    expect(main?.parentElement).toBe(root);
  });

  it('renders every in-page anchor target id exactly once', () => {
    const { container } = render(<PrivacyBody />);
    for (const id of ANCHOR_IDS) {
      expect(container.querySelectorAll(`#${id}`)).toHaveLength(1);
    }
  });

  it('renders the h1 and the top back-link', () => {
    const { container } = render(<PrivacyBody />);
    expect(container.querySelector('h1')?.textContent).toBe('Privacy Policy');

    const back = container.querySelector('a.top-back');
    expect(back?.getAttribute('href')).toBe('/');
  });

  it('wires the footer with "privacy" as plain text and the other items as links', () => {
    const { container } = render(<PrivacyBody />);
    expect(container.querySelector('.byline')?.textContent).toBe(
      'made by Saeed, between rejections.'
    );

    const footLinks = container.querySelector('.foot-links');
    expect(footLinks?.textContent).toContain('privacy');

    const hrefs = Array.from(footLinks?.querySelectorAll('a') ?? []).map((a) =>
      a.getAttribute('href')
    );
    expect(hrefs).not.toContain('/privacy');
    expect(hrefs).toContain('/');
    expect(hrefs).toContain('/download');
  });

  // Regression guard: the crash-reporting rewrite (#927) changed what this page
  // says the app collects but left "Last updated: 30 June 2026" in place, so a
  // store-filed legal document claimed a review that predated the change. The
  // date now comes from one exported constant; this pins that the page actually
  // renders it, so a future edit cannot silently reintroduce a hardcoded date.
  //
  // What this canNOT check is the page's own promise — "if this policy changes,
  // we'll bump the date". Nothing mechanical ties a copy edit to a date bump;
  // that one stays on the author.
  it('renders the Last updated date from the shared constant, in the expected form', () => {
    const { container } = render(<PrivacyBody />);
    const updated = container.querySelector('.updated')?.textContent ?? '';

    expect(updated).toBe(`Last updated: ${LAST_UPDATED}`);
    expect(LAST_UPDATED, `"${LAST_UPDATED}" is not in "D Month YYYY" form`).toMatch(
      /^\d{1,2} (January|February|March|April|May|June|July|August|September|October|November|December) \d{4}$/
    );
  });
});
