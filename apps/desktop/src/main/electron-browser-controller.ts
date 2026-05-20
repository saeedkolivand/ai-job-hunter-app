/**
 * Electron-native browser controller.
 *
 * Drop-in replacement for the Playwright BrowserController in packages/data.
 * Uses Electron's own bundled Chromium via hidden BrowserWindow instances —
 * zero external dependency, same engine as the rest of the app.
 *
 * Exposes the same withPage() interface so scrapers that previously used
 * ctx.browser?.withPage() work unchanged.
 *
 * ElectronPage wraps webContents.executeJavaScript() to provide the subset
 * of the Playwright Page API that the XING and Indeed scrapers actually use:
 *   goto / url / waitForSelector / locator / click / fill / waitForURL /
 *   waitForTimeout / setExtraHTTPHeaders
 *
 * Pages are opened in a hidden BrowserWindow and torn down after withPage()
 * returns — same lifecycle as Playwright pages.
 */
import { BrowserWindow, session, type Session } from 'electron';
import { createLogger } from '@ajh/core';

const logger = createLogger('electron-browser');

// ── ElectronLocator ───────────────────────────────────────────────────────────

/**
 * Minimal Playwright Locator-alike backed by executeJavaScript.
 * Only implements the methods XING/Indeed scrapers call.
 */
export class ElectronLocator {
  constructor(
    private readonly wc: Electron.WebContents,
    private readonly selector: string
  ) {}

  first(): ElectronLocator {
    return new ElectronLocator(this.wc, `__first__(${JSON.stringify(this.selector)})`);
  }

  all(): Promise<ElectronLocator[]> {
    const sel = this.selector;
    const wc = this.wc;
    return this.wc
      .executeJavaScript(
        `(function(){
          const els = Array.from(document.querySelectorAll(${JSON.stringify(sel)}));
          return els.length;
        })()`
      )
      .then((count: number) => {
        return Array.from(
          { length: count },
          (_, i) => new ElectronLocator(wc, `__nth__(${JSON.stringify(sel)},${i})`)
        );
      });
  }

  async innerText(): Promise<string> {
    return this._eval<string>('el => el.innerText || el.textContent || ""');
  }

  async getAttribute(name: string): Promise<string | null> {
    return this._eval<string | null>(`el => el.getAttribute(${JSON.stringify(name)})`);
  }

  async isVisible(opts?: { timeout?: number }): Promise<boolean> {
    try {
      const result = await Promise.race([
        this._eval<boolean>(
          'el => { const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0; }'
        ),
        new Promise<boolean>((r) => setTimeout(() => r(false), opts?.timeout ?? 3_000)),
      ]);
      return result;
    } catch {
      return false;
    }
  }

  async click(opts?: { timeout?: number }): Promise<void> {
    await this._eval<void>('el => el.click()', opts?.timeout);
  }

  locator(subSelector: string): ElectronLocator {
    // Build a compound selector string encoding parent + child so _resolve can handle it.
    return new ElectronLocator(
      this.wc,
      `__child__(${JSON.stringify(this.selector)},${JSON.stringify(subSelector)})`
    );
  }

  // ── internal ────────────────────────────────────────────────────────────────

  private async _eval<T>(fn: string, _timeout?: number): Promise<T> {
    const sel = this.selector;
    const script = ElectronLocator._buildScript(sel, fn);
    return this.wc.executeJavaScript(script);
  }

