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

function stubFetch(impl: () => unknown) {
  vi.stubGlobal(
    'fetch',
    vi.fn(() => Promise.resolve(impl()))
  );
}

const ok = (body: unknown) => ({ ok: true, json: () => Promise.resolve(body) });

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

    // The visible pill must carry only the number; the unit is for the reader.
    expect(
      container.querySelector('.dl-btn[data-platform="macArm"] .dl-count')?.firstChild?.textContent
    ).toBe('21');
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
    stubFetch(() => ({ ok: false, json: () => Promise.reject(new Error('nope')) }));
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
});
