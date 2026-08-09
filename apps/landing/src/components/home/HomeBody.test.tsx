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
});
