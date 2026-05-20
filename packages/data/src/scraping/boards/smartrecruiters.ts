/**
 * SmartRecruiters — public per-company postings API:
 *   https://api.smartrecruiters.com/v1/companies/{company}/postings
 */
import type { JobPosting } from '@ajh/shared';

import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchJson, stripHtml } from '../http.js';

interface ListResp {
  content: Array<{
    id: string;
    uuid?: string;
    name: string;
    location?: { city?: string; country?: string; remote?: boolean };
    releasedDate?: string;
    ref?: string;
  }>;
}
interface DetailResp {
  jobAd?: { sections?: Record<string, { title?: string; text?: string }> };
  ref?: string;
}

export class SmartRecruitersScraper extends BaseScraper {
  readonly id = 'smartrecruiters';
  readonly displayName = 'SmartRecruiters';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const company = input.query.trim();
    if (!company) return [];
    const list = await fetchJson<ListResp>(
      `https://api.smartrecruiters.com/v1/companies/${encodeURIComponent(company)}/postings?limit=100`,
      { ...(ctx.signal ? { signal: ctx.signal } : {}) }
    );
    if (!list?.content) return [];

    const now = Date.now();
    const out: JobPosting[] = [];
    const total = list.content.length;
    for (let i = 0; i < total; i++) {
      if (ctx.signal.aborted) break;
      const p = list.content[i];
      if (!p) continue;
      const detail = await fetchJson<DetailResp>(
        `https://api.smartrecruiters.com/v1/companies/${encodeURIComponent(company)}/postings/${p.id}`,
        { ...(ctx.signal ? { signal: ctx.signal } : {}) }
      );
      const sections = detail?.jobAd?.sections ?? {};
      const description = Object.values(sections)
        .map((s) => `${s?.title ?? ''}\n${stripHtml(s?.text ?? '')}`)
        .join('\n\n')
        .trim();
      const posting: JobPosting = {
        id: this.makeId(p.id),
        source: this.id,
        externalId: p.id,
        url: `https://jobs.smartrecruiters.com/${encodeURIComponent(company)}/${p.id}`,
        title: p.name,
        company,
        location: [p.location?.city, p.location?.country].filter(Boolean).join(', '),
        remote: p.location?.remote,
        description,
        capturedAt: now,
        ...(p.releasedDate ? { postedAt: Date.parse(p.releasedDate) } : {}),
      };
      out.push(posting);
      await ctx.onItem?.(posting);
      ctx.onProgress?.((i + 1) / total);
    }
    return out;
  }
}
