// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, waitFor } from '@testing-library/react';

import { DownloadBody } from '@/components/download/DownloadBody';
import { buildInstallers } from '@/lib/version';

import { DownloadCounts } from './DownloadCounts';

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const COUNTS = {
  macArm: 21,
  macIntel: 5,
  winExe: 1200,
  winMsi: 1,
  linuxAppImage: 12,
  linuxDeb: 10,
  linuxRpm: 0,
};

// Dispatches on the requested URL, so the by-platform pills and the installs
// total (two independent fetches) can be stubbed differently in one test. A
// stub that ignores the `url` argument — most of the existing tests below —
// answers both endpoints identically, which is fine for cases that don't care.
function stubFetch(impl: (url: string) => unknown) {
  vi.stubGlobal(
    'fetch',
    vi.fn((url: string) => Promise.resolve(impl(url)))
  );
}

const ok = (body: unknown) => ({ ok: true, json: () => Promise.resolve(body) });
const notOk = { ok: false, json: () => Promise.reject(new Error('nope')) };

/** The real download page, so the pills are tested against the real buttons. */
function renderPage() {
  return render(
    <>
      <DownloadBody version="1.2.3" installers={buildInstallers('1.2.3')} />
      <DownloadCounts />
    </>
  );
}

describe('DownloadCounts', () => {
  it('puts each platform count on its own button, keyed by data-platform', async () => {
    stubFetch(() => ok(COUNTS));
    const { container } = renderPage();

    await waitFor(() => {
      expect(container.querySelectorAll('.dl-count')).toHaveLength(7);
    });

    for (const [platform, n] of Object.entries(COUNTS)) {
      const btn = container.querySelector(`.dl-btn[data-platform="${platform}"]`);
      expect(btn, `no button for ${platform}`).not.toBeNull();
      // The count belongs to THIS button — a positional bug would still produce
      // seven pills, so assert the pairing, not the tally.
      expect(btn?.querySelector('.dl-count')?.textContent).toContain(
        n === 1200 ? '1,200' : String(n)
      );
    }
  });

  it('speaks the unit, and gets its plural right', async () => {
    stubFetch(() => ok(COUNTS));
    const { container } = renderPage();
    await waitFor(() => expect(container.querySelectorAll('.dl-count')).toHaveLength(7));

    const unitFor = (p: string) =>
      container.querySelector(`.dl-btn[data-platform="${p}"] .dl-count .sr-only`)?.textContent;
    // Without this the link announces as "Intel · .dmg 5".
    expect(unitFor('macArm')).toBe(' downloads');
    expect(unitFor('winMsi')).toBe(' download');
    expect(unitFor('linuxRpm')).toBe(' downloads');

    // The visible number lives in its own node; the icon comes first, the
    // unit is for the reader only.
    expect(
      container.querySelector('.dl-btn[data-platform="macArm"] .dl-count-number')?.textContent
    ).toBe('21');
  });

  it('opens every dl-count pill with the download icon, GitHub and store alike', async () => {
    stubFetch((url) =>
      url.includes('store-counts')
        ? ok({ total: 12345, github: 12000, msStore: 25, snap: 14, chrome: 37, firefox: 2 })
        : ok(COUNTS)
    );
    const { container } = renderPage();

    // 7 GitHub pills + 4 store pills.
    await waitFor(() => expect(container.querySelectorAll('.dl-count')).toHaveLength(11));

    for (const pill of Array.from(container.querySelectorAll('.dl-count'))) {
      const icon = pill.querySelector('svg.dl-icon');
      expect(icon, `pill "${pill.textContent}" has no icon`).not.toBeNull();
      expect(icon?.getAttribute('aria-hidden')).toBe('true');
      // The icon must precede the number, not follow it.
      expect(icon?.nextElementSibling?.classList.contains('dl-count-number')).toBe(true);
    }
  });

  it("puts each store's count on its own button, keyed by data-store, naming its own unit", async () => {
    stubFetch((url) =>
      url.includes('store-counts')
        ? ok({ total: 78, github: 0, msStore: 25, snap: 14, chrome: 37, firefox: 2 })
        : ok({})
    );
    const { container } = renderPage();

    await waitFor(() =>
      expect(container.querySelectorAll('[data-store] .dl-count')).toHaveLength(4)
    );

    const badge = (store: string) => container.querySelector(`[data-store="${store}"] .dl-count`);

    expect(badge('msStore')?.querySelector('.dl-count-number')?.textContent).toBe('25');
    expect(badge('msStore')?.querySelector('.sr-only')?.textContent).toBe(' acquisitions');

    expect(badge('snap')?.querySelector('.dl-count-number')?.textContent).toBe('14');
    expect(badge('snap')?.querySelector('.sr-only')?.textContent).toBe(' installs');

    expect(badge('chrome')?.querySelector('.dl-count-number')?.textContent).toBe('37');
    expect(badge('chrome')?.querySelector('.sr-only')?.textContent).toBe(' users');

    expect(badge('firefox')?.querySelector('.dl-count-number')?.textContent).toBe('2');
    expect(badge('firefox')?.querySelector('.sr-only')?.textContent).toBe(' daily users');

    // Never call a store number a "download" — that unit belongs to GitHub only.
    for (const store of ['msStore', 'snap', 'chrome', 'firefox']) {
      expect(badge(store)?.querySelector('.sr-only')?.textContent).not.toMatch(/download/i);
    }
  });

  it('hides a store badge when its figure is null, missing, or zero — never stale or zero', async () => {
    stubFetch((url) =>
      url.includes('store-counts')
        ? ok({ total: 2, github: 0, msStore: null, snap: 0, chrome: 2 /* firefox absent */ })
        : ok({})
    );
    const { container } = renderPage();

    await waitFor(() =>
      expect(container.querySelector('[data-store="chrome"] .dl-count')).not.toBeNull()
    );
    expect(container.querySelector('[data-store="msStore"] .dl-count')).toBeNull();
    expect(container.querySelector('[data-store="snap"] .dl-count')).toBeNull();
    expect(container.querySelector('[data-store="firefox"] .dl-count')).toBeNull();
    expect(container.querySelectorAll('[data-store] .dl-count')).toHaveLength(1);
  });

  it('leaves a button alone when its platform is absent or not a number', async () => {
    stubFetch(() => ok({ macArm: 21, winExe: 'lots', linuxDeb: null }));
    const { container } = renderPage();
    await waitFor(() =>
      expect(container.querySelector('.dl-btn[data-platform="macArm"] .dl-count')).not.toBeNull()
    );
    expect(container.querySelectorAll('.dl-count')).toHaveLength(1);
  });

  it('does not double up when the effect runs twice over the same buttons', async () => {
    stubFetch(() => ok(COUNTS));
    // Two mounted instances, ONE set of buttons — the effect body runs twice
    // against the same nodes, which is what StrictMode does in dev.
    //
    // Deliberately not written as a rerender(): that re-renders DownloadBody
    // too, replacing the button nodes, so the second pass decorates fresh
    // buttons and the count comes back to 7 whether the guard exists or not.
    // It passed with the guard deleted.
    const { container } = render(
      <>
        <DownloadBody version="1.2.3" installers={buildInstallers('1.2.3')} />
        <DownloadCounts />
        <DownloadCounts />
      </>
    );

    await waitFor(() => expect(container.querySelectorAll('.dl-count')).toHaveLength(7));
    expect(container.querySelectorAll('.dl-btn[data-platform="winExe"] .dl-count')).toHaveLength(1);
  });

  it('stays silent when the counts file is missing, and never touches the buttons', async () => {
    stubFetch(() => notOk);
    const { container } = renderPage();

    await waitFor(() => expect(container.querySelectorAll('a.dl-btn')).toHaveLength(7));
    expect(container.querySelectorAll('.dl-count')).toHaveLength(0);
    // The download links are the point of the page; a missing count may not
    // cost anyone one of them.
    for (const a of Array.from(container.querySelectorAll('a.dl-btn'))) {
      expect(a.getAttribute('href')).toMatch(/^https:\/\/github\.com\//);
    }
  });

  it('survives fetch rejecting outright', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.reject(new Error('offline')))
    );
    const { container } = renderPage();
    await waitFor(() => expect(container.querySelectorAll('a.dl-btn')).toHaveLength(7));
    expect(container.querySelectorAll('.dl-count')).toHaveLength(0);
  });

  it('renders the installs total and un-hides it, formatted', async () => {
    stubFetch((url) =>
      url.includes('store-counts')
        ? ok({ total: 12345, github: 12000, msStore: 200, snap: 100, chrome: 40, firefox: 5 })
        : ok(COUNTS)
    );
    const { container } = renderPage();

    const el = () => container.querySelector<HTMLElement>('[data-installs-total]');
    await waitFor(() => expect(el()?.hidden).toBe(false));
    expect(el()?.textContent).toBe(
      '12,345 installs so far, counting GitHub downloads and the app and extension stores.'
    );
  });

  it('keeps the installs total hidden when store-counts.json 404s, without touching the pills', async () => {
    stubFetch((url) => (url.includes('store-counts') ? notOk : ok(COUNTS)));
    const { container } = renderPage();

    await waitFor(() => expect(container.querySelectorAll('.dl-count')).toHaveLength(7));
    expect(container.querySelector<HTMLElement>('[data-installs-total]')?.hidden).toBe(true);
  });

  it('renders the installs total and store badges when downloads-by-platform.json 404s, without any GitHub pills', async () => {
    stubFetch((url) =>
      url.includes('store-counts')
        ? ok({ total: 999, github: 900, msStore: 50, snap: 30, chrome: 15, firefox: 4 })
        : notOk
    );
    const { container } = renderPage();

    const el = () => container.querySelector<HTMLElement>('[data-installs-total]');
    await waitFor(() => expect(el()?.hidden).toBe(false));
    expect(el()?.textContent).toContain('999 installs so far');
    // The two fetches are independent: a GitHub-side 404 must not cost the
    // store badges, which come from the store-counts fetch alone.
    expect(container.querySelectorAll('.dl-btn .dl-count')).toHaveLength(0);
    expect(container.querySelectorAll('[data-store] .dl-count')).toHaveLength(4);
  });

  it('keeps the installs total hidden when `total` is malformed', async () => {
    stubFetch((url) => (url.includes('store-counts') ? ok({ total: 'lots' }) : ok(COUNTS)));
    const { container } = renderPage();

    await waitFor(() => expect(container.querySelectorAll('.dl-count')).toHaveLength(7));
    expect(container.querySelector<HTMLElement>('[data-installs-total]')?.hidden).toBe(true);
  });
});
