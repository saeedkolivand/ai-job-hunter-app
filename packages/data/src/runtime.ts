/**
 * Data Runtime — implements core.Runtime.
 *
 * Owns: SQLite (Drizzle), LanceDB (vectors), file pipeline, scraping registry,
 * matching engine, hybrid search.
 *
 * It does NOT spawn worker threads itself — workers are owned at the
 * main-process bootstrap and injected into the file pipeline as deps.
 */
import path from 'node:path';
import { mkdir } from 'node:fs/promises';
import { createLogger, type Runtime, type EventBus } from '@ajh/core';
import { createDb, type Db } from './db/client.js';
import { VectorStore } from './vector/lancedb.js';
import { ScraperRegistry } from './scraping/registry.js';
import { ApplierRegistry } from './applying/registry.js';
import { BrowserController } from './scraping/browser.js';
import { MatchingEngine } from './matching/engine.js';
import {
  BoardSessionManager,
  BOARD_SESSION_CONFIGS,
  type BoardSessionStatus,
} from './scraping/board-session-manager.js';
import { InMemoryJobStore } from './jobs/in-memory-store.js';

export interface DataRuntimeOptions {
  userDataDir: string; // Electron app.getPath('userData')
}

export class DataRuntime implements Runtime {
  readonly id = 'data';
  private readonly logger = createLogger('runtime.data');
  private dbHandle?: Db;
  private vectors?: VectorStore;
  readonly scrapers = new ScraperRegistry();
  readonly appliers = new ApplierRegistry();
  readonly matching = new MatchingEngine();
  readonly browser = new BrowserController({ headless: true });
  /**
   * Per-board session managers. The user "Connects" each board once via
   * Settings → Accounts; the persistent context is reused by scrapers.
   */
  readonly boardSessions = new Map<string, BoardSessionManager>();
  /**
   * In-memory store for live scraping results.
   * Jobs are stored temporarily during scraping and cleared when scraping completes.
   */
  readonly liveJobs = new InMemoryJobStore();

  constructor(
    private readonly _bus: EventBus,
    private readonly opts: DataRuntimeOptions
  ) {
    for (const cfg of Object.values(BOARD_SESSION_CONFIGS)) {
      this.boardSessions.set(cfg.id, new BoardSessionManager(this.opts.userDataDir, cfg));
    }
  }

  /** Resolve the on-disk persistent-context dir for a board, or null. */
  boardSessionDir(boardId: string): string | null {
    const m = this.boardSessions.get(boardId);
    return m ? m.resolveSessionDir() : null;
  }

  async boardSessionStatus(boardId: string): Promise<BoardSessionStatus> {
    const m = this.boardSessions.get(boardId);
    if (!m) return { connected: false };
    return m.getStatus();
  }

  async start(): Promise<void> {
    await mkdir(this.opts.userDataDir, { recursive: true });
    this.dbHandle = createDb(path.join(this.opts.userDataDir, 'app.db')).db;
    this.vectors = new VectorStore(path.join(this.opts.userDataDir, 'lancedb'));
    await this.vectors.open();
    this.logger.info(
      {
        userDataDir: this.opts.userDataDir,
        scrapers: this.scrapers.catalog().length,
        appliers: this.appliers.catalog().length,
      },
      'data runtime ready'
    );
  }

  async stop(): Promise<void> {
    await this.browser.close();
    for (const m of this.boardSessions.values()) await m.close().catch(() => {});
    await this.vectors?.close();
    // NeDB doesn't need explicit close, but we can clear the reference
    this.dbHandle = undefined;
  }

  async health(): Promise<Record<string, unknown>> {
    return {
      ready: !!this.dbHandle && !!this.vectors,
      sqlite: !!this.dbHandle,
      vector: !!this.vectors,
      browser: this.browser.isOpen(),
      scrapers: this.scrapers.catalog().length,
    };
  }

  db(): Db {
    if (!this.dbHandle) throw new Error('DataRuntime not started');
    return this.dbHandle;
  }

  vectorStore(): VectorStore {
    if (!this.vectors) throw new Error('DataRuntime not started');
    return this.vectors;
  }
}
