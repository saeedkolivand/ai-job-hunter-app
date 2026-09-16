import { describe, expect, it } from 'vitest';

import { type FitBadgeView, renderFitBadge } from './fit-badge';
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
