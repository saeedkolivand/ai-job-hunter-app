/**
 * LoginManager — opens a headed Playwright browser for board authentication.
 *
 * Replaces Electron's PersistentBoardSession.connect() / getStatus() /
 * disconnect() without depending on any Electron API.
 *
 * ── Session persistence ───────────────────────────────────────────────────────
 * Uses Playwright's `launchPersistentContext(statePath)` — the same role as
 * Electron's `session.fromPartition("persist:<boardId>")`. Cookies and storage
 * are written to disk automatically by Playwright whenever the context is closed.
 * statePath = FileCredentialStore.storageStatePath(boardId) = ~/.ajh/browser-state/<boardId>/
 *
 * ── Auth detection ────────────────────────────────────────────────────────────
 * Ports the same URL-based and cookie-based strategies from Electron's configs.ts
 * using Playwright's Cookie type instead of Electron.Cookie.
 *
 * ── Status persistence ───────────────────────────────────────────────────────
 * After successful login, writes <statePath>/auth-status.json so getStatus()
 * can answer without opening a browser. disconnect() removes this file and
 * deletes the persistent context directory.
 */
import fs from 'node:fs';
import path from 'node:path';

import { type BrowserContext, chromium, type Page } from 'playwright';

import type { FileCredentialStore } from './credentials.js';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface BoardLoginConfig {
  id: string;
  displayName: string;
  loginUrl: string;
  blockPasskeys?: boolean;
  isAuthenticatedUrl?: (url: string) => boolean;
  isAuthenticatedCookies?: (
    cookies: PlaywrightCookie[],
    baseline: PlaywrightCookie[] | undefined
  ) => boolean;
}

// Subset of Playwright's Cookie type — only what auth detection needs.
interface PlaywrightCookie {
  name: string;
  value: string;
  domain: string;
}

interface AuthStatus {
  connected: boolean;
  connectedAt?: number;
}

// ── Board configs ─────────────────────────────────────────────────────────────
// Ported from apps/desktop/src/main/board-sessions/configs.ts

const DEFAULT_IS_AUTHED_URL = (url: string) =>
  !url.includes('/login') &&
  !url.includes('/auth') &&
  !url.includes('/signin') &&
  !url.includes('/checkpoint') &&
  !url.includes('/uas/');

export const BOARD_CONFIGS: Record<string, BoardLoginConfig> = {
  linkedin: {
    id: 'linkedin',
    displayName: 'LinkedIn',
    loginUrl: 'https://www.linkedin.com/login',
    blockPasskeys: true,
    isAuthenticatedCookies: (cookies) => {
      const liAt = cookies.find((c) => c.name === 'li_at' && c.domain?.includes('linkedin.com'));
      return !!liAt && liAt.value.length > 10;
    },
  },

  indeed: {
    id: 'indeed',
    displayName: 'Indeed',
    loginUrl: 'https://secure.indeed.com/auth',
    isAuthenticatedCookies: (cookies, baseline) => {
      if (!baseline) return false;
      const seen = new Set(baseline.map((c) => `${c.name}\x00${c.domain}`));
      return cookies.some(
        (c) =>
          c.domain?.includes('indeed.com') &&
          !seen.has(`${c.name}\x00${c.domain}`) &&
          c.value.length > 40
      );
    },
  },

  xing: {
    id: 'xing',
    displayName: 'Xing',
    loginUrl: 'https://login.xing.com/login',
    isAuthenticatedUrl: (u) =>
      u.includes('xing.com') && !u.includes('login.xing.com') && !u.includes('/login'),
  },

  glassdoor: {
    id: 'glassdoor',
    displayName: 'Glassdoor',
    loginUrl: 'https://www.glassdoor.com/profile/login_input.htm',
    isAuthenticatedUrl: (u) =>
      u.includes('glassdoor.com') && !u.includes('/profile/login') && !u.includes('/index.htm?sso'),
  },
};

// Script injected to block WebAuthn/passkey prompts (LinkedIn only).
const DISABLE_PASSKEY_SCRIPT = `
(function () {
  try {
    const orig = navigator.credentials;
    if (!orig) return;
    const origGet = orig.get.bind(orig);
    const origCreate = orig.create.bind(orig);
    const notAllowed = () =>
      Promise.reject(
        Object.assign(new DOMException('User cancelled', 'NotAllowedError'), { code: 20 })
      );
    Object.defineProperty(navigator, 'credentials', {
      configurable: true,
      get: () => ({
        get:  (o) => (o?.publicKey ? notAllowed() : origGet(o)),
        create: (o) => (o?.publicKey ? notAllowed() : origCreate(o)),
        store: orig.store.bind(orig),
        preventSilentAccess: orig.preventSilentAccess.bind(orig),
      }),
    });
  } catch (_) {}
})();
`.trim();

