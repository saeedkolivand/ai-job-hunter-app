/**
 * Personio — public XML feed per company:
 *   https://{company}.jobs.personio.de/xml
 *   (also: .com / .jobs.personio.com depending on tenant)
 */
import { load } from 'cheerio';
import type { JobPosting } from '@ajh/shared';
import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchText, stripHtml } from '../http.js';

const HOSTS = ['jobs.personio.de', 'jobs.personio.com'];

export class PersonioScraper extends BaseScraper {
  readonly id = 'personio';
  readonly displayName = 'Personio';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const company = input.query.trim().toLowerCase();
    if (!company) return [];

    let xml: string | null = null;
    for (const host of HOSTS) {
      const res = await fetchText(`https://${company}.${host}/xml`, {
        ...(ctx.signal ? { signal: ctx.signal } : {}),
      });
      if (res.statusCode === 200 && res.text.includes('<position')) {
        xml = res.text;
        break;
      }
    }
    if (!xml) return [];

    const $ = load(xml, { xmlMode: true });
    const now = Date.now();
    const out: JobPosting[] = [];
    $('position').each((_, el) => {
      const $el = $(el);
      const id = $el.find('id').first().text().trim();
      if (!id) return;
      const title = $el.find('name').first().text().trim();
      const office = $el.find('office').first().text().trim();
      const desc = $el
        .find('jobDescription value')
        .map((__, v) => stripHtml($(v).text()))
        .get()
        .join('\n\n');
      const created = $el.find('createdAt').first().text().trim();
      const posting: JobPosting = {
        id: this.makeId(id),
        source: this.id,
        externalId: id,
        url: `https://${company}.${HOSTS[0]}/job/${id}`,
        title,
        company,
        location: office,
        description: desc,
        capturedAt: now,
        ...(created ? { postedAt: Date.parse(created) } : {}),
      };
      out.push(posting);
      void ctx.onItem?.(posting);
    });
    ctx.onProgress?.(1);
    return out;
  }
}
