// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import sitemap from '@/app/sitemap';

import { HomeBody } from './HomeBody';

afterEach(() => {
  cleanup();
});

describe('HomeBody', () => {
  // / is the only page with real crawl equity, so a sitemap route linked *only*
  // from the doc-page SiteFooter sits one hop behind a page Google may not have
  // fetched yet. Search Console (Aug 2026) reported /agent-system,
  // /architecture-map and /tech-radar as "Discovered - currently not indexed"
  // with last-crawled N/A for exactly that reason — the same orphan class #934
  // fixed for /how-it-works. Home stays the entry point for every crawlable
  // route; where on the page the link lives is free to change.
  it('links every sitemap route', () => {
    const { container } = render(<HomeBody />);
    const hrefs = new Set(
      Array.from(container.querySelectorAll('a')).map((a) => a.getAttribute('href'))
    );

    const orphaned = sitemap()
      .map((entry) => new URL(entry.url).pathname)
      .filter((route) => route !== '/' && !hrefs.has(route));

    expect(orphaned).toEqual([]);
  });

  // The Growth section's two charts are same-origin static assets copied into
  // public/ at build time (pages.yml) — never a third-party fetch — and each
  // needs real alt text since they're the first <img> elements on the page.
  it('renders the growth charts as same-origin images with real alt text', () => {
    const { container } = render(<HomeBody />);
    const images = Array.from(container.querySelectorAll('.growth img'));

    expect(images).toHaveLength(2);
    for (const img of images) {
      const src = img.getAttribute('src') ?? '';
      expect(src.startsWith('http')).toBe(false);
      expect(src.startsWith('/')).toBe(true);
      expect((img.getAttribute('alt') ?? '').length).toBeGreaterThan(0);
    }
  });

  // .growth sits between .testi and .finale in document order (HomeBody's
  // own comment on why: the #journey overlay walks `main > section` and
  // relies on that placement).
  it('places the growth section between testimonials and finale', () => {
    const { container } = render(<HomeBody />);
    const classNames = Array.from(container.querySelectorAll('main > section')).map(
      (section) => section.className
    );
    const testiIndex = classNames.findIndex((c) => c.includes('testi'));
    const growthIndex = classNames.findIndex((c) => c.includes('growth'));
    const finaleIndex = classNames.findIndex((c) => c.includes('finale'));

    expect(testiIndex).toBeGreaterThanOrEqual(0);
    expect(growthIndex).toBeGreaterThan(testiIndex);
    expect(finaleIndex).toBeGreaterThan(growthIndex);
  });
});
