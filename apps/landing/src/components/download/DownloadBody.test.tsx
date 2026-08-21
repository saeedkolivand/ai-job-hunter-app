// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { CHROME_EXT, FIREFOX_EXT, KOFI, PAYPAL, SPONSOR } from '@/lib/site-links';
import { buildInstallers } from '@/lib/version';

import { DownloadBody } from './DownloadBody';

afterEach(() => {
  cleanup();
});

const VERSION = '1.2.3';
const installers = buildInstallers(VERSION);

// DownloadFreshness swaps `.dl-btn` hrefs by `querySelectorAll` positional
// index, not by identifier — this order is the load-bearing contract
// (DownloadCards.tsx).
const DL_BTN_ORDER = [
  installers.macArm,
  installers.macIntel,
  installers.winExe,
  installers.winMsi,
  installers.linuxAppImage,
  installers.linuxDeb,
  installers.linuxRpm,
];

describe('DownloadBody', () => {
  it('renders exactly 7 dl-btn anchors in the DownloadFreshness positional order', () => {
    const { container } = render(<DownloadBody version={VERSION} installers={installers} />);
    const dlBtns = Array.from(container.querySelectorAll('a.dl-btn'));
    expect(dlBtns).toHaveLength(7);
    expect(dlBtns.map((a) => a.getAttribute('href'))).toEqual(DL_BTN_ORDER);
  });

  it('renders #downloads-block as a display:contents node inside .platforms', () => {
    const { container } = render(<DownloadBody version={VERSION} installers={installers} />);
    const block = container.querySelector('#downloads-block');
    expect(block).not.toBeNull();
    expect((block as HTMLElement | null)?.style.display).toBe('contents');
    expect(block?.parentElement?.classList.contains('platforms')).toBe(true);
  });

  it('renders the copy-cmd chips with the exact data-copy values, role=button, tabIndex 0', () => {
    const { container } = render(<DownloadBody version={VERSION} installers={installers} />);
    const chips = Array.from(container.querySelectorAll('.copy-cmd'));
    expect(chips).toHaveLength(5);

    const dataCopies = chips.map((el) => el.getAttribute('data-copy'));
    expect(dataCopies).toContain('xattr -cr "/Applications/AI Job Hunter.app"');
    expect(
      dataCopies.filter(
        (v) =>
          v ===
          'brew tap saeedkolivand/ai-job-hunter-app https://github.com/saeedkolivand/ai-job-hunter-app'
      )
    ).toHaveLength(2);
    expect(dataCopies.filter((v) => v === 'brew install --cask ai-job-hunter')).toHaveLength(2);

    for (const chip of chips) {
      expect(chip.getAttribute('role')).toBe('button');
      expect((chip as HTMLElement).tabIndex).toBe(0);
    }
  });

  it('renders the h1 and the top back-link', () => {
    const { container } = render(<DownloadBody version={VERSION} installers={installers} />);
    expect(container.querySelector('h1')?.textContent).toBe('Take the app');

    const back = container.querySelector('a.top-back');
    expect(back?.getAttribute('href')).toBe('/');
  });

  it('renders the ext-grid with 2 ext-btn anchors to the Chrome and Firefox store urls', () => {
    const { container } = render(<DownloadBody version={VERSION} installers={installers} />);
    const extBtns = Array.from(container.querySelectorAll('.ext-grid a.ext-btn'));
    expect(extBtns).toHaveLength(2);
    expect(extBtns.map((a) => a.getAttribute('href'))).toEqual([CHROME_EXT, FIREFOX_EXT]);

    for (const a of extBtns) {
      expect(a.getAttribute('target')).toBe('_blank');
      expect(a.getAttribute('rel')).toBe('noopener noreferrer');
    }
  });

  it('offers all three funding destinations, in order, above the extension section', () => {
    const { container } = render(<DownloadBody version={VERSION} installers={installers} />);
    const notes = Array.from(container.querySelectorAll('p.auto-note'));
    const tipJar = notes.find((p) => p.textContent?.includes('earns its keep'));
    expect(tipJar).toBeTruthy();

    const links = Array.from(tipJar?.querySelectorAll('a') ?? []);
    // Order is the owner's call, not incidental — assert it rather than membership.
    expect(links.map((a) => a.getAttribute('href'))).toEqual([SPONSOR, PAYPAL, KOFI]);
    expect(links.map((a) => a.textContent)).toEqual(['GitHub Sponsors', 'PayPal', 'Ko-fi']);

    for (const a of links) {
      expect(a.getAttribute('target')).toBe('_blank');
      expect(a.getAttribute('rel')).toBe('noopener noreferrer');
    }

    // Slot A: the ask sits before the extension section, not stacked on the
    // footer's own "♥ sponsor" link (DownloadBody.tsx's placement comment).
    const extGrid = container.querySelector('.ext-grid');
    expect(tipJar?.compareDocumentPosition(extGrid as Node)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
  });

  it('states the funding ask without a price or a positional reference', () => {
    const { container } = render(<DownloadBody version={VERSION} installers={installers} />);
    const tipJar = Array.from(container.querySelectorAll('p.auto-note')).find((p) =>
      p.textContent?.includes('earns its keep')
    );
    const text = tipJar?.textContent ?? '';

    // A committed price silently becomes a lie when certificate costs change —
    // the failure scripts/check-landing-drift.mjs exists for.
    expect(text).not.toMatch(/[$€£]\s?\d/);
    // WCAG 1.3.3: "the warning above" is meaningless in a linear screen-reader
    // read, and the warnings reflow anyway. Name the thing, never its position.
    expect(text).not.toMatch(/\b(above|below|to the (left|right))\b/i);
  });

  it('wires the footer with "download" as plain text', () => {
    const { container } = render(<DownloadBody version={VERSION} installers={installers} />);
    const footLinks = container.querySelector('.foot-links');
    expect(footLinks?.textContent).toContain('download');

    const hrefs = Array.from(footLinks?.querySelectorAll('a') ?? []).map((a) =>
      a.getAttribute('href')
    );
    expect(hrefs).not.toContain('/download');
  });
});
