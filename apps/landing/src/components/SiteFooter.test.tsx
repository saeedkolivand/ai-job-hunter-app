// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { CHROME_EXT, FIREFOX_EXT, GITHUB_REPO, SPONSOR } from '@/lib/site-links';

import { SiteFooter } from './SiteFooter';

afterEach(() => {
  cleanup();
});

// The exact byte-for-byte footer text every marketing page shares (see
// how-it-works/privacy/download body.html <p class="foot-links">) — textContent
// is identical across variants since only the tag (a vs. plain text) differs
// for the "current" item, never the word itself.
const FOOT_LINKS_TEXT =
  'home · download · privacy · ▶ the short film · design system · GitHub · Chrome extension · Firefox extension · ♥ sponsor';

describe('SiteFooter', () => {
  it('renders the byline', () => {
    const { container } = render(<SiteFooter />);
    expect(container.querySelector('.byline')?.textContent).toBe(
      'made by Saeed, between rejections.'
    );
  });

  it('matches the how-it-works footer text exactly when no current page is given', () => {
    const { container } = render(<SiteFooter />);
    expect(container.querySelector('.foot-links')?.textContent).toBe(FOOT_LINKS_TEXT);
  });

  it('renders the full link order + hrefs with no current page', () => {
    const { container } = render(<SiteFooter />);
    const links = Array.from(container.querySelectorAll('.foot-links a'));
    expect(links.map((a) => [a.getAttribute('href'), a.textContent])).toEqual([
      ['/', 'home'],
      ['/download', 'download'],
      ['/privacy', 'privacy'],
      ['/creature', '▶ the short film'],
      ['/storybook/', 'design system'],
      [GITHUB_REPO, 'GitHub'],
      [CHROME_EXT, 'Chrome extension'],
      [FIREFOX_EXT, 'Firefox extension'],
      [SPONSOR, '♥ sponsor'],
    ]);
  });

  it('renders "privacy" as plain text (not a link) when current="privacy", matching privacy/body.html', () => {
    const { container } = render(<SiteFooter current="privacy" />);
    expect(container.querySelector('.foot-links')?.textContent).toBe(FOOT_LINKS_TEXT);
    const hrefs = Array.from(container.querySelectorAll('.foot-links a')).map((a) =>
      a.getAttribute('href')
    );
    expect(hrefs).not.toContain('/privacy');
    expect(hrefs).toEqual([
      '/',
      '/download',
      '/creature',
      '/storybook/',
      GITHUB_REPO,
      CHROME_EXT,
      FIREFOX_EXT,
      SPONSOR,
    ]);
  });

  it('renders "download" as plain text (not a link) when current="download", matching download/body.html', () => {
    const { container } = render(<SiteFooter current="download" />);
    expect(container.querySelector('.foot-links')?.textContent).toBe(FOOT_LINKS_TEXT);
    const hrefs = Array.from(container.querySelectorAll('.foot-links a')).map((a) =>
      a.getAttribute('href')
    );
    expect(hrefs).not.toContain('/download');
    expect(hrefs).toEqual([
      '/',
      '/privacy',
      '/creature',
      '/storybook/',
      GITHUB_REPO,
      CHROME_EXT,
      FIREFOX_EXT,
      SPONSOR,
    ]);
  });

  it('gives every external link target="_blank" rel="noopener noreferrer"', () => {
    const { container } = render(<SiteFooter />);
    const externals = [GITHUB_REPO, CHROME_EXT, FIREFOX_EXT, SPONSOR];
    for (const href of externals) {
      const a = container.querySelector(`.foot-links a[href="${href}"]`);
      expect(a?.getAttribute('target')).toBe('_blank');
      expect(a?.getAttribute('rel')).toBe('noopener noreferrer');
    }
  });

  it('never puts target/rel on the internal links', () => {
    const { container } = render(<SiteFooter />);
    for (const href of ['/', '/download', '/privacy', '/creature', '/storybook/']) {
      const a = container.querySelector(`.foot-links a[href="${href}"]`);
      expect(a?.getAttribute('target')).toBeNull();
      expect(a?.getAttribute('rel')).toBeNull();
    }
  });
});
