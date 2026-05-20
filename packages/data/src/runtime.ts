/**
 * Data Runtime — implements core.Runtime.
 *
 * Owns: SQLite (Drizzle), LanceDB (vectors), file pipeline, scraping registry,
 * matching engine, hybrid search.
 *
 * It does NOT spawn worker threads itself — workers are owned at the
 * main-process bootstrap and injected into the file pipeline as deps.
 */
import { mkdir } from 'node:fs/promises';
import path from 'node:path';

import { createLogger, type EventBus, type Runtime } from '@ajh/core';

import { ApplierRegistry } from './applying/registry.js';
import { createDb, type Db } from './db/client.js';
import { InMemoryJobStore } from './jobs/in-memory-store.js';
import { MatchingEngine } from './matching/engine.js';
import { BrowserController } from './scraping/browser.js';
import { ScraperRegistry } from './scraping/registry.js';
import { VectorStore } from './vector/lancedb.js';

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
   * In-memory store for live scraping results.
   * Jobs are stored temporarily during scraping and cleared when scraping completes.
   */
  readonly liveJobs = new InMemoryJobStore();

  constructor(
    private readonly _bus: EventBus,
    private readonly opts: DataRuntimeOptions
  ) {}

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
    await this.vectors?.close();
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