  private static _buildScript(selector: string, fn: string): string {
    // Handle the three encoded selector forms.
    if (selector.startsWith('__first__(')) {
      const inner = selector.slice('__first__('.length, -1);
      const s = JSON.parse(inner) as string;
      return `(function(){
        const el = document.querySelector(${JSON.stringify(s)});
        if (!el) return null;
        return (${fn})(el);
      })()`;
    }
    if (selector.startsWith('__nth__(')) {
      const inner = selector.slice('__nth__('.length, -1);
      const commaIdx = inner.lastIndexOf(',');
      const s = JSON.parse(inner.slice(0, commaIdx)) as string;
      const idx = Number(inner.slice(commaIdx + 1));
      return `(function(){
        const el = document.querySelectorAll(${JSON.stringify(s)})[${idx}];
        if (!el) return null;
        return (${fn})(el);
      })()`;
    }
    if (selector.startsWith('__child__(')) {
      const inner = selector.slice('__child__('.length, -1);
      const commaIdx = inner.indexOf(',');
      const parentSel = JSON.parse(inner.slice(0, commaIdx)) as string;
      const childSel = JSON.parse(inner.slice(commaIdx + 1)) as string;
      // Resolve parent first using _buildScript, then querySelector from it.
      return `(function(){
        ${ElectronLocator._buildScript(
          parentSel,
          `
          parent => {
            const el = parent ? parent.querySelector(${JSON.stringify(childSel)}) : null;
            if (!el) return null;
            return (${fn})(el);
          }
        `
        )}
      })()`;
    }
    // Plain CSS selector.
    return `(function(){
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return null;
      return (${fn})(el);
    })()`;
  }
}

// ── ElectronPage ──────────────────────────────────────────────────────────────

/**
 * Playwright Page-alike backed by a hidden Electron BrowserWindow.
 */
export class ElectronPage {
  constructor(private readonly win: BrowserWindow) {}

  get webContents(): Electron.WebContents {
    return this.win.webContents;
  }

  async goto(
    url: string,
    opts?: { waitUntil?: 'domcontentloaded' | 'load' | 'networkidle'; timeout?: number }
  ): Promise<void> {
    const timeout = opts?.timeout ?? 30_000;
    await Promise.race([
      new Promise<void>((resolve, reject) => {
        const waitForLoad = opts?.waitUntil === 'load';
        const handler = () => resolve();
        if (waitForLoad) {
          this.win.webContents.once('did-finish-load', handler);
          this.win.webContents.once('did-fail-load', (_, code, desc) => {
            this.win.webContents.removeListener('did-finish-load', handler);
            reject(new Error(`Navigation failed: ${desc} (${code})`));
          });
        } else {
          this.win.webContents.once('dom-ready', handler);
          this.win.webContents.once('did-fail-load', (_, code, desc) => {
            this.win.webContents.removeListener('dom-ready', handler);
            reject(new Error(`Navigation failed: ${desc} (${code})`));
          });
        }
        this.win.webContents.loadURL(url).catch(reject);
      }),
      new Promise<void>((_, reject) =>
        setTimeout(() => reject(new Error(`goto timeout (${timeout}ms): ${url}`)), timeout)
      ),
    ]);
  }

  url(): string {
    return this.win.webContents.getURL();
  }

