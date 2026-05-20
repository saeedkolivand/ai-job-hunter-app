/**
 * Greenhouse — public per-company JSON board API:
 *   https://boards-api.greenhouse.io/v1/boards/{company}/jobs
 * We treat the "query" as the company slug for the MVP, then fan-out per posting.
 */
import { request } from 'undici';

import type { JobPosting } from '@ajh/shared';

import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';

interface GhJobsResponse {
  jobs: Array<{
    id: number;
    title: string;
    absolute_url: string;
    location: { name: string };
    content?: string;
    updated_at?: string;
    metadata?: Array<{ name: string; value: unknown }>;
  }>;
}

export class GreenhouseScraper extends BaseScraper {
  readonly id = 'greenhouse';
  readonly displayName = 'Greenhouse';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const company = input.query.trim();
    if (!company) return [];
    const url = `https://boards-api.greenhouse.io/v1/boards/${encodeURIComponent(company)}/jobs?content=true`;
    const { statusCode, body } = await request(url, { signal: ctx.signal });
    if (statusCode !== 200) return [];
    const data = (await body.json()) as GhJobsResponse;
    const out: JobPosting[] = [];
    const now = Date.now();
    for (const j of data.jobs) {
      const posting: JobPosting = {
        id: this.makeId(String(j.id)),
        source: this.id,
        externalId: String(j.id),
        url: j.absolute_url,
        title: j.title,
        company,
        location: j.location?.name,
        description: (j.content ?? '')
          .replace(/<[^>]+>/g, ' ')
          .replace(/\s+/g, ' ')
          .trim(),
        capturedAt: now,
        ...(j.updated_at ? { postedAt: Date.parse(j.updated_at) } : {}),
      };
      out.push(posting);
      await ctx.onItem?.(posting);
    }
    ctx.onProgress?.(1);
    return out;
  }
}
