// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { AccessibilityBody } from './AccessibilityBody';
import { LAST_UPDATED } from './sections/Footer';

afterEach(() => {
  cleanup();
});

// The nine in-page anchor targets the `<h2 id>` `#` self-links jump to — must
// each exist exactly once, mirroring components/privacy/PrivacyBody.test.tsx.
const ANCHOR_IDS = [
  'commitment',
  'scope',
  'standard',
  'conformance',
  'barriers',
  'in-place',
  'assessment',
  'feedback',
  'review',
];

describe('AccessibilityBody', () => {
  it('wraps main.wrap in a display:contents root div', () => {
    const { container } = render(<AccessibilityBody />);
    const root = container.firstElementChild;
    expect(root?.tagName).toBe('DIV');
    expect((root as HTMLElement | null)?.style.display).toBe('contents');

    const main = container.querySelector('main.wrap');
    expect(main).not.toBeNull();
    expect(main?.parentElement).toBe(root);
  });

  it('renders every in-page anchor target id exactly once', () => {
    const { container } = render(<AccessibilityBody />);
    for (const id of ANCHOR_IDS) {
      expect(container.querySelectorAll(`#${id}`)).toHaveLength(1);
    }
  });

  // Reverse direction of the check above, derived from the DOM rather than a
  // second hand-maintained list: every `<h2><a class="anchor" href="#x">` must
  // point at an id that's actually rendered on an `<h2>` — a typo'd href would
  // render a dead self-link without ever failing the "ids exist" check alone.
  it('every h2 anchor self-link resolves to a rendered heading id', () => {
    const { container } = render(<AccessibilityBody />);
    const headingIds = Array.from(container.querySelectorAll('h2[id]')).map((h2) => h2.id);
    const anchorTargets = Array.from(container.querySelectorAll('h2 a.anchor[href^="#"]')).map(
      (a) => a.getAttribute('href')?.slice(1)
    );

    expect(anchorTargets).toHaveLength(headingIds.length);
    expect(new Set(anchorTargets)).toEqual(new Set(headingIds));
  });

  it('renders exactly one h1 and the top back-link', () => {
    const { container } = render(<AccessibilityBody />);
    expect(container.querySelectorAll('h1')).toHaveLength(1);
    expect(container.querySelector('h1')?.textContent).toBe('Accessibility Statement');

    const back = container.querySelector('a.top-back');
    expect(back?.getAttribute('href')).toBe('/');
  });

  it('provides a contact mailto and wires the footer with "accessibility" as plain text', () => {
    const { container } = render(<AccessibilityBody />);
    const mailto = container.querySelector('a[href="mailto:contact@aijobhunter.app"]');
    expect(mailto).not.toBeNull();

    const footLinks = container.querySelector('.foot-links');
    expect(footLinks?.textContent).toContain('accessibility');

    const hrefs = Array.from(footLinks?.querySelectorAll('a') ?? []).map((a) =>
      a.getAttribute('href')
    );
    expect(hrefs).not.toContain('/accessibility');
    expect(hrefs).toContain('/privacy');
  });

  // Regression guard: this statement must keep saying "partially conformant"
  // for the landing site and the desktop app — a future edit that silently
  // upgrades the claim to "fully conformant" without new evidence must fail
  // this test.
  it('keeps the conformance section wording at "partially conformant"', () => {
    const { container } = render(<AccessibilityBody />);
    const conformanceHeading = container.querySelector('#conformance');

    // Walk forward through following siblings up to (not including) the next
    // <h2> to grab just this section's own body text.
    let text = '';
    let node = conformanceHeading?.nextElementSibling ?? null;
    while (node && node.tagName !== 'H2') {
      text += node.textContent ?? '';
      node = node.nextElementSibling;
    }

    expect(text).toContain('Partially conformant');
    expect(text.toLowerCase()).not.toContain('fully conformant');
  });

  // Regression guard: no wording anywhere on the page — not just the
  // conformance section — may upgrade the claim to "fully conformant". Zero
  // automated violations after the 1 August 2026 fix round is not the same
  // as WCAG conformance, and this must stay true even if other sections grow.
  it('never claims "fully conformant" anywhere on the page', () => {
    const { container } = render(<AccessibilityBody />);
    expect((container.textContent ?? '').toLowerCase()).not.toContain('fully conformant');
  });

  // Regression guard: the desktop contrast fix only covers the home view —
  // most of the renderer's low-alpha text-foreground usages are unaudited.
  // A future edit must not silently drop this scope disclosure while the
  // "0 violations" home-view claim stays on the page.
  it('discloses that the desktop contrast fix is scoped to the home view', () => {
    const { container } = render(<AccessibilityBody />);
    const text = container.textContent ?? '';

    expect(text).toContain('scoped to the home view');
    expect(text).toContain('1,100');
  });
});

// Mirrors scripts/bump-last-updated.mjs, which only bumps a "Last updated:"
// header in .md files and can't reach this JSX constant — this is what makes
// the "review cadence" section's annual-review promise mechanical instead of
// something someone has to remember.
const MONTHS = [
  'January',
  'February',
  'March',
  'April',
  'May',
  'June',
  'July',
  'August',
  'September',
  'October',
  'November',
  'December',
];

function parseStatementDate(display: string): Date {
  const match = display.match(/^(\d{1,2}) (\w+) (\d{4})$/);
  const day = match?.[1];
  const monthName = match?.[2];
  const year = match?.[3];
  const monthIndex = monthName ? MONTHS.indexOf(monthName) : -1;
  if (!day || !year || monthIndex === -1) {
    throw new Error(`LAST_UPDATED "${display}" isn't in the expected "D Month YYYY" form.`);
  }
  return new Date(Number(year), monthIndex, Number(day));
}

describe('LAST_UPDATED', () => {
  it('was reviewed within the last 12 months, per the review-cadence promise', () => {
    const reviewedAt = parseStatementDate(LAST_UPDATED);
    const cutoff = new Date();
    cutoff.setFullYear(cutoff.getFullYear() - 1);

    expect(
      reviewedAt >= cutoff,
      `The accessibility statement's LAST_UPDATED ("${LAST_UPDATED}") is over 12 months old — ` +
        "re-review this page's claims against the current app and site, then bump LAST_UPDATED " +
        'in apps/landing/src/components/accessibility/sections/Footer.tsx to today.'
    ).toBe(true);
  });
});
