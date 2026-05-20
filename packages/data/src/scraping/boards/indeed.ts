/**
 * Indeed — Playwright-driven.
 *
 * Indeed serves heavily rendered HTML and aggressively challenges direct HTTP
 * requests. We drive a real Chromium via the shared BrowserController, scrape
 * search-result tiles, and fetch each detail panel.
 *
 * The scraper degrades gracefully when Indeed shows a CAPTCHA — it simply
 * returns what it has so far. Locale defaults to the user's country in URL.
 */
import path from 'node:path';
import { existsSync } from 'node:fs';
import type { JobPosting } from '@ajh/shared';
import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';

/**
 * Resolve <userData>/board-sessions/<boardId> from the per-board
 * `storageStatePath` (`<userData>/browser-state/<boardId>`). Returns null if
 * neither exists. Cross-platform — never use string-replace on these paths.
 */
function resolveBoardSessionDir(ctx: ScrapeContext, boardId: string): string | null {
  const statePath = ctx.credentials?.storageStatePath(boardId);
  if (!statePath) return null;
  const userData = path.dirname(path.dirname(statePath));
  const sessionDir = path.join(userData, 'board-sessions', boardId);
  return existsSync(sessionDir) ? sessionDir : null;
}

// Maps locale code → Indeed subdomain
const INDEED_DOMAINS: Record<string, string> = {
  us: 'www.indeed.com',
  de: 'de.indeed.com',
  uk: 'uk.indeed.com',
  fr: 'fr.indeed.com',
  at: 'at.indeed.com',
  ch: 'ch.indeed.com',
  au: 'au.indeed.com',
  ca: 'ca.indeed.com',
  nl: 'nl.indeed.com',
  be: 'be.indeed.com',
  es: 'es.indeed.com',
  it: 'it.indeed.com',
  pl: 'pl.indeed.com',
  br: 'br.indeed.com',
  in: 'in.indeed.com',
  sg: 'sg.indeed.com',
  jp: 'jp.indeed.com',
};

export class IndeedScraper extends BaseScraper {
  readonly id = 'indeed';
  readonly displayName = 'Indeed';
  readonly mode = 'browser' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    if (!ctx.browser) return [];
    const keywords = input.query.trim();
    const location = input.location?.trim() ?? '';
    const maxPages = Math.min(Math.max(input.pages, 1), 5);
    const domain = INDEED_DOMAINS[input.locale ?? 'us'] ?? 'www.indeed.com';
    const out: JobPosting[] = [];
    const seen = new Set<string>();
    const now = Date.now();

    const creds = await ctx.credentials?.get(this.id);
    const sessionDir = resolveBoardSessionDir(ctx, this.id);
    const hasSession = !!sessionDir;
    const stateDir = hasSession
      ? sessionDir
      : creds && ctx.credentials
        ? ctx.credentials.storageStatePath(this.id)
        : undefined;

    await ctx.browser.withPage(
      async (page) => {
        // Auth check — secure.indeed.com is universal across all locales
        await page
          .goto('https://secure.indeed.com/account/view', {
            waitUntil: 'domcontentloaded',
            timeout: 30_000,
          })
          .catch(() => {});
        if (page.url().includes('/auth') || page.url().includes('/login')) {
          if (creds) {
            await page
              .goto('https://secure.indeed.com/auth', { waitUntil: 'domcontentloaded' })
              .catch(() => {});
            if (page.url().includes('/auth')) {
              await page
                .fill('input[type="email"], input#ifl-InputFormField-3', creds.username)
                .catch(() => {});
              await page.click('button[type="submit"]').catch(() => {});
              const pwd = page.locator('input[type="password"]').first();
              if (await pwd.isVisible({ timeout: 4000 }).catch(() => false)) {
                await pwd.fill(creds.password);
                await page.click('button[type="submit"]').catch(() => {});
              }
              await page.waitForURL(/indeed\.com\/(?!auth)/, { timeout: 120_000 }).catch(() => {});
            }
          } else {
            throw new Error(
              'Indeed session expired. Please re-connect Indeed in Account Settings.'
            );
          }
        }

        for (let p = 0; p < maxPages; p++) {
          if (ctx.signal.aborted) break;
          const url =
            `https://${domain}/jobs?q=${encodeURIComponent(keywords)}` +
            `&l=${encodeURIComponent(location)}&start=${p * 10}`;
          try {
            await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30_000 });
          } catch {
            break;
          }

          // Captcha / interstitial detection
          if (
            await page
              .locator('text=Verify you are human')
              .first()
              .isVisible()
              .catch(() => false)
          )
            break;

          const cards = await page.locator('a.tapItem, a.result, [data-jk]').all();
          if (cards.length === 0) break;

          for (const card of cards) {
            if (ctx.signal.aborted) break;
            const jk =
              (await card.getAttribute('data-jk')) ?? (await card.getAttribute('id')) ?? '';
            if (!jk || seen.has(jk)) continue;
            seen.add(jk);

            const title = (
              await card
                .locator('h2.jobTitle, .jobTitle')
                .first()
                .innerText()
                .catch(() => '')
            ).trim();
            const company = (
              await card
                .locator('[data-testid="company-name"], .companyName')
                .first()
                .innerText()
                .catch(() => '')
            ).trim();
            const loc = (
              await card
                .locator('[data-testid="text-location"], .companyLocation')
                .first()
                .innerText()
                .catch(() => '')
            ).trim();
            let postedAt: number | undefined;
            try {
              const timeElement = card
                .locator('span[data-testid="myJobsStateDate"], .date')
                .first();
              if (await timeElement.isVisible().catch(() => false)) {
                const dateText = await timeElement.innerText().catch(() => '');
                if (dateText) {
                  // Indeed uses relative dates like "Posted 2 days ago", "Posted just now"
                  // Parse the relative date
                  const match = dateText.match(/(\d+)\s*(day|hour|minute|week|month)s?\s*ago/i);
                  if (match && match[1] && match[2]) {
                    const value = parseInt(match[1]);
                    const unit = match[2].toLowerCase();
                    const now = Date.now();
                    const multiplier =
                      unit === 'hour'
                        ? 3600000
                        : unit === 'minute'
                          ? 60000
                          : unit === 'day'
                            ? 86400000
                            : unit === 'week'
                              ? 604800000
                              : 2592000000;
                    postedAt = now - value * multiplier;
                  } else if (dateText.toLowerCase().includes('just now')) {
                    postedAt = now;
                  }
                }
              }
            } catch {
              /* ignore */
            }
            let description = '';
            try {
              await card.click({ timeout: 4_000 });
              await page.waitForSelector('#jobDescriptionText', { timeout: 6_000 });
              description = (await page.locator('#jobDescriptionText').innerText()).trim();
            } catch {
              /* keep going */
            }

            if (!title) continue;
            const posting: JobPosting = {
              id: this.makeId(jk),
              source: this.id,
              externalId: jk,
              url: `https://${domain}/viewjob?jk=${encodeURIComponent(jk)}`,
              title,
              company: company || 'Unknown',
              location: loc,
              description,
              capturedAt: now,
              ...(postedAt ? { postedAt } : {}),
            };
            out.push(posting);
            await ctx.onItem?.(posting);
          }
          ctx.onProgress?.((p + 1) / maxPages);
          await page.waitForTimeout(800 + Math.random() * 700);
        }
      },
      stateDir ? { persistentStateDir: stateDir, boardId: this.id } : {}
    );

    return out;
  }
}
