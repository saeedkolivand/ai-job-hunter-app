/**
 * Generic Board Session Manager
 *
 * Manages an authenticated Playwright persistent context per job board
 * (LinkedIn, Indeed, Xing, Glassdoor, …). The user manually completes login
 * once in a visible browser window; the persistent context is then stored on
 * disk and reused for subsequent scraping runs.
 *
 * Per-board on-disk layout:
 *   <userData>/board-sessions/<boardId>
 *
 * For LinkedIn there is also a legacy directory <userData>/linkedin-session
 * which we keep working transparently for users that connected before this
 * generalization landed.
 */
import fs from 'node:fs';
import path from 'node:path';

import { type BrowserContext, chromium } from 'playwright';

import { createLogger } from '@ajh/core';

export interface BoardSessionConfig {
  /** Stable board id (matches scraper.id). */
  id: string;
  /** Human-readable display name (used in logs only). */
  displayName: string;
  /** URL to load when the user clicks "Connect". Should be the board's login page. */
  loginUrl: string;
  /** URL we navigate to in order to validate that the user is signed in. */
  validateUrl: string;
  /**
   * Predicate that decides if the current URL implies a valid signed-in session.
   * Defaults to "URL no longer matches the login URL".
   */
  isAuthenticatedUrl?: (url: string) => boolean;
  /** Optional legacy directory to fall back to if `board-sessions/<id>` doesn't exist. */
  legacyDir?: string;
}

export interface BoardSessionStatus {
  connected: boolean;
  lastConnected?: number;
  accountEmail?: string;
  sessionPath?: string;
}

export class BoardSessionManager {
  private readonly logger;
  private context?: BrowserContext;
  private readonly sessionDir: string;
  private readonly legacyDir?: string;

  constructor(
    userDataDir: string,
    private readonly cfg: BoardSessionConfig
  ) {
    this.logger = createLogger(`board-session.${cfg.id}`);
    this.sessionDir = path.join(userDataDir, 'board-sessions', cfg.id);
    if (cfg.legacyDir) this.legacyDir = path.join(userDataDir, cfg.legacyDir);
  }

  /** Returns the active on-disk session directory (legacy or current). */
  resolveSessionDir(): string {
    if (this.legacyDir && fs.existsSync(this.legacyDir) && !fs.existsSync(this.sessionDir)) {
      return this.legacyDir;
    }
    return this.sessionDir;
  }

  hasSessionOnDisk(): boolean {
    return fs.existsSync(this.sessionDir) || (!!this.legacyDir && fs.existsSync(this.legacyDir));
  }

  /** Launch a visible browser so the user can complete login manually. */
  async connect(): Promise<BoardSessionStatus> {
    this.logger.info({ board: this.cfg.id }, 'starting connect flow');
    await this.disconnect();

    this.context = await chromium.launchPersistentContext(this.sessionDir, {
      headless: false,
      viewport: { width: 1366, height: 900 },
      userAgent:
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36',
      locale: 'en-US',
      timezoneId: 'Europe/Berlin',
      args: ['--disable-blink-features=AutomationControlled'],
    });

    const pages = this.context.pages();
    const page = pages.length > 0 ? pages[0] : await this.context.newPage();
    if (!page) throw new Error('failed to create page');

    await page.goto(this.cfg.loginUrl, { waitUntil: 'domcontentloaded', timeout: 60_000 });

    // Wait up to 5 minutes for the user to land on a non-login URL.
    const isAuthed =
      this.cfg.isAuthenticatedUrl ??
      ((u: string) => !u.includes('/login') && !u.includes('/auth') && !u.includes('/signin'));
    await page
      .waitForFunction(
        (loginUrl: string) => {
          const u = window.location.href;
          // Loose check executed in the page; mirrors `isAuthed` defaults.
          return (
            u !== loginUrl &&
            !u.includes('/login') &&
            !u.includes('/auth') &&
            !u.includes('/signin')
          );
        },
        this.cfg.loginUrl,
        { timeout: 300_000 }
      )
      .catch(() => {});

    const status = await this.validateSession(isAuthed);
    if (status.connected) {
      this.logger.info({ board: this.cfg.id }, 'connect successful');
      // Export cookies to state.json for scrapers to read
      await this.exportCookies();
      await this.context.close().catch(() => {});
      this.context = undefined;
      return status;
    }
    this.logger.warn({ board: this.cfg.id }, 'connect validation failed');
    await this.disconnect();
    throw new Error(`${this.cfg.displayName} authentication failed`);
  }

