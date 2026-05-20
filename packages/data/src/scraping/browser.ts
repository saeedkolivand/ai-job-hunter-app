/**
 * Playwright controller — lazy singleton.
 *
 * Spawns Chromium on first use, reuses a single context across scrapers,
 * applies sensible anti-bot defaults, and supports clean shutdown.
 * Pages are acquired/released via `withPage()` so scrapers never leak.
 */
import { chromium, type Browser, type BrowserContext, type Page } from 'playwright';
import { createLogger } from '@ajh/core';

export interface BrowserControllerOptions {
  headless?: boolean;
  userAgent?: string;
  locale?: string;
  viewport?: { width: number; height: number };
}

export interface WithPageOptions {
  /**
   * If provided, uses a persistent context stored on disk at this path.
   * Cookies, localStorage, and IndexedDB survive between runs — so once the
   * user logs in to a board, they stay logged in.
   */
  persistentStateDir?: string;
  /** Logical board id, used for logging only. */
  boardId?: string;
}

export class BrowserController {
  private readonly logger = createLogger('browser');
  private browser?: Browser;
  private context?: BrowserContext;
  private starting?: Promise<void>;
  private readonly opts: Required<BrowserControllerOptions>;
  /** persistentStateDir → BrowserContext (each is its own browser instance). */
  private readonly persistent = new Map<string, BrowserContext>();
  private pagesOut = 0;

  constructor(opts: BrowserControllerOptions = {}) {
    this.opts = {
      headless: opts.headless ?? true,
      userAgent:
        opts.userAgent ??
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36',
      locale: opts.locale ?? 'en-US',
      viewport: opts.viewport ?? { width: 1366, height: 900 },
    };
  }

  private async ensure(): Promise<void> {
    if (this.browser && this.context) return;
    if (this.starting) return this.starting;
    this.starting = (async () => {
      this.browser = await chromium.launch({
        headless: this.opts.headless,
        args: [
          '--disable-blink-features=AutomationControlled',
          '--no-default-browser-check',
          '--disable-features=IsolateOrigins,site-per-process',
        ],
      });
      this.context = await this.browser.newContext({
        userAgent: this.opts.userAgent,
        locale: this.opts.locale,
        viewport: this.opts.viewport,
        timezoneId: 'Europe/Berlin',
        bypassCSP: true,
        javaScriptEnabled: true,
      });
      // Light fingerprint smoothing.
      await this.context.addInitScript(() => {
        Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
      });
      this.logger.info('chromium launched');
    })();
    try {
      await this.starting;
    } finally {
      this.starting = undefined;
    }
  }

  async withPage<T>(fn: (page: Page) => Promise<T>, opts: WithPageOptions = {}): Promise<T> {
    const context = opts.persistentStateDir
      ? await this.ensurePersistent(opts.persistentStateDir, opts.boardId)
      : await (async () => {
          await this.ensure();
          if (!this.context) throw new Error('Browser context unavailable after ensure()');
          return this.context;
        })();
    const page = await context.newPage();
    this.pagesOut++;
    try {
      await page.setExtraHTTPHeaders({ 'accept-language': 'en-US,en;q=0.9,de;q=0.8' });
      return await fn(page);
    } finally {
      this.pagesOut--;
      await page.close().catch(() => {});
    }
  }

  private async ensurePersistent(dir: string, boardId?: string): Promise<BrowserContext> {
    const cached = this.persistent.get(dir);
    if (cached) return cached;
    const ctx = await chromium.launchPersistentContext(dir, {
      headless: this.opts.headless,
      userAgent: this.opts.userAgent,
      locale: this.opts.locale,
      viewport: this.opts.viewport,
      timezoneId: 'Europe/Berlin',
      bypassCSP: true,
      args: ['--disable-blink-features=AutomationControlled'],
    });
    await ctx.addInitScript(() => {
      Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
    });
    this.persistent.set(dir, ctx);
    this.logger.info({ dir, boardId }, 'persistent context opened');
    return ctx;
  }

  async close(): Promise<void> {
    await this.closePersistent();
    try {
      await this.context?.close();
    } catch {
      /* noop */
    }
    try {
      await this.browser?.close();
    } catch {
      /* noop */
    }
    this.context = undefined;
    this.browser = undefined;
  }

  /**
   * Close all persistent (per-board) contexts so their on-disk profile
   * directories can be safely deleted (Windows holds file locks while
   * Chromium is running). Leaves the shared non-persistent browser intact.
   */
  async closePersistent(): Promise<void> {
    for (const ctx of this.persistent.values()) {
      try {
        await ctx.close();
      } catch {
        /* noop */
      }
    }
    this.persistent.clear();
  }

  isOpen(): boolean {
    return !!this.browser;
  }
  inFlight(): number {
    return this.pagesOut;
  }
}
