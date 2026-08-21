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
  it('renders the growth charts as same-origin images', () => {
    const { container } = render(<HomeBody />);
    const images = Array.from(container.querySelectorAll('.growth img'));

    expect(images).toHaveLength(2);
    for (const img of images) {
      const src = img.getAttribute('src') ?? '';
      expect(src.startsWith('http')).toBe(false);
      expect(src.startsWith('/')).toBe(true);
    }
  });

  // A non-empty alt is not the property worth asserting — `alt=" "` satisfies
  // it. What each alt has to do is name the quantity being plotted, and say
  // nothing about which way the line goes: these SVGs are rebuilt nightly from
  // live data this file never sees, so a directional word is a claim that turns
  // into a lie the first flat or falling week. Same class of guard as the
  // no-price assertion in DownloadBody.test.tsx.
  it('gives each chart an alt naming the metric, with no directional claim', () => {
    const { container } = render(<HomeBody />);
    const alts = Array.from(container.querySelectorAll('.growth img')).map(
      (img) => img.getAttribute('alt') ?? ''
    );

    expect(alts).toHaveLength(2);
    expect(alts.some((a) => /\bstars?\b/i.test(a))).toBe(true);
    expect(alts.some((a) => /\bdownloads?\b/i.test(a))).toBe(true);

    for (const alt of alts) {
      expect(alt.trim().length).toBeGreaterThan(20);
      expect(alt).toMatch(/\bchart\b/i);
      expect(alt).not.toMatch(
        /\b(climb\w*|ris\w+|grow\w+|soar\w*|surg\w+|upward\w*|increas\w+|fall\w+|declin\w+|drop\w+)\b/i
      );
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
