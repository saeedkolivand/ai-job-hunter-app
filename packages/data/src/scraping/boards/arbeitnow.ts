/**
 * Arbeitnow — public JSON API:
 *   https://www.arbeitnow.com/api/job-board-api?page={n}
 */
import type { JobPosting } from '@ajh/shared';

import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchJson, stripHtml } from '../http.js';

interface Resp {
  data: Array<{
    slug: string;
    company_name: string;
    title: string;
    description?: string;
    remote?: boolean;
    url: string;
    tags?: string[];
    location?: string;
    created_at?: number;
  }>;
  links?: { next?: string };
}

export class ArbeitnowScraper extends BaseScraper {
  readonly id = 'arbeitnow';
  readonly displayName = 'Arbeitnow';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const q = input.query.trim().toLowerCase();
    const now = Date.now();
    const out: JobPosting[] = [];
    const maxPages = Math.min(Math.max(input.pages, 1), 5);

    for (let page = 1; page <= maxPages; page++) {
      if (ctx.signal.aborted) break;
      const data = await fetchJson<Resp>(
        `https://www.arbeitnow.com/api/job-board-api?page=${page}`,
        {
          ...(ctx.signal ? { signal: ctx.signal } : {}),
        }
      );
      if (!data?.data?.length) break;
      for (const j of data.data) {
        const haystack = `${j.title} ${j.company_name} ${(j.tags ?? []).join(' ')}`.toLowerCase();
        if (q && !haystack.includes(q)) continue;
        const posting: JobPosting = {
          id: this.makeId(j.slug),
          source: this.id,
          externalId: j.slug,
          url: j.url,
          title: j.title,
          company: j.company_name,
          location: j.location,
          remote: j.remote,
          description: stripHtml(j.description ?? ''),
          requirements: j.tags,
          capturedAt: now,
          ...(j.created_at ? { postedAt: j.created_at * 1000 } : {}),
        };
        out.push(posting);
        await ctx.onItem?.(posting);
      }
      ctx.onProgress?.(page / maxPages);
      if (!data.links?.next) break;
    }
    return out;
  }
}
