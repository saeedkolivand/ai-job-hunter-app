import { describe, expect, it, vi } from 'vitest';

import { MAX_APPLIED_CHECK_BATCH_URLS } from '@ajh/shared';

import { NOTEBOOK_LIGHT } from './notebook-palette';
import {
  collectResultsCards,
  MAX_STAMP_CARDS,
  peekStampShadow,
  type StampInput,
  stampResultsCards,
} from './results-stamp';

// The three-way cap (Rust `MAX_BATCH_URLS`, the shared TS
// `MAX_APPLIED_CHECK_BATCH_URLS`, and this collector's `MAX_STAMP_CARDS`) is
// documented to mirror exactly, with nothing that enforces it. This test
// pins the two copies THIS package owns; the Rust constant lives in another
// package's file and is out of scope here (see the shared schema's own
// `.max()` for the half of the parity that runs at validation time).
it('MAX_STAMP_CARDS mirrors the shared MAX_APPLIED_CHECK_BATCH_URLS cap exactly', () => {
  expect(MAX_STAMP_CARDS).toBe(MAX_APPLIED_CHECK_BATCH_URLS);
});

function card(href: string, text = 'A job'): string {
  return `<li><a href="${href}">${text}</a></li>`;
}

describe('collectResultsCards', () => {
  it('finds job-card links by a generic href pattern, never a board-specific selector', () => {
    document.body.innerHTML = `<ul>
      ${card('/jobs/123-engineer')}
      ${card('/careers/456')}
      ${card('/about-us')}
    </ul>`;
    const found = collectResultsCards(document);
    expect(found.map((c) => c.url)).toEqual([
      'http://localhost:3000/jobs/123-engineer',
      'http://localhost:3000/careers/456',
    ]);
  });

  it('matches a query-param job id shape too', () => {
    document.body.innerHTML = `<ul>${card('/view?jk=abc123')}</ul>`;
    const found = collectResultsCards(document);
    expect(found).toHaveLength(1);
  });

  it('dedupes repeated hrefs, keeping the first occurrence only', () => {
    document.body.innerHTML = `<ul>${card('/jobs/1')}${card('/jobs/1')}</ul>`;
    const found = collectResultsCards(document);
    expect(found).toHaveLength(1);
  });

  it('skips a hidden anchor', () => {
    // A <div>, not a nested <li> — HTML auto-closes a <li> when another <li>
    // opens inside it, which would silently un-nest the fixture and defeat
    // the assertion below without the collector itself doing anything wrong.
    document.body.innerHTML = `<div style="display:none">${card('/jobs/1')}</div>`;
    const found = collectResultsCards(document);
    expect(found).toHaveLength(0);
  });

  it('caps at MAX_STAMP_CARDS', () => {
    const links = Array.from({ length: MAX_STAMP_CARDS + 10 }, (_, i) => card(`/jobs/${i}`)).join(
      ''
    );
    document.body.innerHTML = `<ul>${links}</ul>`;
    const found = collectResultsCards(document);
    expect(found).toHaveLength(MAX_STAMP_CARDS);
  });

  it('assigns indices in document order, starting at 0', () => {
    document.body.innerHTML = `<ul>${card('/jobs/1')}${card('/jobs/2')}</ul>`;
    const found = collectResultsCards(document);
    expect(found.map((c) => c.index)).toEqual([0, 1]);
  });

  it('skips a cross-origin job-shaped href — never trust an attacker-controlled url', () => {
    document.body.innerHTML = `<ul>${card('https://evil.example.com/jobs/1')}</ul>`;
    const found = collectResultsCards(document);
    expect(found).toHaveLength(0);
  });
});

