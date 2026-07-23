// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, waitFor } from '@testing-library/react';

import { DownloadCards } from '@/components/download/DownloadCards';
import { buildInstallers } from '@/lib/version';

import { DownloadFreshness } from './DownloadFreshness';

const BAKED = '0.127.0';

// Renders the real production component (not a hand-copied fixture), so this
// test can't drift from the markup DownloadFreshness reads.
function mountDownloadsBlock(version: string): void {
  render(<DownloadCards version={version} installers={buildInstallers(version)} />);
}

function dlBtnHrefs(): string[] {
  return Array.from(document.querySelectorAll<HTMLAnchorElement>('#downloads-block .dl-btn')).map(
    (a) => a.getAttribute('href') ?? ''
  );
}

function versionLabel(): string | null {
  return document.querySelector('#downloads-block .dl-version b')?.textContent ?? null;
}

function mockFetch(impl: () => Promise<Pick<Response, 'ok' | 'json'>>): void {
  vi.stubGlobal('fetch', vi.fn(impl));
}

// The effect body has no observable completion signal beyond DOM mutation, so
// negative-space (no-op) assertions flush the microtask queue before checking
// the DOM held steady.
const flush = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

afterEach(() => {
  cleanup();
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
});

describe('DownloadFreshness', () => {
  it('is a silent no-op on a non-ok response', async () => {
    mountDownloadsBlock(BAKED);
    const before = dlBtnHrefs();
    mockFetch(() => Promise.resolve({ ok: false, json: () => Promise.resolve({}) }));

    render(<DownloadFreshness baked={BAKED} />);
    await flush();

    expect(dlBtnHrefs()).toEqual(before);
    expect(versionLabel()).toBe(`v${BAKED}`);
  });

  it('is a silent no-op on a network rejection', async () => {
    mountDownloadsBlock(BAKED);
    const before = dlBtnHrefs();
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));

    render(<DownloadFreshness baked={BAKED} />);
    await flush();

    expect(dlBtnHrefs()).toEqual(before);
  });

  it('is a silent no-op on malformed JSON', async () => {
    mountDownloadsBlock(BAKED);
    const before = dlBtnHrefs();
    mockFetch(() =>
      Promise.resolve({ ok: true, json: () => Promise.reject(new SyntaxError('bad json')) })
    );

    render(<DownloadFreshness baked={BAKED} />);
    await flush();

    expect(dlBtnHrefs()).toEqual(before);
  });

  it('is a silent no-op when tag_name is missing', async () => {
    mountDownloadsBlock(BAKED);
    const before = dlBtnHrefs();
    mockFetch(() => Promise.resolve({ ok: true, json: () => Promise.resolve({}) }));

    render(<DownloadFreshness baked={BAKED} />);
    await flush();

    expect(dlBtnHrefs()).toEqual(before);
    expect(versionLabel()).toBe(`v${BAKED}`);
  });

  it('is a silent no-op when the remote version equals the baked version', async () => {
    mountDownloadsBlock(BAKED);
    const before = dlBtnHrefs();
    mockFetch(() =>
      Promise.resolve({ ok: true, json: () => Promise.resolve({ tag_name: `v${BAKED}` }) })
    );

    render(<DownloadFreshness baked={BAKED} />);
    await flush();

    expect(dlBtnHrefs()).toEqual(before);
    expect(versionLabel()).toBe(`v${BAKED}`);
  });

  it('is a silent no-op when the remote version is older than the baked version', async () => {
    mountDownloadsBlock(BAKED);
    const before = dlBtnHrefs();
    mockFetch(() =>
      Promise.resolve({ ok: true, json: () => Promise.resolve({ tag_name: 'v0.126.0' }) })
    );

    render(<DownloadFreshness baked={BAKED} />);
    await flush();

    expect(dlBtnHrefs()).toEqual(before);
  });

  it('swaps the version label and every dl-btn href, positionally, when the remote is newer', async () => {
    const remote = '0.128.0';
    mountDownloadsBlock(BAKED);
    mockFetch(() =>
      Promise.resolve({ ok: true, json: () => Promise.resolve({ tag_name: `v${remote}` }) })
    );

    render(<DownloadFreshness baked={BAKED} />);
    await waitFor(() => expect(versionLabel()).toBe(`v${remote}`));

    // Positional order per DownloadFreshness.tsx / downloads.ts markup:
    // macArm, macIntel, winExe, winMsi, linuxAppImage, linuxDeb, linuxRpm.
    const expected = buildInstallers(remote);
    expect(dlBtnHrefs()).toEqual([
      expected.macArm,
      expected.macIntel,
      expected.winExe,
      expected.winMsi,
      expected.linuxAppImage,
      expected.linuxDeb,
      expected.linuxRpm,
    ]);
  });

  it('never lets a hostile tag_name escape the hardcoded github repo host, even when it parses as newer', async () => {
    // Non-numeric junk alone (e.g. plain "javascript:alert(1)//") parses to the
    // semver triple [0,0,0] and can never beat a real baked version, so it is
    // already covered by the no-op cases above. Prefixing it with a version
    // that IS numerically newer forces the mutation branch to run with hostile
    // content flowing into `remote` — the real path an injection would need.
    const hostileTag = '999.0.0-javascript:alert(1)//';
    mountDownloadsBlock(BAKED);
    const bakedFirstHref = dlBtnHrefs()[0];
    // Resolves on a macrotask (not a same-tick microtask) so this fails loudly
    // if the sync ever regresses to a tautological pre-mutation check.
    vi.stubGlobal(
      'fetch',
      vi.fn(
        () =>
          new Promise<Pick<Response, 'ok' | 'json'>>((resolve) => {
            setTimeout(
              () => resolve({ ok: true, json: () => Promise.resolve({ tag_name: hostileTag }) }),
              20
            );
          })
      )
    );

    render(<DownloadFreshness baked={BAKED} />);
    // Gate on the mutation actually having happened, not on the anchors merely
    // existing (they exist pre-render too, which would make this a no-op-path
    // tautology run against the still-baked, still-https-prefixed hrefs).
    await waitFor(() => expect(dlBtnHrefs()[0]).not.toBe(bakedFirstHref));

    for (const href of dlBtnHrefs()) {
      expect(href.startsWith('https://github.com/')).toBe(true);
    }
  });
});
