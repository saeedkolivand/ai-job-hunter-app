// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { AccessibilityBody } from './AccessibilityBody';

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
});
