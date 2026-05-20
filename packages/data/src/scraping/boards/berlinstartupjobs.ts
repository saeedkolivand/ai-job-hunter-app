/**
 * Berlin Startup Jobs — public WordPress RSS feed.
 *   https://berlinstartupjobs.com/feed/
 *
 * Title format on this site is typically "Senior Engineer at Acme GmbH".
 */
import { load } from 'cheerio';

import type { JobPosting } from '@ajh/shared';

import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchText, stripHtml } from '../http.js';

export class BerlinStartupJobsScraper extends BaseScraper {
  readonly id = 'berlinstartupjobs';
  readonly displayName = 'Berlin Startup Jobs';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const q = input.query.trim().toLowerCase();
    const res = await fetchText('https://berlinstartupjobs.com/feed/', {
      ...(ctx.signal ? { signal: ctx.signal } : {}),
    });
    if (res.statusCode !== 200) return [];

    const $ = load(res.text, { xmlMode: true });
    const now = Date.now();
    const out: JobPosting[] = [];

    $('item').each((_, el) => {
      const $el = $(el);
      const title = $el.find('title').first().text().trim();
      const link = $el.find('link').first().text().trim();
      const guid = $el.find('guid').first().text().trim() || link;
      const descHtml = $el.find('description').first().text();
      const description = stripHtml(descHtml);
      const pubDate = $el.find('pubDate').first().text().trim();
      const categories = $el
        .find('category')
        .map((__, c) => $(c).text().trim())
        .get();

      // Split "Job Title at Company" → { title, company }
      let cleanTitle = title;
      let company = 'Unknown';
      const m = / at (.+)$/i.exec(title);
      if (m && m[1]) {
        company = m[1].trim();
        cleanTitle = title.slice(0, m.index).trim();
      }

      const haystack = `${title} ${categories.join(' ')}`.toLowerCase();
      if (q && !haystack.includes(q)) return;

      const posting: JobPosting = {
        id: this.makeId(guid),
        source: this.id,
        externalId: guid,
        url: link,
        title: cleanTitle,
        company,
        location: 'Berlin',
        description,
        requirements: categories,
        language: 'en', // most BSJ posts are English
        capturedAt: now,
        ...(pubDate ? { postedAt: Date.parse(pubDate) } : {}),
      };
      out.push(posting);
      void ctx.onItem?.(posting);
    });
    ctx.onProgress?.(1);
    return out;
  }
}