describe('stampResultsCards', () => {
  it('stamps only FOUND cards, at the same index collectResultsCards assigned', () => {
    document.body.innerHTML = `<ul>${card('/jobs/1')}${card('/jobs/2')}</ul>`;
    collectResultsCards(document);
    const results: StampInput[] = [
      { url: 'http://localhost:3000/jobs/1', found: true, status: 'saved' },
      { url: 'http://localhost:3000/jobs/2', found: false },
    ];
    const stamped = stampResultsCards(document, NOTEBOOK_LIGHT, results);
    expect(stamped).toBe(1);
    const hosts = document.querySelectorAll('[data-ajh-stamp]');
    expect(hosts).toHaveLength(1);
    const anchor = document.querySelectorAll('a')[0] as HTMLAnchorElement;
    expect(peekStampShadow(anchor)?.textContent).toContain('Saved');
  });

  it('shows "Applied" for an applied status', () => {
    document.body.innerHTML = `<ul>${card('/jobs/1')}</ul>`;
    collectResultsCards(document);
    stampResultsCards(document, NOTEBOOK_LIGHT, [
      { url: 'http://localhost:3000/jobs/1', found: true, status: 'applied' },
    ]);
    const anchor = document.querySelectorAll('a')[0] as HTMLAnchorElement;
    expect(peekStampShadow(anchor)?.textContent).toContain('Applied');
  });

  it('renders inside a closed shadow root — the page cannot read the outcome back', () => {
    document.body.innerHTML = `<ul>${card('/jobs/1')}</ul>`;
    collectResultsCards(document);
    stampResultsCards(document, NOTEBOOK_LIGHT, [
      { url: 'http://localhost:3000/jobs/1', found: true, status: 'saved' },
    ]);
    const host = document.querySelector('[data-ajh-stamp]') as HTMLElement;
    // Closed mode: `host.shadowRoot` is null to anyone but the caller that
    // received `peekStampShadow`'s return value — the light-DOM host itself
    // never carries the saved/applied text.
    expect(host.shadowRoot).toBeNull();
    expect(host.textContent).toBe('');
  });

  it('is idempotent — re-stamping the same anchor replaces, never duplicates', () => {
    document.body.innerHTML = `<ul>${card('/jobs/1')}</ul>`;
    collectResultsCards(document);
    const results: StampInput[] = [
      { url: 'http://localhost:3000/jobs/1', found: true, status: 'saved' },
    ];
    stampResultsCards(document, NOTEBOOK_LIGHT, results);
    stampResultsCards(document, NOTEBOOK_LIGHT, results);
    expect(document.querySelectorAll('[data-ajh-stamp]')).toHaveLength(1);
  });

  it('skips an out-of-range or unknown index rather than mis-stamping', () => {
    document.body.innerHTML = `<ul>${card('/jobs/1')}</ul>`;
    collectResultsCards(document);
    const stamped = stampResultsCards(document, NOTEBOOK_LIGHT, [
      { url: 'http://localhost:3000/jobs/1', found: true, status: 'saved' },
      { url: 'http://localhost:3000/jobs/999-nonexistent', found: true, status: 'saved' },
    ]);
    expect(stamped).toBe(1);
  });

  it('the results array crosses a JSON round-trip unchanged (executeScript args boundary, PR2 lesson)', () => {
    document.body.innerHTML = `<ul>${card('/jobs/1')}</ul>`;
    collectResultsCards(document);
    const results: StampInput[] = [
      { url: 'http://localhost:3000/jobs/1', found: true, status: 'saved' },
    ];
    const roundTripped = JSON.parse(JSON.stringify(results)) as StampInput[];
    expect(roundTripped).toEqual(results);
    const stamped = stampResultsCards(document, NOTEBOOK_LIGHT, roundTripped);
    expect(stamped).toBe(1);
  });

  it('a dismiss button on the stamp removes it', () => {
    document.body.innerHTML = `<ul>${card('/jobs/1')}</ul>`;
    collectResultsCards(document);
    stampResultsCards(document, NOTEBOOK_LIGHT, [
      { url: 'http://localhost:3000/jobs/1', found: true, status: 'saved' },
    ]);
    const anchor = document.querySelectorAll('a')[0] as HTMLAnchorElement;
    const dismiss = peekStampShadow(anchor)?.querySelector('button');
    (dismiss as HTMLButtonElement).click();
    expect(document.querySelectorAll('[data-ajh-stamp]')).toHaveLength(0);
  });
});

// ── #1220: dedup must survive a FRESH injection ──────────────────────────────
// The file is injected as a fresh classic script on EVERY Stamp click, so the
// module-level WeakMaps are empty each time while the DOM still holds the
// hosts earlier instances placed — the dedup guard therefore has to read the
// DOM, not the maps. `vi.resetModules()` re-evaluates the module (fresh,
// empty maps) while the jsdom document keeps whatever was stamped before it,
// which is exactly the real injection shape.