  async validateSession(isAuthed?: (url: string) => boolean): Promise<BoardSessionStatus> {
    if (!this.context) return { connected: false };
    try {
      const pages = this.context.pages();
      const page = pages.length > 0 ? pages[0] : await this.context.newPage();
      if (!page) return { connected: false };

      await page.goto(this.cfg.validateUrl, { waitUntil: 'domcontentloaded', timeout: 30_000 });
      const url = page.url();
      const ok = (
        isAuthed ??
        this.cfg.isAuthenticatedUrl ??
        ((u) => !u.includes('/login') && !u.includes('/auth') && !u.includes('/signin'))
      )(url);
      if (!ok) {
        this.logger.warn({ board: this.cfg.id, url }, 'session not authenticated');
        return { connected: false };
      }
      return { connected: true, lastConnected: Date.now(), sessionPath: this.sessionDir };
    } catch (err) {
      this.logger.error({ board: this.cfg.id, err }, 'validate failed');
      return { connected: false };
    }
  }

  /**
   * Cheap status check that doesn't launch a browser. We trust the on-disk
   * session unless the actual scraper run discovers it has expired.
   */
  async getStatus(): Promise<BoardSessionStatus> {
    if (this.context) return this.validateSession();
    return this.hasSessionOnDisk()
      ? { connected: true, sessionPath: this.resolveSessionDir() }
      : { connected: false };
  }

  async disconnect(): Promise<void> {
    if (this.context) {
      this.logger.info({ board: this.cfg.id }, 'closing context');
      await this.context.close().catch(() => {});
      this.context = undefined;
    }
  }

  /**
   * Export cookies from the current context to a state.json file.
   * This allows scrapers to read the session data without launching a browser.
   */
  private async exportCookies(): Promise<void> {
    if (!this.context) return;
    try {
      const cookies = await this.context.cookies();
      const fs = await import('node:fs/promises');
      const statePath = path.join(this.sessionDir, 'state.json');
      await fs.writeFile(statePath, JSON.stringify({ cookies }, null, 2));
      this.logger.info(
        { board: this.cfg.id, cookieCount: cookies.length },
        'exported cookies to state.json'
      );
    } catch (error) {
      this.logger.error({ board: this.cfg.id, error }, 'failed to export cookies');
    }
  }

  /** Wipe the session directory(ies) so the next connect starts fresh. */
  async reset(): Promise<void> {
    await this.disconnect();
    const fsp = await import('node:fs/promises');
    await fsp.rm(this.sessionDir, { recursive: true, force: true }).catch(() => {});
    if (this.legacyDir) {
      await fsp.rm(this.legacyDir, { recursive: true, force: true }).catch(() => {});
    }
  }

  async close(): Promise<void> {
    await this.disconnect();
  }
}

/**
 * Built-in configurations for the boards we support today. Adding a new
 * board only requires registering a config here (and a scraper that calls
 * `boardSessions.get(id).resolveSessionDir()` for its persistent context).
 */
export const BOARD_SESSION_CONFIGS: Record<string, BoardSessionConfig> = {
  linkedin: {
    id: 'linkedin',
    displayName: 'LinkedIn',
    loginUrl: 'https://www.linkedin.com/login',
    validateUrl: 'https://www.linkedin.com/feed/',
    isAuthenticatedUrl: (u) =>
      !u.includes('/login') && !u.includes('/uas/login') && !u.includes('/checkpoint/lg'),
    legacyDir: 'linkedin-session',
  },
  indeed: {
    id: 'indeed',
    displayName: 'Indeed',
    loginUrl: 'https://secure.indeed.com/auth',
    validateUrl: 'https://secure.indeed.com/account/view',
    isAuthenticatedUrl: (u) => !u.includes('/auth') && !u.includes('/account/login'),
  },
  xing: {
    id: 'xing',
    displayName: 'Xing',
    loginUrl: 'https://login.xing.com/login',
    validateUrl: 'https://www.xing.com/jobs/find',
    isAuthenticatedUrl: (u) => !u.includes('login.xing.com') && !u.includes('/login'),
  },
  glassdoor: {
    id: 'glassdoor',
    displayName: 'Glassdoor',
    loginUrl: 'https://www.glassdoor.com/profile/login_input.htm',
    validateUrl: 'https://www.glassdoor.com/member/home/index.htm',
    isAuthenticatedUrl: (u) => !u.includes('/profile/login') && !u.includes('/index.htm?sso'),
  },
};
