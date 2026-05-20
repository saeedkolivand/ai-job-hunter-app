/**
 * Scraper runtime boundary.
 *
 * ScraperRuntimeClient is the stable interface all job handlers depend on.
 * Today it is backed by InProcessScraperRuntime (everything in main thread).
 * A future UtilityProcessScraperRuntime or HTTP sidecar implements the same
 * interface without touching any job handler or IPC route.
 *
 * ── Rollback ──────────────────────────────────────────────────────────────
 * AJH_SCRAPER_MODE=in-process  forces the in-process fallback regardless of
 * any future default. Useful when isolating a crash or testing the boundary.
 *
 * ── Protocol types ────────────────────────────────────────────────────────
 * ScraperCommand / ScraperEvent define the serialisable message shapes that
 * cross a process boundary. They are not used by InProcessScraperRuntime
 * (which calls methods directly) but document the contract a remote
 * implementation must honour.
 */
import { createLogger } from '@ajh/core';
import type { ApplyResult, DataRuntime } from '@ajh/data';
import type { JobPosting } from '@ajh/shared';

import type { CredentialStore } from './credentials.js';
import type { ElectronBrowserController } from './electron-browser-controller.js';

// ── Public interface ───────────────────────────────────────────────────────

export interface ScraperCatalogEntry {
  id: string;
  displayName: string;
  mode: 'http' | 'browser';
}

export interface ScraperRuntimeHealth {
  mode: 'in-process' | 'utility-process' | 'http-sidecar';
  scrapers: ScraperCatalogEntry[];
  ready: boolean;
}

/**
 * Stable abstraction over "whatever runs the scrapers".
 * Job handlers in bootstrap.ts type against this — not the concrete class.
 */
export interface ScraperRuntimeClient {
  scrapeBoard(
    payload: ScrapeBoardPayload,
    ctx: ScrapeBoardCtx
  ): Promise<{ board: string; count: number }>;
  applyJob(payload: ApplyJobPayload, ctx: ApplyJobCtx): Promise<ApplyResult>;
  scrapeUrl(payload: { url: string }, ctx: ScrapeUrlCtx): Promise<JobPosting>;
  health(): Promise<ScraperRuntimeHealth>;
  catalog(): ScraperCatalogEntry[];
}

// ── Protocol types (process-boundary contract) ─────────────────────────────
// These are the serialisable messages a remote runtime must accept/emit.
// A UtilityProcess or Tauri sidecar sends ScraperCommands and receives
// ScraperEvents over stdio, localhost HTTP, or WebSocket.

export type ScraperCommand =
  | { kind: 'scrape.board'; jobId: string; payload: ScrapeBoardPayload }
  | { kind: 'apply.job'; jobId: string; payload: ApplyJobPayload }
  | { kind: 'scrape.url'; jobId: string; payload: { url: string } }
  | { kind: 'health' }
  | { kind: 'catalog' };

export type ScraperEvent =
  | { kind: 'progress'; jobId: string; p: number }
  | { kind: 'item'; jobId: string; item: JobPosting }
  | { kind: 'step'; jobId: string; stage: string; ok: boolean; note?: string }
  | { kind: 'done'; jobId: string; result: unknown }
  | { kind: 'error'; jobId: string; message: string }
  | { kind: 'health.reply'; health: ScraperRuntimeHealth }
  | { kind: 'catalog.reply'; scrapers: ScraperCatalogEntry[] };

// ── Implementation ─────────────────────────────────────────────────────────

const logger = createLogger('scraper-runtime');

export interface ScrapeBoardPayload {
  board: string;
  query: string;
  location?: string;
  pages: number;
  dateFilter?: string;
  locale?: string;
}

export interface ApplyJobPayload {
  board: string;
  url: string;
  coverLetter?: string;
  resumePath?: string;
  autoSubmit?: boolean;
}

export interface ScrapeBoardCtx {
  signal: AbortSignal;
  jobId: string;
  onProgress(p: number): void;
  onItem(item: JobPosting): void | Promise<void>;
}

export interface ApplyJobCtx {
  signal: AbortSignal;
  onProgress(p: number, stage: string): void;
  onStep(step: { stage: string; ok: boolean; note?: string }): void;
}

export interface ScrapeUrlCtx {
  signal: AbortSignal;
  onItem(item: JobPosting): void;
}

