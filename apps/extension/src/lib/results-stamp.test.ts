import { describe, expect, it } from 'vitest';

import { NOTEBOOK_LIGHT } from './notebook-palette';
import {
  collectResultsCards,
  MAX_STAMP_CARDS,
  peekStampShadow,
  type StampInput,
  stampResultsCards,
} from './results-stamp';

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