describe('stamp dedup survives a fresh injection (#1220)', () => {
  const STAMP: StampInput = { url: 'http://localhost:3000/jobs/1', found: true, status: 'saved' };

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('a re-stamp from a FRESH injected instance replaces the previous host instead of appending another (empty WeakMaps, DOM already stamped)', async () => {
    document.body.innerHTML = `<ul>${card('/jobs/1')}</ul>`;
    // First injection: collect + stamp normally (the WeakMap works in-instance).
    const first = await import('./results-stamp');
    first.collectResultsCards(document);
    first.stampResultsCards(document, NOTEBOOK_LIGHT, [STAMP]);
    expect(document.querySelectorAll('[data-ajh-stamp]')).toHaveLength(1);

    // Second Stamp click = a fresh classic-script evaluation: module state is
    // gone (empty WeakMaps) but the host from the first injection is still on
    // the page. The DOM-based scan must find and replace it.
    vi.resetModules();
    const fresh = await import('./results-stamp');
    fresh.collectResultsCards(document);
    fresh.stampResultsCards(document, NOTEBOOK_LIGHT, [STAMP]);

    expect(document.querySelectorAll('[data-ajh-stamp]')).toHaveLength(1);
  });

  it('heals PRE-FIX accumulated duplicates (a contiguous run of stale hosts right after one anchor)', async () => {
    document.body.innerHTML = `<ul>${card('/jobs/1')}</ul>`;
    // Reconstruct the old #1220 state by hand: three hosts stacked after the
    // anchor, the exact light-DOM shape repeated placement produced.
    const anchor = document.querySelectorAll('a')[0] as HTMLAnchorElement;
    for (let i = 0; i < 3; i += 1) {
      const host = document.createElement('span');
      host.setAttribute('data-ajh-stamp', 'true');
      host.style.cssText = 'display:inline-flex;vertical-align:middle;margin-left:6px';
      anchor.insertAdjacentElement('afterend', host);
    }
    expect(document.querySelectorAll('[data-ajh-stamp]')).toHaveLength(3);

    vi.resetModules();
    const fresh = await import('./results-stamp');
    fresh.collectResultsCards(document);
    fresh.stampResultsCards(document, NOTEBOOK_LIGHT, [STAMP]);

    expect(document.querySelectorAll('[data-ajh-stamp]')).toHaveLength(1);
  });

  it('never removes a page-authored element carrying only the marker attribute (attacker-controlled page)', async () => {
    // A hostile page node with our marker but none of the host's style
    // signature sits right where our scan starts; it must survive unchanged.
    document.body.innerHTML = `<ul>${card('/jobs/1')}<span data-ajh-stamp="true">page decor</span></ul>`;

    vi.resetModules();
    const fresh = await import('./results-stamp');
    fresh.collectResultsCards(document);
    fresh.stampResultsCards(document, NOTEBOOK_LIGHT, [STAMP]);

    // Our own host + the page node both remain — nothing arbitrary was removed.
    expect(document.querySelectorAll('[data-ajh-stamp]')).toHaveLength(2);
    // The host carries the signature inline-style attribute and renders its
    // content in a closed shadow root (textContent ''), so target the page
    // node by the absence of that style — the exact thing that kept it alive.
    const decor = document.querySelector('span[data-ajh-stamp]:not([style])') as HTMLElement;
    expect(decor.textContent).toBe('page decor');
  });

  it("never sweeps up an ADJACENT card's stamp when clearing this card's (scan stops at the first non-stamp sibling)", async () => {
    // Cards whose anchor wraps the card directly: `afterend` puts each host
    // between anchors, so anchor1's sibling chain is [host1, anchor2, host2].
    document.body.innerHTML = `<a href="/jobs/1">Card one</a><a href="/jobs/2">Card two</a>`;
    vi.resetModules();
    const fresh = await import('./results-stamp');
    fresh.collectResultsCards(document);
    const results: StampInput[] = [
      { url: 'http://localhost:3000/jobs/1', found: true, status: 'saved' },
      { url: 'http://localhost:3000/jobs/2', found: true, status: 'saved' },
    ];
    fresh.stampResultsCards(document, NOTEBOOK_LIGHT, results);
    // Re-run ONLY card one from a fresh instance: card two's stamp must stay.
    vi.resetModules();
    const again = await import('./results-stamp');
    again.collectResultsCards(document);
    again.stampResultsCards(document, NOTEBOOK_LIGHT, [STAMP]);

    expect(document.querySelectorAll('[data-ajh-stamp]')).toHaveLength(2);
  });
});
