/**
 * Recruitee — public per-company offers API:
 *   https://{company}.recruitee.com/api/offers/
 */
import type { JobPosting } from '@ajh/shared';

import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchJson, stripHtml } from '../http.js';

interface Resp {
  offers: Array<{
    id: number;
    slug: string;
    title: string;
    description?: string;
    requirements?: string;
    careers_url: string;
    city?: string;
    country?: string;
    remote?: boolean;
    created_at?: string;
    company_name?: string;
  }>;
}

export class RecruiteeScraper extends BaseScraper {
  readonly id = 'recruitee';
  readonly displayName = 'Recruitee';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const company = input.query.trim();
    if (!company) return [];
    const data = await fetchJson<Resp>(
      `https://${encodeURIComponent(company)}.recruitee.com/api/offers/`,
      { ...(ctx.signal ? { signal: ctx.signal } : {}) }
    );
    if (!data?.offers) return [];
    const now = Date.now();
    const out: JobPosting[] = [];
    for (const o of data.offers) {
      const description = [stripHtml(o.description ?? ''), stripHtml(o.requirements ?? '')]
        .filter(Boolean)
        .join('\n\n');
      const posting: JobPosting = {
        id: this.makeId(String(o.id)),
        source: this.id,
        externalId: String(o.id),
        url: o.careers_url,
        title: o.title,
        company: o.company_name ?? company,
        location: [o.city, o.country].filter(Boolean).join(', '),
        remote: o.remote,
        description,
        capturedAt: now,
        ...(o.created_at ? { postedAt: Date.parse(o.created_at) } : {}),
      };
      out.push(posting);
      await ctx.onItem?.(posting);
    }
    ctx.onProgress?.(1);
    return out;
  }
}
