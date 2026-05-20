/**
 * Workday — per-tenant CXS API:
 *   POST https://{tenant}.wd{N}.myworkdayjobs.com/wday/cxs/{tenant}/{site}/jobs
 *
 * The `query` MUST be a tenant URL OR the form `tenant:site:wd<N>`, since
 * Workday is hosted per-customer (e.g. nvidia:NVIDIAExternalCareerSite:wd5).
 *
 * Examples:
 *   - "nvidia:NVIDIAExternalCareerSite:wd5"
 *   - "https://nvidia.wd5.myworkdayjobs.com/NVIDIAExternalCareerSite"
 */
import type { JobPosting } from '@ajh/shared';
import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchJson, stripHtml } from '../http.js';

interface JobsResp {
  jobPostings: Array<{
    title: string;
    externalPath: string;
    locationsText?: string;
    postedOn?: string;
    bulletFields?: string[];
  }>;
  total: number;
}

interface DetailResp {
  jobPostingInfo?: { jobDescription?: string; jobPostingId?: string };
}

export class WorkdayScraper extends BaseScraper {
  readonly id = 'workday';
  readonly displayName = 'Workday';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const parsed = this.parseTarget(input.query);
    if (!parsed) return [];
    const { tenant, site, host } = parsed;

    const base = `https://${tenant}.${host}.myworkdayjobs.com/wday/cxs/${tenant}/${site}`;
    const maxPages = Math.min(Math.max(input.pages, 1), 5);
    const limit = 20;
    const now = Date.now();
    const out: JobPosting[] = [];

    for (let p = 0; p < maxPages; p++) {
      if (ctx.signal.aborted) break;
      const data = await fetchJson<JobsResp>(`${base}/jobs`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          appliedFacets: {},
          searchText: '',
          limit,
          offset: p * limit,
        }),
        ...(ctx.signal ? { signal: ctx.signal } : {}),
      });
      if (!data?.jobPostings?.length) break;

      for (const j of data.jobPostings) {
        const externalId = j.externalPath.split('/').pop() ?? '';
        const detail = await fetchJson<DetailResp>(`${base}${j.externalPath}`, {
          ...(ctx.signal ? { signal: ctx.signal } : {}),
        });
        const posting: JobPosting = {
          id: this.makeId(externalId),
          source: this.id,
          externalId,
          url: `https://${tenant}.${host}.myworkdayjobs.com/en-US/${site}${j.externalPath}`,
          title: j.title,
          company: tenant,
          location: j.locationsText,
          description: stripHtml(detail?.jobPostingInfo?.jobDescription ?? ''),
          capturedAt: now,
          ...(j.postedOn ? { postedAt: Date.parse(j.postedOn) } : {}),
        };
        out.push(posting);
        await ctx.onItem?.(posting);
      }
      ctx.onProgress?.((p + 1) / maxPages);
      if (data.jobPostings.length < limit) break;
    }
    return out;
  }

  private parseTarget(q: string): { tenant: string; site: string; host: string } | null {
    const trimmed = q.trim();
    if (!trimmed) return null;
    if (trimmed.includes(':')) {
      const [tenant, site, host] = trimmed.split(':');
      if (!tenant || !site) return null;
      return { tenant, site, host: host ?? 'wd1' };
    }
    const m = /^https?:\/\/([^.]+)\.(wd\d+)\.myworkdayjobs\.com\/(?:[a-z-]+\/)?([^/?#]+)/i.exec(
      trimmed
    );
    if (!m) return null;
    return { tenant: m[1] ?? '', host: m[2] ?? '', site: m[3] ?? '' };
  }
}
