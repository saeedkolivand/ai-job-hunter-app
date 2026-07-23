// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { PrivacyBody } from './PrivacyBody';

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
});
