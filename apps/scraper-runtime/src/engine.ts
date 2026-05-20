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
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { createLogger } from '@ajh/core';
import {
  ApplierRegistry,
  BrowserController,
  extractDocxFromBytes,
  extractPdfFromBytes,
  ScraperRegistry,
} from '@ajh/data';
import type { JobPosting } from '@ajh/shared';

import type { FileCredentialStore } from './credentials.js';
import { DataStore } from './data-store.js';
import { LoginManager } from './login.js';
import type {
  ApplyJobPayload,
  ApplyResult,
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
  private readonly appliers = new ApplierRegistry();
  private browser?: BrowserController;
  private readonly loginManager: LoginManager;
  readonly dataStore: DataStore;
  /** Maximum number of concurrent scraping jobs (adjusted by performance mode). */
  private maxConcurrentJobs = 2;
  /** jobId → AbortController (lets the caller cancel a running job). */
  private readonly jobs = new Map<string, AbortController>();

  constructor(
    private readonly credentials: FileCredentialStore,
    dataDir: string
  ) {
    this.loginManager = new LoginManager(credentials);
    this.dataStore = new DataStore(dataDir);
  }

  async openDataStore(): Promise<void> {
    await this.dataStore.open();
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

    if (this.jobs.size >= this.maxConcurrentJobs) {
      throw new Error(
        `Concurrency limit reached (max ${this.maxConcurrentJobs}). Cancel a running job first.`
      );
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

  setPerformanceMode(mode: 'low-memory' | 'balanced' | 'performance'): void {
    this.maxConcurrentJobs = mode === 'low-memory' ? 1 : mode === 'performance' ? 4 : 2;
    logger.info({ mode, maxConcurrentJobs: this.maxConcurrentJobs }, 'performance mode applied');
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

  // ── Apply flow ────────────────────────────────────────────────────────────

  applierCatalog(): Array<{ id: string; displayName: string }> {
    return this.appliers.catalog();
  }

  async applyJob(
    payload: ApplyJobPayload,
    jobId: string,
    emit: (event: ScraperEvent) => void
  ): Promise<ApplyResult> {
    const applier = this.appliers.get(payload.board);
    if (!applier) throw new Error(`No applier registered for board: ${payload.board}`);

    const browser = await this.ensureBrowser();
    const creds = await this.credentials.get(payload.board);
    const statePath = this.credentials.storageStatePath(payload.board);

    // Write resume bytes to a temp file if provided — appliers expect a path.
    let resumePath: string | undefined;
    let tempFile: string | undefined;
    if (payload.resumeBytesBase64 && payload.resumeName) {
      const ext = path.extname(payload.resumeName) || '.pdf';
      tempFile = path.join(os.tmpdir(), `ajh-resume-${jobId}${ext}`);
      fs.writeFileSync(tempFile, Buffer.from(payload.resumeBytesBase64, 'base64'));
      resumePath = tempFile;
    }

    try {
      const result = await applier.apply(payload.url, {
        signal: new AbortController().signal,
        browser: browser as never,
        storageStatePath: statePath,
        credentials: creds ?? null,
        coverLetter: payload.coverLetter,
        resumePath,
        autoSubmit: payload.autoSubmit ?? false,
        onProgress: (p, _stage) => emit({ kind: 'progress', jobId, p }),
        onStep: (step) =>
          emit({
            kind: 'done',
            jobId,
            result: { type: 'step', ...step },
          }),
      });
      return result;
    } finally {
      if (tempFile) {
        try {
          fs.unlinkSync(tempFile);
        } catch {
          /* ignore */
        }
      }
    }
  }

  // ── Text extraction ───────────────────────────────────────────────────────

  /**
   * Extract plain text from a document file (PDF, DOCX, TXT, MD).
   * Bytes are passed as base64 to stay within the JSON protocol.
   */
  async extractText(name: string, bytesBase64: string): Promise<{ text: string }> {
    const bytes = new Uint8Array(Buffer.from(bytesBase64, 'base64'));
    const ext = name.toLowerCase().split('.').pop() ?? '';

    if (ext === 'pdf') {
      const { text } = await extractPdfFromBytes(bytes);
      return { text };
    }
    if (ext === 'docx') {
      return extractDocxFromBytes(bytes);
    }
    if (ext === 'txt' || ext === 'md' || ext === 'markdown') {
      return { text: Buffer.from(bytes).toString('utf-8').trim() };
    }
    throw new Error(`unsupported file type: .${ext}`);
  }

  // ── Shutdown ───────────────────────────────────────────────────────────────

  async shutdown(): Promise<void> {
    for (const ac of this.jobs.values()) ac.abort();
    this.jobs.clear();
    await this.browser?.close().catch(() => {});
    logger.info('scraper engine shut down');
  }
}
