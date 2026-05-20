/**
 * Bundesagentur für Arbeit — official German federal employment agency.
 *
 * Public Job Search REST API (used by arbeitsagentur.de itself):
 *   List:   GET https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v4/jobs?was=&wo=&page=&size=
 *   Detail: GET https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v4/jobdetails/{encodedRefnr}
 *
 * The `X-API-Key` value is the public client identifier the site uses for
 * unauthenticated browsing. It is documented in the BA developer portal and
 * shipped in their own front-end JS — not a user secret.
 */
import type { JobPosting } from '@ajh/shared';

import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import { fetchJson, stripHtml } from '../http.js';

const API_BASE = 'https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v4';
const API_KEY = 'jobboerse-jobsuche';

interface ListResp {
  stellenangebote?: Array<{
    refnr: string;
    titel?: string;
    beruf?: string;
    arbeitgeber?: string;
    arbeitsort?: { ort?: string; region?: string; land?: string };
    aktuelleVeroeffentlichungsdatum?: string;
    eintrittsdatum?: string;
    externeUrl?: string;
    hashId?: string;
  }>;
  maxErgebnisse?: number;
}

interface DetailResp {
  refnr: string;
  stellenbeschreibung?: string;
  arbeitgeberdarstellung?: string;
  branche?: { bezeichnung?: string };
  arbeitgeber?: string;
  titel?: string;
}

export class ArbeitsagenturScraper extends BaseScraper {
  readonly id = 'arbeitsagentur';
  readonly displayName = 'Arbeitsagentur';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    const q = input.query.trim();
    const loc = input.location?.trim() ?? '';
    const maxPages = Math.min(Math.max(input.pages, 1), 10);
    const size = 25;
    const out: JobPosting[] = [];
    const seen = new Set<string>();
    const now = Date.now();

    for (let page = 1; page <= maxPages; page++) {
      if (ctx.signal.aborted) break;
      const params = new URLSearchParams({
        was: q,
        page: String(page),
        size: String(size),
      });
      if (loc) params.set('wo', loc);

      const list = await fetchJson<ListResp>(`${API_BASE}/jobs?${params.toString()}`, {
        headers: {
          'X-API-Key': API_KEY,
          accept: 'application/json',
          'accept-language': 'de-DE,de;q=0.9',
        },
        ...(ctx.signal ? { signal: ctx.signal } : {}),
      });
      const items = list?.stellenangebote ?? [];
      if (items.length === 0) break;

      for (const j of items) {
        if (ctx.signal.aborted) break;
        if (!j.refnr || seen.has(j.refnr)) continue;
        seen.add(j.refnr);

        // Detail call uses Base64URL of the refnr
        const hash = j.hashId ?? toBase64Url(j.refnr);
        const detail = await fetchJson<DetailResp>(
          `${API_BASE}/jobdetails/${encodeURIComponent(hash)}`,
          {
            headers: { 'X-API-Key': API_KEY, accept: 'application/json' },
            ...(ctx.signal ? { signal: ctx.signal } : {}),
          }
        );

        const description = stripHtml(
          [detail?.stellenbeschreibung, detail?.arbeitgeberdarstellung].filter(Boolean).join('\n\n')
        );

        const location = [j.arbeitsort?.ort, j.arbeitsort?.region, j.arbeitsort?.land]
          .filter(Boolean)
          .join(', ');

        const posting: JobPosting = {
          id: this.makeId(j.refnr),
          source: this.id,
          externalId: j.refnr,
          url:
            j.externeUrl ??
            `https://www.arbeitsagentur.de/jobsuche/jobdetail/${encodeURIComponent(hash)}`,
          title: (j.titel ?? j.beruf ?? '').trim(),
          company: (j.arbeitgeber ?? 'Unbekannt').trim(),
          location,
          description,
          language: 'de',
          capturedAt: now,
          ...(j.aktuelleVeroeffentlichungsdatum
            ? { postedAt: Date.parse(j.aktuelleVeroeffentlichungsdatum) }
            : {}),
        };
        out.push(posting);
        await ctx.onItem?.(posting);
      }

      ctx.onProgress?.(page / maxPages);
      if (items.length < size) break;
      await delay(700 + Math.random() * 500);
    }
    return out;
  }
}

function toBase64Url(s: string): string {
  return Buffer.from(s, 'utf8')
    .toString('base64')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

function delay(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}