// ── Auth detection ────────────────────────────────────────────────────────────

async function waitForAuth(
  context: BrowserContext,
  page: Page,
  config: BoardLoginConfig,
  timeoutMs = 300_000
): Promise<boolean> {
  return new Promise<boolean>((resolve) => {
    let resolved = false;
    let baseline: PlaywrightCookie[] | undefined;
    let pollInterval: ReturnType<typeof setInterval> | undefined;

    const finish = (connected: boolean) => {
      if (resolved) return;
      resolved = true;
      if (pollInterval) clearInterval(pollInterval);
      resolve(connected);
    };

    // Timeout safety net.
    const timer = setTimeout(() => finish(false), timeoutMs);

    const checkCookies = async () => {
      if (resolved || !config.isAuthenticatedCookies) return;
      const cookies = (await context.cookies()) as PlaywrightCookie[];
      if (config.isAuthenticatedCookies(cookies, baseline)) {
        clearTimeout(timer);
        finish(true);
      }
    };

    const handleUrl = (url: string) => {
      if (resolved) return;
      void checkCookies();
      const isAuthedUrl = config.isAuthenticatedUrl ?? DEFAULT_IS_AUTHED_URL;
      if (!config.isAuthenticatedCookies && isAuthedUrl(url)) {
        clearTimeout(timer);
        finish(true);
      }
    };

    // Baseline: snapshot cookies after first page render.
    page.once('load', () => {
      void context.cookies().then((c: PlaywrightCookie[]) => {
        baseline = c;
        void checkCookies(); // user may already be logged in
      });
    });

    // URL-based detection.
    page.on('framenavigated', (frame: { url: () => string }) => {
      if ((frame as unknown) === page.mainFrame()) handleUrl(frame.url());
    });

    // Cookie polling for AJAX-login boards (Indeed).
    if (config.isAuthenticatedCookies) {
      pollInterval = setInterval(() => void checkCookies(), 1_000);
    }

    // User closed the window.
    context.once('close', () => {
      clearTimeout(timer);
      finish(false);
    });
  });
}

// ── LoginManager ──────────────────────────────────────────────────────────────

export class LoginManager {
  constructor(private readonly credentials: FileCredentialStore) {}

  /**
   * Open a headed Playwright browser for the board's login flow.
   * Returns when the user successfully authenticates or the window is closed.
   * The persistent context is saved to disk either way.
   */
  async openLogin(
    boardId: string,
    onStatus?: (msg: string) => void
  ): Promise<{ connected: boolean }> {
    const config = BOARD_CONFIGS[boardId];
    if (!config) throw new Error(`No login config for board: ${boardId}`);

    const statePath = this.credentials.storageStatePath(boardId);
    onStatus?.(`Opening ${config.displayName} login window…`);

    // launchPersistentContext keeps cookies on disk automatically.
    const context = await chromium.launchPersistentContext(statePath, {
      headless: false,
      args: ['--disable-blink-features=AutomationControlled'],
      ignoreDefaultArgs: ['--enable-automation'],
    });

    const page = await context.newPage();

    if (config.blockPasskeys) {
      await page.addInitScript(DISABLE_PASSKEY_SCRIPT);
    }

    await page.goto(config.loginUrl).catch(() => {});

    const connected = await waitForAuth(context, page, config);

    // Export storage state (cookies + localStorage) to state.json so the
    // HTTP scraper can read session cookies without needing a browser.
    if (connected) {
      const stateFile = path.join(statePath, 'state.json');
      await context.storageState({ path: stateFile }).catch(() => {});
    }

    await context.close().catch(() => {});

    this.writeAuthStatus(boardId, connected);
    onStatus?.(connected ? 'Login successful' : 'Login cancelled or timed out');

    return { connected };
  }

  /** Check whether this board has a stored authenticated session. */
  getStatus(boardId: string): { connected: boolean } {
    const statePath = this.credentials.storageStatePath(boardId);
    const statusFile = path.join(statePath, 'auth-status.json');
    try {
      const raw = fs.readFileSync(statusFile, 'utf8');
      return JSON.parse(raw) as AuthStatus;
    } catch {
      return { connected: false };
    }
  }

  /** Clear the board's session — user will need to log in again. */
  disconnect(boardId: string): void {
    const statePath = this.credentials.storageStatePath(boardId);
    try {
      fs.rmSync(statePath, { recursive: true, force: true });
    } catch {
      /* ignore */
    }
  }

  private writeAuthStatus(boardId: string, connected: boolean): void {
    const statePath = this.credentials.storageStatePath(boardId);
    fs.mkdirSync(statePath, { recursive: true });
    const status: AuthStatus = { connected, connectedAt: connected ? Date.now() : undefined };
    fs.writeFileSync(path.join(statePath, 'auth-status.json'), JSON.stringify(status));
  }
}