export class InProcessScraperRuntime implements ScraperRuntimeClient {
  constructor(
    private readonly data: DataRuntime,
    private readonly credentials: CredentialStore,
    private readonly browser: ElectronBrowserController
  ) {}

  catalog(): ScraperCatalogEntry[] {
    return this.data.scrapers.catalog().map((s) => ({
      id: s.id,
      displayName: s.displayName,
      mode: s.mode,
    }));
  }

  async health(): Promise<ScraperRuntimeHealth> {
    return {
      mode: 'in-process',
      scrapers: this.catalog(),
      ready: true,
    };
  }

  private credentialsAccessor() {
    return {
      get: async (id: string) => {
        const c = await this.credentials.getDecrypted(id);
        return c ? { username: c.username, password: c.password } : null;
      },
      storageStatePath: (id: string) => this.credentials.storageStatePath(id),
    };
  }

  private async upsertPosting(posting: Record<string, unknown>): Promise<void> {
    const db = this.data.db();
    const query = posting.externalId
      ? { source: posting.source, externalId: posting.externalId }
      : { url: posting.url };
    await new Promise<void>((resolve, reject) => {
      db.jobPostings.update(
        query,
        { $set: { ...posting, capturedAt: Date.now() } },
        { upsert: true },
        (err) => {
          if (err) reject(err);
          else resolve();
        }
      );
    });
  }

  async scrapeBoard(
    payload: ScrapeBoardPayload,
    ctx: ScrapeBoardCtx
  ): Promise<{ board: string; count: number }> {
    const scraper = this.data.scrapers.get(payload.board);
    if (!scraper) throw new Error(`Unknown board: ${payload.board}`);
    logger.info(
      { board: payload.board, query: payload.query, mode: scraper.mode },
      'scrape.board start'
    );

    this.data.liveJobs.clearAll();

    const results = await scraper.search(
      {
        query: payload.query,
        ...(payload.location ? { location: payload.location } : {}),
        pages: payload.pages,
        ...(payload.dateFilter ? { dateFilter: payload.dateFilter } : {}),
        ...(payload.locale ? { locale: payload.locale } : {}),
      },
      {
        signal: ctx.signal,
        onProgress: ctx.onProgress,
        onItem: (item) => {
          this.data.liveJobs.add(ctx.jobId, item);
          ctx.onItem(item);
        },
        browser: this.browser as never,
        credentials: this.credentialsAccessor(),
      }
    );

    return { board: payload.board, count: results.length };
  }

  async applyJob(payload: ApplyJobPayload, ctx: ApplyJobCtx): Promise<ApplyResult> {
    const applier = this.data.appliers.get(payload.board);
    if (!applier) throw new Error(`No applier registered for ${payload.board}`);
    const cred = await this.credentials.getDecrypted(payload.board);
    logger.info(
      { board: payload.board, url: payload.url, autoSubmit: !!payload.autoSubmit },
      'apply.job start'
    );

    const result = await applier.apply(payload.url, {
      signal: ctx.signal,
      browser: this.browser as never,
      storageStatePath: this.credentials.storageStatePath(payload.board),
      credentials: cred ? { username: cred.username, password: cred.password } : null,
      ...(payload.coverLetter ? { coverLetter: payload.coverLetter } : {}),
      ...(payload.resumePath ? { resumePath: payload.resumePath } : {}),
      autoSubmit: !!payload.autoSubmit,
      onProgress: ctx.onProgress,
      onStep: ctx.onStep,
    });

    // Scrape and persist the job posting after applying so it appears in the DB.
    const ca = this.credentialsAccessor();
    for (const s of this.data.scrapers.list()) {
      if (typeof s.fromUrl !== 'function') continue;
      const item = await s.fromUrl(payload.url, {
        signal: ctx.signal,
        browser: this.browser as never,
        credentials: ca,
      });
      if (item) {
        await this.upsertPosting(item);
        break;
      }
    }

    return result;
  }

  async scrapeUrl(payload: { url: string }, ctx: ScrapeUrlCtx): Promise<JobPosting> {
    const ca = this.credentialsAccessor();
    for (const s of this.data.scrapers.list()) {
      if (typeof s.fromUrl !== 'function') continue;
      const item = await s.fromUrl(payload.url, {
        signal: ctx.signal,
        browser: this.browser as never,
        credentials: ca,
      });
      if (item) {
        ctx.onItem(item);
        return item;
      }
    }
    throw new Error('No scraper accepted this URL');
  }
}
