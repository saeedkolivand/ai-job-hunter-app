/**
 * Xing — German/DACH professional network.
 *
 * Xing's job pages are gated; there is no useful unauthenticated API.
 * This scraper REQUIRES the user to have connected their Xing account in
 * Settings → Connected Accounts. Without credentials it returns an empty
 * list and logs a hint (the bootstrap layer surfaces this via the job log).
 *
 * Flow:
 *   1. Reuse persistent Playwright context for board id `xing`.
 *   2. Probe `/jobs/find`. If redirected to /login, perform login with creds.
 *   3. Allow up to 2 minutes for the user to clear 2FA / captcha in-window.
 *   4. Scrape job result cards + detail panels.
 *
 * Selectors target stable `data-testid` attributes where possible, with
 * fallbacks to class-based selectors. Xing's DOM evolves; if breakage
 * happens, the scraper degrades gracefully (returns what it has).
 */
import { existsSync } from 'node:fs';
import path from 'node:path';

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

export class XingScraper extends BaseScraper {
  readonly id = 'xing';
  readonly displayName = 'Xing';
  readonly mode = 'browser' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    if (!ctx.browser || !ctx.credentials) return [];
    const creds = await ctx.credentials.get(this.id);
    const sessionDir = resolveBoardSessionDir(ctx, this.id);
    const hasSession = !!sessionDir;
    if (!creds && !hasSession) return []; // auth-only board

    const out: JobPosting[] = [];
    const seen = new Set<string>();
    const now = Date.now();
    const keywords = input.query.trim();
    const location = input.location?.trim() ?? '';
    const maxPages = Math.min(Math.max(input.pages, 1), 5);

    // Prefer the persistent session if it exists; otherwise fall back to
    // stored username/password credentials (legacy path).
    const stateDir = hasSession
      ? sessionDir
      : creds && ctx.credentials
        ? ctx.credentials.storageStatePath(this.id)
        : undefined;

    await ctx.browser.withPage(
      async (page) => {
        // Probe session
        await page
          .goto('https://www.xing.com/jobs/find', {
            waitUntil: 'domcontentloaded',
            timeout: 30_000,
          })
          .catch(() => {});
        if (/\/login/i.test(page.url())) {
          if (creds) {
            await this.performLogin(page, creds);
            if (ctx.signal.aborted) return;
          } else {
            // Persistent session expired and no stored credentials to fall back on.
            throw new Error('Xing session expired. Please re-connect Xing in Account Settings.');
          }
        }

        for (let p = 0; p < maxPages; p++) {
          if (ctx.signal.aborted) break;
          const url =
            `https://www.xing.com/jobs/search?keywords=${encodeURIComponent(keywords)}` +
            (location ? `&location=${encodeURIComponent(location)}` : '') +
            `&page=${p + 1}`;
          try {
            await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30_000 });
          } catch {
            break;
          }

          // Result list
          await page
            .waitForSelector(
              '[data-testid="job-search-result"], article[data-testid="job-teaser-list-item"], article[class*="JobTeaser"]',
              { timeout: 8_000 }
            )
            .catch(() => {});
          const cards = await page
            .locator(
              '[data-testid="job-search-result"], article[data-testid="job-teaser-list-item"], article[class*="JobTeaser"]'
            )
            .all();
          if (cards.length === 0) break;

          for (const card of cards) {
            if (ctx.signal.aborted) break;
            const href =
              (await card
                .locator('a[href*="/jobs/"]')
                .first()
                .getAttribute('href')
                .catch(() => '')) ?? '';
            const m = /\/jobs\/([^/?#]+)/.exec(href);
            const externalId = m?.[1];
            if (!externalId || seen.has(externalId)) continue;
            seen.add(externalId);

            const title = (
              await card
                .locator('[data-testid="job-title"], h2, [class*="title"]')
                .first()
                .innerText()
                .catch(() => '')
            ).trim();
            const company = (
              await card
                .locator('[data-testid="company-name"], [class*="companyName"]')
                .first()
                .innerText()
                .catch(() => '')
            ).trim();
            const loc = (
              await card
                .locator('[data-testid="job-location"], [class*="location"]')
                .first()
                .innerText()
                .catch(() => '')
            ).trim();
            let postedAt: number | undefined;
            try {
              const timeElement = card
                .locator('time, [data-testid="job-date"], [class*="date"]')
                .first();
              if (await timeElement.isVisible().catch(() => false)) {
                const datetime = await timeElement.getAttribute('datetime').catch(() => null);
                if (datetime) postedAt = Date.parse(datetime);
              }
            } catch {
              /* ignore */
            }

            // Detail panel
            let description = '';
            try {
              await card.click({ timeout: 5_000 });
              await page.waitForSelector(
                '[data-testid="job-description"], section[class*="description"]',
                { timeout: 6_000 }
              );
              description = (
                await page
                  .locator('[data-testid="job-description"], section[class*="description"]')
                  .first()
                  .innerText()
              ).trim();
            } catch {
              /* keep going without description */
            }

            if (!title) continue;
            const fullUrl = href.startsWith('http') ? href : `https://www.xing.com${href}`;
            const posting: JobPosting = {
              id: this.makeId(externalId),
              source: this.id,
              externalId,
              url: fullUrl.split('?')[0] ?? '',
              title,
              company: company || 'Unknown',
              location: loc,
              description,
              language: 'de',
              capturedAt: now,
              ...(postedAt ? { postedAt } : {}),
            };
            out.push(posting);
            await ctx.onItem?.(posting);
          }
          ctx.onProgress?.((p + 1) / maxPages);
          await page.waitForTimeout(900 + Math.random() * 800);
        }
      },
      stateDir ? { persistentStateDir: stateDir, boardId: this.id } : {}
    );

    return out;
  }

  private async performLogin(
    page: {
      goto: (
        url: string,
        opts?: { waitUntil?: 'load' | 'domcontentloaded' | 'networkidle' | 'commit' }
      ) => Promise<unknown>;
      fill: (selector: string, value: string) => Promise<void>;
      click: (selector: string) => Promise<void>;
      waitForURL: (pattern: RegExp, opts?: { timeout?: number }) => Promise<void>;
    },
    creds: { username: string; password: string }
  ): Promise<void> {
    await page
      .goto('https://login.xing.com/login', { waitUntil: 'domcontentloaded' })
      .catch(() => {});
    // Fill credentials
    await page
      .fill('input[type="email"], input[name="username"], input#username', creds.username)
      .catch(() => {});
    await page
      .fill('input[type="password"], input[name="password"], input#password', creds.password)
      .catch(() => {});
    await page.click('button[type="submit"]').catch(() => {});
    // Give the user up to 2 minutes to clear 2FA or any human challenge.
    await page.waitForURL(/xing\.com\/(?!login)/, { timeout: 120_000 }).catch(() => {});
  }
}