  async waitForSelector(selector: string, opts?: { timeout?: number }): Promise<ElectronLocator> {
    const timeout = opts?.timeout ?? 10_000;
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const found = await this.win.webContents.executeJavaScript(
        `!!document.querySelector(${JSON.stringify(selector)})`
      );
      if (found) return new ElectronLocator(this.win.webContents, selector);
      await this.waitForTimeout(100);
    }
    throw new Error(`waitForSelector timeout: ${selector}`);
  }

  locator(selector: string): ElectronLocator {
    return new ElectronLocator(this.win.webContents, selector);
  }

  async fill(selector: string, value: string): Promise<void> {
    await this.win.webContents.executeJavaScript(`
      (function(){
        const el = document.querySelector(${JSON.stringify(selector)});
        if (!el) return;
        const nativeSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
        if (nativeSetter) nativeSetter.call(el, ${JSON.stringify(value)});
        else el.value = ${JSON.stringify(value)};
        el.dispatchEvent(new Event('input', { bubbles: true }));
        el.dispatchEvent(new Event('change', { bubbles: true }));
      })()
    `);
  }

  async click(selector: string): Promise<void> {
    await this.win.webContents.executeJavaScript(
      `document.querySelector(${JSON.stringify(selector)})?.click()`
    );
  }

  async waitForURL(pattern: RegExp | string, opts?: { timeout?: number }): Promise<void> {
    const timeout = opts?.timeout ?? 30_000;
    const deadline = Date.now() + timeout;
    const re = typeof pattern === 'string' ? new RegExp(pattern) : pattern;
    while (Date.now() < deadline) {
      if (re.test(this.win.webContents.getURL())) return;
      await this.waitForTimeout(200);
    }
    throw new Error(`waitForURL timeout: ${pattern}`);
  }

  async waitForFunction(fn: string, _arg?: unknown, opts?: { timeout?: number }): Promise<void> {
    const timeout = opts?.timeout ?? 30_000;
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const result = await this.win.webContents
        .executeJavaScript(`(function(){ return (${fn})(); })()`)
        .catch(() => false);
      if (result) return;
      await this.waitForTimeout(200);
    }
    // Resolve silently on timeout (matches Playwright .catch(() => {}) patterns).
  }

  waitForTimeout(ms: number): Promise<void> {
    return new Promise((r) => setTimeout(r, ms));
  }

  async setExtraHTTPHeaders(_headers: Record<string, string>): Promise<void> {
    // Electron doesn't have a direct per-page header API; set User-Agent via
    // session or just no-op — scrapers work without extra headers in Electron.
  }

  async evaluate<T>(fn: (arg?: unknown) => T, arg?: unknown): Promise<T> {
    const argStr = arg !== undefined ? JSON.stringify(arg) : 'undefined';
    return this.win.webContents.executeJavaScript(`(${fn.toString()})(${argStr})`);
  }
}

// ── ElectronBrowserController ─────────────────────────────────────────────────

export interface WithPageOptions {
  persistentStateDir?: string;
  boardId?: string;
}

/**
 * Drop-in replacement for BrowserController (packages/data/src/scraping/browser.ts).
 *
 * The interface matches what scrapers receive as ctx.browser — withPage() is
 * the only method scrapers call. isOpen() and inFlight() are used for health
 * checks; close() / closePersistent() are called on shutdown.
 */
export class ElectronBrowserController {
  private pagesOut = 0;

  async withPage<T>(
    fn: (page: ElectronPage) => Promise<T>,
    opts: WithPageOptions = {}
  ): Promise<T> {
    const partition = opts.boardId
      ? `persist:board-${opts.boardId}`
      : `ephemeral:scrape:${Date.now()}`;

    let sess: Session;
    if (opts.boardId) {
      // Reuse the persistent session that was created during the connect flow.
      sess = session.fromPartition(partition, { cache: true });
    } else {
      sess = session.fromPartition(partition, { cache: false });
    }

    const win = new BrowserWindow({
      show: false,
      width: 1366,
      height: 900,
      webPreferences: {
        session: sess,
        contextIsolation: false, // allow executeJavaScript in main world
        nodeIntegration: false,
        sandbox: false,
        backgroundThrottling: false,
      },
    });

    // Anti-bot: present as a real desktop Chrome.
    win.webContents.setUserAgent(
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ' +
        '(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36'
    );
    // Suppress automation flag readable by JS fingerprinting.
    await win.webContents
      .executeJavaScript('Object.defineProperty(navigator, "webdriver", { get: () => undefined })')
      .catch(() => {});

    const page = new ElectronPage(win);
    this.pagesOut++;
    try {
      return await fn(page);
    } finally {
      this.pagesOut--;
      try {
        win.close();
      } catch {
        /* already closed */
      }
      logger.debug({ partition }, 'page closed');
    }
  }

  isOpen(): boolean {
    return true;
  }
  inFlight(): number {
    return this.pagesOut;
  }
  async close(): Promise<void> {
    /* windows close themselves in withPage() */
  }
  async closePersistent(): Promise<void> {
    /* sessions managed by Electron */
  }
}
