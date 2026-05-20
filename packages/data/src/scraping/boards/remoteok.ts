/**
 * RemoteOK — public JSON feed (legend item at index 0):
 *   https://remoteok.com/api
 */
import type { JobPosting } from '@ajh/shared';
import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchJson, stripHtml } from '../http.js';

type RemoteOkItem = {
  id?: string | number;
  slug?: string;
  position?: string;
  company?: string;
  location?: string;
  tags?: string[];
  description?: string;
  url?: string;
  apply_url?: string;
  date?: string;
};

export class RemoteOkScraper extends BaseScraper {
  readonly id = 'remoteok';
  readonly displayName = 'RemoteOK';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const q = input.query.trim().toLowerCase();
    const items = await fetchJson<RemoteOkItem[]>('https://remoteok.com/api', {
      ...(ctx.signal ? { signal: ctx.signal } : {}),
    });
    if (!items?.length) return [];
    const now = Date.now();
    const out: JobPosting[] = [];
    for (const it of items) {
      if (!it.id || !it.position) continue; // skip legend entry
      const haystack =
        `${it.position ?? ''} ${it.company ?? ''} ${(it.tags ?? []).join(' ')}`.toLowerCase();
      if (q && !haystack.includes(q)) continue;
      const id = String(it.id);
      const posting: JobPosting = {
        id: this.makeId(id),
        source: this.id,
        externalId: id,
        url: it.url ?? it.apply_url ?? `https://remoteok.com/remote-jobs/${it.slug ?? id}`,
        title: it.position ?? '',
        company: it.company ?? 'Unknown',
        location: it.location,
        remote: true,
        description: stripHtml(it.description ?? ''),
        requirements: it.tags,
        capturedAt: now,
        ...(it.date ? { postedAt: Date.parse(it.date) } : {}),
      };
      out.push(posting);
      await ctx.onItem?.(posting);
    }
    ctx.onProgress?.(1);
    return out;
  }
}
