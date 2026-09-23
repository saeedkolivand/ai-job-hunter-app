import { describe, expect, it, vi } from 'vitest';

import { type FitBadgeView, renderFitBadge, STALE_URL_POLL_MS } from './fit-badge';
import { NOTEBOOK_LIGHT } from './notebook-palette';

const VIEW: FitBadgeView = {
  score: 82,
  band: 'strong match',
  scoreLabel: 'keyword coverage',
  gaps: ['typescript', 'graphql', 'ci/cd'],
  applied: null,
};

describe('renderFitBadge', () => {
  it('renders inside a closed shadow root — the page cannot read the content', () => {
    document.body.innerHTML = '';
    renderFitBadge(document, NOTEBOOK_LIGHT, VIEW);
    const root = document.getElementById('ajh-fit-badge');
    expect(root).not.toBeNull();
    // Closed mode: `element.shadowRoot` is null to anyone but the caller
    // that received the return value — that's the whole point of the fix.
    expect(root!.shadowRoot).toBeNull();
    expect(root!.textContent).toBe('');
  });

  it('renders a pill with the score and band, collapsed by default', () => {
    document.body.innerHTML = '';
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, VIEW);
    expect(shadow.textContent).toContain('82%');
    expect(shadow.textContent).toContain('strong match');
    const card = shadow.querySelector('[hidden]');
    expect(card).not.toBeNull();
  });

  it('names the score-source qualifier in the pill aria-label, not buried behind a click', () => {
    document.body.innerHTML = '';
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, VIEW);
    const pill = shadow.querySelector('button')!;
    expect(pill.getAttribute('aria-label')).toContain('keyword coverage');
  });

  it('initializes aria-expanded false and flips it true on expand, false again on collapse, updating the label so it is never stale', () => {
    document.body.innerHTML = '';
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, VIEW);
    const pill = shadow.querySelector('button')!;

    expect(pill.getAttribute('aria-expanded')).toBe('false');
    expect(pill.getAttribute('aria-label')).toContain('Expand for details.');

    pill.click();
    expect(pill.getAttribute('aria-expanded')).toBe('true');
    // Stale-label regression: once expanded, the label must not still say
    // "Expand for details" (there is nothing left to expand).
    expect(pill.getAttribute('aria-label')).not.toContain('Expand for details.');
    expect(pill.getAttribute('aria-label')).toContain('Collapse the details.');

    pill.click();
    expect(pill.getAttribute('aria-expanded')).toBe('false');
    expect(pill.getAttribute('aria-label')).toContain('Expand for details.');
  });

  it('shows the score-source qualifier in the mini card, matching the panel', () => {
    document.body.innerHTML = '';
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, VIEW);
    (shadow.querySelector('button') as HTMLButtonElement).click();
    expect(shadow.textContent).toContain('keyword coverage');
  });

  it('expands the mini card on pill click and shows missing-keyword chips', () => {
    document.body.innerHTML = '';
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, VIEW);
    const pill = shadow.querySelector('button')!;
    (pill as HTMLButtonElement).click();
    // The mini card itself is a DIRECT child of the shadow root — filtered
    // from `shadow.children` rather than `querySelectorAll('div')[1]`, which
    // would instead pick the nested missing-keyword chips container
    // (appended INSIDE the card once populated), whose `hidden` is always
    // `false` — that selector could never fail this assertion even if
    // expansion broke.
    const card = [...shadow.children].find((el) => el.tagName === 'DIV') as
      HTMLDivElement | undefined;
    expect(card?.hidden).toBe(false);
    expect(card?.textContent).toContain('typescript');
  });

  it('does not populate the mini card until the pill is clicked (defense in depth)', () => {
    document.body.innerHTML = '';
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, {
      ...VIEW,
      salary: { posting: '€70,000–€90,000', expectation: '€80,000' },
    });
    expect(shadow.textContent).not.toContain('typescript');
    expect(shadow.textContent).not.toContain('Posting says');
    (shadow.querySelector('button') as HTMLButtonElement).click();
    expect(shadow.textContent).toContain('typescript');
    expect(shadow.textContent).toContain('Posting says');
  });

  it('shows the saved/applied chip in the pill label when present', () => {
    document.body.innerHTML = '';
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, { ...VIEW, applied: 'applied' });
    expect(shadow.textContent).toContain('Applied');
  });

  it('shows the salary facts line verbatim, never a verdict, only when present', () => {
    document.body.innerHTML = '';
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, {
      ...VIEW,
      salary: { posting: '€70,000–€90,000', expectation: '€80,000' },
    });
    (shadow.querySelector('button') as HTMLButtonElement).click();
    const card = [...shadow.children].find((el) => el.tagName === 'DIV') as
      HTMLDivElement | undefined;
    // The salary paragraph's COMPLETE text must equal exactly the two
    // factual strings joined — a `toContain` on the whole shadow root would
    // still pass if a comparative/verdict string were appended to this same
    // line (or anywhere else in the card), which is exactly what "never a
    // verdict" must catch.
    const salaryParagraph = [...(card?.querySelectorAll('p') ?? [])].find((p) =>
      p.textContent?.startsWith('Posting says')
    );
    expect(salaryParagraph?.textContent).toBe('Posting says €70,000–€90,000 · You want €80,000');
  });

  it('omits the salary line entirely when absent', () => {
    document.body.innerHTML = '';
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, VIEW);
    (shadow.querySelector('button') as HTMLButtonElement).click();
    expect(shadow.textContent).not.toContain('Posting says');
  });

  it('dismiss removes the badge from the page', () => {
    document.body.innerHTML = '';
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, VIEW);
    const dismiss = shadow.querySelector('button[aria-label="Dismiss the fit badge"]');
    (dismiss as HTMLButtonElement).click();
    expect(document.getElementById('ajh-fit-badge')).toBeNull();
  });

  it('is idempotent — a second render replaces the first rather than stacking', () => {
    document.body.innerHTML = '';
    renderFitBadge(document, NOTEBOOK_LIGHT, VIEW);
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, {
      ...VIEW,
      score: 40,
      band: 'low match',
    });
    expect(document.querySelectorAll('#ajh-fit-badge').length).toBe(1);
    expect(shadow.textContent).toContain('40%');
  });

  it('the view crosses a JSON round-trip unchanged (executeScript args boundary, PR2 lesson)', () => {
    const roundTripped = JSON.parse(JSON.stringify(VIEW)) as FitBadgeView;
    expect(roundTripped).toEqual(VIEW);
    document.body.innerHTML = '';
    // Must render identically from the round-tripped copy — proves nothing
    // in this view depends on a class instance / typed array / Map that a
    // JSON round trip would silently corrupt.
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, roundTripped);
    expect(shadow.textContent).toContain('82%');
  });
});

