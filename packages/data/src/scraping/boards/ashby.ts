/**
 * Ashby — public posting API:
 *   https://api.ashbyhq.com/posting-api/job-board/{board}
 * The "query" is the board slug (e.g. "linear", "anthropic").
 */
import type { JobPosting } from '@ajh/shared';
import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchJson } from '../http.js';

interface AshbyResponse {
  apiVersion: string;
  jobs: Array<{
    id: string;
    title: string;
    departmentName?: string;
    teamName?: string;
    locationName?: string;
    isRemote?: boolean;
    jobUrl: string;
    descriptionPlain?: string;
    publishedAt?: string;
  }>;
}

export class AshbyScraper extends BaseScraper {
  readonly id = 'ashby';
  readonly displayName = 'Ashby';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const board = input.query.trim();
    if (!board) return [];
    const data = await fetchJson<AshbyResponse>(
      `https://api.ashbyhq.com/posting-api/job-board/${encodeURIComponent(board)}?includeCompensation=true`,
      { ...(ctx.signal ? { signal: ctx.signal } : {}) }
    );
    if (!data?.jobs) return [];
    const now = Date.now();
    const out: JobPosting[] = [];
    for (const j of data.jobs) {
      const posting: JobPosting = {
        id: this.makeId(j.id),
        source: this.id,
        externalId: j.id,
        url: j.jobUrl,
        title: j.title,
        company: board,
        location: j.locationName,
        remote: j.isRemote,
        description: (j.descriptionPlain ?? '').trim(),
        capturedAt: now,
        ...(j.publishedAt ? { postedAt: Date.parse(j.publishedAt) } : {}),
      };
      out.push(posting);
      await ctx.onItem?.(posting);
    }
    ctx.onProgress?.(1);
    return out;
  }
}
