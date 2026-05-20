/**
 * GermanTechJobs — Next.js powered board for English-speaking tech roles in DE.
 *   https://germantechjobs.de/
 *
 * Embedded `__NEXT_DATA__` JSON on the home/listing page contains the full
 * job list (props.pageProps.jobs). We parse that instead of the rendered DOM
 * to stay resilient to UI changes.
 */
import { load } from 'cheerio';

import type { JobPosting } from '@ajh/shared';

import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchText, stripHtml } from '../http.js';

interface NextJob {
  _id?: string;
  id?: string;
  slug?: string;
  title?: string;
  companyName?: string;
  description?: string;
  location?: string | string[];
  remote?: boolean;
  tags?: string[];
  skills?: string[];
  createdAt?: string;
  publishedAt?: string;
  url?: string;
}

interface NextData {
  props?: { pageProps?: { jobs?: NextJob[]; jobsList?: NextJob[] } };
}

export class GermanTechJobsScraper extends BaseScraper {
  readonly id = 'germantechjobs';
  readonly displayName = 'German Tech Jobs';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const q = input.query.trim().toLowerCase();
    const loc = input.location?.trim().toLowerCase() ?? '';
    const res = await fetchText('https://germantechjobs.de/', {
      headers: { 'accept-language': 'en-US,en;q=0.9' },
      ...(ctx.signal ? { signal: ctx.signal } : {}),
    });
    if (res.statusCode !== 200) return [];

    const $ = load(res.text);
    const raw = $('#__NEXT_DATA__').first().text();
    if (!raw) return [];

    let data: NextData;
    try {
      data = JSON.parse(raw) as NextData;
    } catch {
      return [];
    }
    const jobs = data.props?.pageProps?.jobs ?? data.props?.pageProps?.jobsList ?? [];
    if (jobs.length === 0) return [];

    const now = Date.now();
    const out: JobPosting[] = [];
    for (const j of jobs) {
      const externalId = j._id ?? j.id ?? j.slug ?? '';
      if (!externalId) continue;

      const skills = j.tags ?? j.skills ?? [];
      const location = Array.isArray(j.location) ? j.location.join(', ') : (j.location ?? '');
      const haystack = `${j.title ?? ''} ${j.companyName ?? ''} ${skills.join(' ')}`.toLowerCase();
      if (q && !haystack.includes(q)) continue;
      if (loc && !location.toLowerCase().includes(loc)) continue;

      const url =
        j.url ??
        (j.slug
          ? `https://germantechjobs.de/job/${j.slug}`
          : `https://germantechjobs.de/job/${externalId}`);

      const posting: JobPosting = {
        id: this.makeId(externalId),
        source: this.id,
        externalId,
        url,
        title: (j.title ?? '').trim(),
        company: (j.companyName ?? 'Unknown').trim(),
        location,
        remote: j.remote,
        description: stripHtml(j.description ?? ''),
        requirements: skills,
        language: 'en',
        capturedAt: now,
        ...(j.publishedAt
          ? { postedAt: Date.parse(j.publishedAt) }
          : j.createdAt
            ? { postedAt: Date.parse(j.createdAt) }
            : {}),
      };
      out.push(posting);
      await ctx.onItem?.(posting);
    }
    ctx.onProgress?.(1);
    return out;
  }
}