// ── staleness watcher (issue #1221) — the badge must clear itself when the
//    page moves to a DIFFERENT posting (an SPA job→job navigation), keyed off
//    job identity (location.href), never re-run on individual mutations.

describe('fit badge staleness watcher', () => {
  const postingUrl = 'https://jobs.linkedin.com/jobs/view/123';
  const otherUrl = 'https://jobs.linkedin.com/jobs/view/456';

  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('removes the badge the next poll tick after an SPA-style url change (pushState fires no event in any world)', () => {
    document.body.innerHTML = '';
    vi.stubGlobal('location', { href: postingUrl } as Location);
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, VIEW, postingUrl);
    expect(document.getElementById('ajh-fit-badge')).not.toBeNull();
    expect(shadow.textContent).toContain('82%');

    // pushState-style navigation: `location.href` changes, NO popstate/
    // hashchange fires (they never fire for pushState) — only the poll sees it.
    vi.stubGlobal('location', { href: otherUrl } as Location);
    vi.advanceTimersByTime(STALE_URL_POLL_MS);

    expect(document.getElementById('ajh-fit-badge')).toBeNull();
  });

  it('keeps the badge while the url stays unchanged across several poll ticks', () => {
    document.body.innerHTML = '';
    vi.stubGlobal('location', { href: postingUrl } as Location);
    renderFitBadge(document, NOTEBOOK_LIGHT, VIEW, postingUrl);
    expect(document.getElementById('ajh-fit-badge')).not.toBeNull();

    vi.advanceTimersByTime(STALE_URL_POLL_MS * 3);
    expect(document.getElementById('ajh-fit-badge')).not.toBeNull();
  });

  it('removes the badge on the immediate first check when the page already moved on', () => {
    // The background's pre-render in-page check and this render can straddle a
    // same-tick navigation — the already-stale badge must never be left up.
    document.body.innerHTML = '';
    vi.stubGlobal('location', { href: otherUrl } as Location);
    renderFitBadge(document, NOTEBOOK_LIGHT, VIEW, postingUrl);
    expect(document.getElementById('ajh-fit-badge')).toBeNull();
  });

  it('a popstate (back/forward) removes the badge without waiting for a poll tick', () => {
    document.body.innerHTML = '';
    vi.stubGlobal('location', { href: postingUrl } as Location);
    renderFitBadge(document, NOTEBOOK_LIGHT, VIEW, postingUrl);
    expect(document.getElementById('ajh-fit-badge')).not.toBeNull();

    vi.stubGlobal('location', { href: `${postingUrl}?tab=2` } as Location);
    window.dispatchEvent(new Event('popstate'));

    expect(document.getElementById('ajh-fit-badge')).toBeNull();
  });

  it('does not start a watcher when expectedUrl is omitted — a badge with no captured url stays put', () => {
    document.body.innerHTML = '';
    vi.stubGlobal('location', { href: postingUrl } as Location);
    renderFitBadge(document, NOTEBOOK_LIGHT, VIEW);
    expect(document.getElementById('ajh-fit-badge')).not.toBeNull();

    vi.stubGlobal('location', { href: otherUrl } as Location);
    vi.advanceTimersByTime(STALE_URL_POLL_MS * 3);
    expect(document.getElementById('ajh-fit-badge')).not.toBeNull();
  });

  it('dismiss stops the watcher — no interval or listener survives the badge it watched', () => {
    document.body.innerHTML = '';
    vi.stubGlobal('location', { href: postingUrl } as Location);
    const shadow = renderFitBadge(document, NOTEBOOK_LIGHT, VIEW, postingUrl);
    expect(vi.getTimerCount()).toBe(1);

    const dismiss = shadow.querySelector('button[aria-label="Dismiss the fit badge"]');
    (dismiss as HTMLButtonElement).click();

    // The tear-down disconnected the interval (and would the listeners).
    expect(vi.getTimerCount()).toBe(0);
    expect(document.getElementById('ajh-fit-badge')).toBeNull();
  });

  it('a re-render replaces the previous watcher rather than stacking intervals', () => {
    document.body.innerHTML = '';
    vi.stubGlobal('location', { href: postingUrl } as Location);
    renderFitBadge(document, NOTEBOOK_LIGHT, VIEW, postingUrl);
    expect(vi.getTimerCount()).toBe(1);

    renderFitBadge(document, NOTEBOOK_LIGHT, { ...VIEW, score: 40 }, postingUrl);

    expect(vi.getTimerCount()).toBe(1);
    expect(document.querySelectorAll('#ajh-fit-badge').length).toBe(1);
  });
});
