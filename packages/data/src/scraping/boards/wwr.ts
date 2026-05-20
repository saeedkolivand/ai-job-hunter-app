/**
 * We Work Remotely — public RSS feed:
 *   https://weworkremotely.com/remote-jobs.rss
 */
import { load } from 'cheerio';

import type { JobPosting } from '@ajh/shared';

import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchText, stripHtml } from '../http.js';

export class WeWorkRemotelyScraper extends BaseScraper {
  readonly id = 'wwr';
  readonly displayName = 'We Work Remotely';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const q = input.query.trim().toLowerCase();
    const res = await fetchText('https://weworkremotely.com/remote-jobs.rss', {
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
      // WWR titles often look like "Company: Senior Engineer"
      const split = title.split(/:\s+/);
      const company = split.length > 1 ? (split[0]?.trim() ?? 'Unknown') : 'Unknown';
      const cleanTitle = split.length > 1 ? split.slice(1).join(': ').trim() : title;
      if (q && !title.toLowerCase().includes(q)) return;
      const posting: JobPosting = {
        id: this.makeId(guid),
        source: this.id,
        externalId: guid,
        url: link,
        title: cleanTitle,
        company,
        remote: true,
        description,
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
