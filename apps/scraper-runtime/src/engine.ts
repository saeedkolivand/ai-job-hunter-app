/**
 * ScraperEngine — wires ScraperRegistry + BrowserController for the sidecar.
 *
 * Responsibilities:
 *  - Lazily initialise a Playwright BrowserController on first browser-mode
 *    scrape (HTTP-only scrapers never need it).
 *  - Hold a per-job AbortController map so jobs can be cancelled.
 *  - Provide a single scrapeBoard / scrapeUrl / health surface that
 *    server.ts calls directly.
 *  - Clean shutdown: close the browser and cancel running jobs on exit.
 */
import { createLogger } from '@ajh/core';
import { BrowserController, ScraperRegistry } from '@ajh/data';
import type { JobPosting } from '@ajh/shared';

import type { FileCredentialStore } from './credentials.js';
import { LoginManager } from './login.js';
import type {
  ScrapeBoardPayload,
  ScraperCatalogEntry,
  ScraperEvent,
  ScraperRuntimeHealth,
} from './protocol.js';

const logger = createLogger('scraper-engine');

export interface ScrapeResult {
  board: string;
  count: number;
}

export class ScraperEngine {
  private readonly registry = new ScraperRegistry();
  private browser?: BrowserController;
  private readonly loginManager: LoginManager;
  /** jobId → AbortController (lets the caller cancel a running job). */
  private readonly jobs = new Map<string, AbortController>();

  constructor(private readonly credentials: FileCredentialStore) {
    this.loginManager = new LoginManager(credentials);
  }

  // ── Catalog / health ───────────────────────────────────────────────────────

  catalog(): ScraperCatalogEntry[] {
    return this.registry.catalog();
  }

  health(): ScraperRuntimeHealth {
    return {
      mode: 'http-sidecar',
      scrapers: this.catalog(),
      ready: true,
      port: 0, // overwritten by server.ts via setServerPort
    };
  }

  // ── Lazy browser ───────────────────────────────────────────────────────────

  private async ensureBrowser(): Promise<BrowserController> {
    if (!this.browser) {
      this.browser = new BrowserController({ headless: true });
      logger.info('browser controller initialised');
    }
    return this.browser;
  }

  // ── scrapeBoard ────────────────────────────────────────────────────────────

  async scrapeBoard(
    payload: ScrapeBoardPayload,
    jobId: string,
    emit: (event: ScraperEvent) => void
  ): Promise<ScrapeResult> {
    const scraper = this.registry.get(payload.board);
    if (!scraper) {
      throw new Error(`Unknown board: ${payload.board}`);
    }

    const ac = new AbortController();
    this.jobs.set(jobId, ac);

    let browser: BrowserController | undefined;
    if (scraper.mode === 'browser') {
      browser = await this.ensureBrowser();
    }

    let count = 0;
    try {
      const results = await scraper.search(
        {
          query: payload.query,
          location: payload.location,
          pages: payload.pages,
          dateFilter: payload.dateFilter,
          locale: payload.locale,
        },
        {
          signal: ac.signal,
          browser,
          credentials: this.credentials,
          onProgress: (p) => emit({ kind: 'progress', jobId, p }),
          onItem: (item: JobPosting) => {
            count++;
            emit({ kind: 'item', jobId, item: item as Record<string, unknown> });
          },
        }
      );
      count = results.length;
      logger.info({ board: payload.board, count }, 'scrapeBoard done');
      return { board: payload.board, count };
    } finally {
      this.jobs.delete(jobId);
    }
  }

  // ── scrapeUrl ──────────────────────────────────────────────────────────────

  async scrapeUrl(
    url: string,
    jobId: string,
    emit: (event: ScraperEvent) => void
  ): Promise<JobPosting | null> {
    const ac = new AbortController();
    this.jobs.set(jobId, ac);

    let browser: BrowserController | undefined;
    // Try each scraper that implements fromUrl.
    for (const scraper of this.registry.list()) {
      if (typeof scraper.fromUrl !== 'function') continue;
      if (scraper.mode === 'browser' && !browser) {
        browser = await this.ensureBrowser();
      }
      try {
        const posting = await scraper.fromUrl(url, {
          signal: ac.signal,
          browser,
          credentials: this.credentials,
          onProgress: (p) => emit({ kind: 'progress', jobId, p }),
        });
        if (posting) {
          this.jobs.delete(jobId);
          return posting;
        }
      } catch (e) {
        logger.debug({ scraper: scraper.id, url, err: e }, 'scrapeUrl miss');
      }
    }
    this.jobs.delete(jobId);
    return null;
  }

  // ── Cancel ─────────────────────────────────────────────────────────────────

  cancel(jobId: string): void {
    this.jobs.get(jobId)?.abort();
    this.jobs.delete(jobId);
  }

  setCredentials(boardId: string, username: string, password: string): void {
    this.credentials.set(boardId, username, password);
    this.credentials.flush();
    logger.info({ boardId }, 'credentials updated');
  }

  // ── Login window ───────────────────────────────────────────────────────────

  /**
   * Open a headed Playwright browser for the board's login flow.
   * Streams login.status events while waiting; ends with done once resolved.
   */
  async openLogin(
    boardId: string,
    emit: (event: ScraperEvent) => void
  ): Promise<{ connected: boolean }> {
    const result = await this.loginManager.openLogin(boardId, (note) => {
      emit({ kind: 'login.status', boardId, connected: false, note });
    });
    emit({ kind: 'login.status', boardId, connected: result.connected });
    return result;
  }

  getBoardStatus(boardId: string): { connected: boolean } {
    return this.loginManager.getStatus(boardId);
  }

  disconnectBoard(boardId: string): void {
    this.loginManager.disconnect(boardId);
    logger.info({ boardId }, 'board disconnected');
  }

  // ── Shutdown ───────────────────────────────────────────────────────────────

  async shutdown(): Promise<void> {
    for (const ac of this.jobs.values()) ac.abort();
    this.jobs.clear();
    await this.browser?.close().catch(() => {});
    logger.info('scraper engine shut down');
  }
}
