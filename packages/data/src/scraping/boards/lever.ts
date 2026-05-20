/**
 * Lever — public JSON board API:
 *   https://api.lever.co/v0/postings/{company}?mode=json
 */
import { request } from 'undici';

import type { JobPosting } from '@ajh/shared';

import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';

interface LeverPosting {
  id: string;
  text: string;
  hostedUrl: string;
  categories?: { location?: string; team?: string; commitment?: string };
  descriptionPlain?: string;
  createdAt?: number;
}

export class LeverScraper extends BaseScraper {
  readonly id = 'lever';
  readonly displayName = 'Lever';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const company = input.query.trim();
    if (!company) return [];
    const url = `https://api.lever.co/v0/postings/${encodeURIComponent(company)}?mode=json`;
    const { statusCode, body } = await request(url, { signal: ctx.signal });
    if (statusCode !== 200) return [];
    const data = (await body.json()) as LeverPosting[];
    const now = Date.now();
    const out: JobPosting[] = [];
    for (const p of data) {
      const posting: JobPosting = {
        id: this.makeId(p.id),
        source: this.id,
        externalId: p.id,
        url: p.hostedUrl,
        title: p.text,
        company,
        location: p.categories?.location,
        description: (p.descriptionPlain ?? '').trim(),
        capturedAt: now,
        ...(p.createdAt ? { postedAt: p.createdAt } : {}),
      };
      out.push(posting);
      await ctx.onItem?.(posting);
    }
    ctx.onProgress?.(1);
    return out;
  }
}
