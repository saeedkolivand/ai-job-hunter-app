/**
 * StepStone (Germany) — public listing pages, parsed with cheerio.
 *   https://www.stepstone.de/jobs/{keywords}/in-{location}?action=paging_next&page={n}
 *
 * Site markup changes often — selectors here target stable data-* attributes
 * and fall back to ld+json embedded in each card where possible.
 */
import { load } from 'cheerio';
import type { JobPosting } from '@ajh/shared';
import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchText, stripHtml } from '../http.js';

export class StepStoneScraper extends BaseScraper {
  readonly id = 'stepstone';
  readonly displayName = 'StepStone';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const q = input.query.trim();
    const loc = input.location?.trim() ?? '';
    const maxPages = Math.min(Math.max(input.pages, 1), 5);
    const out: JobPosting[] = [];
    const seen = new Set<string>();
    const now = Date.now();

    for (let p = 1; p <= maxPages; p++) {
      if (ctx.signal.aborted) break;
      const url =
        `https://www.stepstone.de/jobs/${encodeURIComponent(q)}` +
        (loc ? `/in-${encodeURIComponent(loc)}` : '') +
        `?page=${p}`;
      const res = await fetchText(url, {
        headers: { 'accept-language': 'de-DE,de;q=0.9,en;q=0.7' },
        ...(ctx.signal ? { signal: ctx.signal } : {}),
      });
      if (res.statusCode !== 200) break;
      const $ = load(res.text);

      // Try ld+json JobPosting blocks first — most reliable on StepStone.
      const ldBlocks = $('script[type="application/ld+json"]').toArray();
      let foundAny = false;
      for (const node of ldBlocks) {
        try {
          const raw = $(node).contents().first().data() ?? $(node).text();
          const data = JSON.parse(String(raw));
          const items = Array.isArray(data) ? data : [data];
          for (const it of items) {
            if (!it || it['@type'] !== 'JobPosting') continue;
            const url = it.url ?? '';
            const id = /[?&]ID=([^&]+)/.exec(url)?.[1] ?? /(\d{6,})/.exec(url)?.[1] ?? url;
            if (!id || seen.has(id)) continue;
            seen.add(id);
            foundAny = true;
            const posting: JobPosting = {
              id: this.makeId(id),
              source: this.id,
              externalId: id,
              url,
              title: String(it.title ?? '').trim(),
              company: String(it.hiringOrganization?.name ?? 'Unknown').trim(),
              location: [
                it.jobLocation?.address?.addressLocality,
                it.jobLocation?.address?.addressCountry,
              ]
                .filter(Boolean)
                .join(', '),
              description: stripHtml(String(it.description ?? '')),
              language: 'de',
              capturedAt: now,
              ...(it.datePosted ? { postedAt: Date.parse(String(it.datePosted)) } : {}),
            };
            out.push(posting);
            await ctx.onItem?.(posting);
          }
        } catch {
          /* ignore malformed ld+json */
        }
      }
      if (!foundAny) break;
      ctx.onProgress?.(p / maxPages);
      await delay(900 + Math.random() * 600);
    }
    return out;
  }
}

function delay(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}
