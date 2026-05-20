/**
 * Remotive — public JSON API:
 *   https://remotive.com/api/remote-jobs?search={q}
 */
import type { JobPosting } from '@ajh/shared';

import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchJson, stripHtml } from '../http.js';

interface Resp {
  jobs: Array<{
    id: number;
    url: string;
    title: string;
    company_name: string;
    candidate_required_location?: string;
    tags?: string[];
    description?: string;
    publication_date?: string;
  }>;
}

export class RemotiveScraper extends BaseScraper {
  readonly id = 'remotive';
  readonly displayName = 'Remotive';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const q = input.query.trim();
    const url = q
      ? `https://remotive.com/api/remote-jobs?search=${encodeURIComponent(q)}`
      : 'https://remotive.com/api/remote-jobs';
    const data = await fetchJson<Resp>(url, { ...(ctx.signal ? { signal: ctx.signal } : {}) });
    if (!data?.jobs) return [];
    const now = Date.now();
    const out: JobPosting[] = [];
    for (const j of data.jobs) {
      const posting: JobPosting = {
        id: this.makeId(String(j.id)),
        source: this.id,
        externalId: String(j.id),
        url: j.url,
        title: j.title,
        company: j.company_name,
        location: j.candidate_required_location,
        remote: true,
        description: stripHtml(j.description ?? ''),
        requirements: j.tags,
        capturedAt: now,
        ...(j.publication_date ? { postedAt: Date.parse(j.publication_date) } : {}),
      };
      out.push(posting);
      await ctx.onItem?.(posting);
    }
    ctx.onProgress?.(1);
    return out;
  }
}
