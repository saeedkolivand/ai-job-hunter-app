/**
 * Y Combinator Jobs (Work at a Startup) — public Algolia search:
 *   https://hn.algolia.com/api/v1/search?tags=show_hn  -- (no, that's HN)
 * The reliable public path is the Algolia index that powers workatastartup search:
 *   POST https://0t3kibpncw-dsn.algolia.net/1/indexes/JobPosting/query
 *   with the WaaS public API key in headers.
 *
 * Keys rotate; treat failures gracefully. We fall back to an empty list.
 */
import type { JobPosting } from '@ajh/shared';
import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchJson, stripHtml } from '../http.js';

interface Hit {
  objectID: string;
  title?: string;
  description?: string;
  company_name?: string;
  location?: string;
  remote?: boolean;
  apply_url?: string;
  created_at_i?: number;
}
interface AlgoliaResp {
  hits: Hit[];
}

// Public Algolia creds for Work-at-a-Startup. If/when these rotate, this scraper
// degrades gracefully — main process never throws.
const ALGOLIA_APP_ID = '45BWZJ1SGC';
const ALGOLIA_API_KEY =
  'NDYyYmNmMDU5OWVmNzNlNzMwMWQ5MDE4ZWY3M2NlNDU0NjA5MTRmZTdiNDAxYjE3MTUyYmU5OWZlNjVmZmUyZHRhZ0ZpbHRlcnM9JTVCJTIyam9icyUyMiU1RA==';

export class YCombinatorScraper extends BaseScraper {
  readonly id = 'ycombinator';
  readonly displayName = 'Y Combinator';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const q = input.query.trim();
    const url = `https://${ALGOLIA_APP_ID.toLowerCase()}-dsn.algolia.net/1/indexes/JobPosting/query`;
    const data = await fetchJson<AlgoliaResp>(url, {
      method: 'POST',
      headers: {
        'x-algolia-application-id': ALGOLIA_APP_ID,
        'x-algolia-api-key': ALGOLIA_API_KEY,
        'content-type': 'application/json',
      },
      body: JSON.stringify({ query: q, hitsPerPage: 50 }),
      ...(ctx.signal ? { signal: ctx.signal } : {}),
    });
    if (!data?.hits) return [];
    const now = Date.now();
    const out: JobPosting[] = [];
    for (const h of data.hits) {
      if (!h.title) continue;
      const posting: JobPosting = {
        id: this.makeId(h.objectID),
        source: this.id,
        externalId: h.objectID,
        url: h.apply_url ?? `https://www.workatastartup.com/jobs/${h.objectID}`,
        title: h.title,
        company: h.company_name ?? 'Unknown',
        location: h.location,
        remote: h.remote,
        description: stripHtml(h.description ?? ''),
        capturedAt: now,
        ...(h.created_at_i ? { postedAt: h.created_at_i * 1000 } : {}),
      };
      out.push(posting);
      await ctx.onItem?.(posting);
    }
    ctx.onProgress?.(1);
    return out;
  }
}
