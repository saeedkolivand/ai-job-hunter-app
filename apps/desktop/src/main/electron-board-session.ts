/**
 * Electron-native board authentication.
 *
 * Opens a BrowserWindow (Electron's own bundled Chromium — no external
 * dependency) at the board's login URL, waits for the user to sign in, then
 * extracts cookies and writes them to state.json so the HTTP-based scrapers
 * can pick them up immediately.
 *
 * ── Auth detection strategies ─────────────────────────────────────────────
 *
 * LinkedIn  — li_at cookie (set on full-page navigation after login).
 *             Detected via session.cookies.on('changed') + 1 s poll.
 *
 * Indeed    — AJAX login: clicking Sign In posts credentials and sets auth
 *             cookies WITHOUT navigating the page. We snapshot the cookies
 *             present when the page first loads (baseline), then watch
 *             session.cookies.on('changed') for new cookies that weren't
 *             in that baseline. ≥2 new indeed.com cookies = auth complete.
 *             No reload, no polling, no interference with form filling.
 *
 * Others    — URL-based: detect when the browser leaves the auth domain.
 *
 * ── Passkey / WebAuthn blocking ───────────────────────────────────────────
 * LinkedIn prompts for passkeys via navigator.credentials.get({publicKey}).
 * We override that in the page's main world on every dom-ready to reject
 * only publicKey requests, letting password autofill pass through normally.
 */
import fs from 'node:fs/promises';
import path from 'node:path';

import { BrowserWindow, type Session } from 'electron';

import { createLogger } from '@ajh/core';

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
        get: (opts) => (opts && opts.publicKey ? notAllowed() : origGet(opts)),
        create: (opts) => (opts && opts.publicKey ? notAllowed() : origCreate(opts)),
        store: orig.store.bind(orig),
        preventSilentAccess: orig.preventSilentAccess.bind(orig),
      }),
    });
  } catch (_) {}
})();
`.trim();

export interface ElectronBoardSessionConfig {
  id: string;
  displayName: string;
  loginUrl: string;
  /** Only enable on boards that actively prompt for passkeys (LinkedIn). */
  blockPasskeys?: boolean;
  /**
   * Cookie-based auth check.
   * @param cookies  All current cookies in the session.
   * @param baseline Cookies present when the page first loaded (pre-auth).
   *                 Undefined until the first did-finish-load fires.
   *                 Return false when baseline is undefined to wait for it.
   */
  isAuthenticatedCookies?: (
    cookies: Electron.Cookie[],
    baseline: Electron.Cookie[] | undefined
  ) => boolean;
  /**
   * URL-based fallback. Used when isAuthenticatedCookies is not defined.
   * Return true when the URL indicates the user has left the auth flow.
   */
  isAuthenticatedUrl?: (url: string) => boolean;
}

const DEFAULT_IS_AUTHED_URL = (url: string) =>
  !url.includes('/login') &&
  !url.includes('/auth') &&
  !url.includes('/signin') &&
  !url.includes('/checkpoint') &&
  !url.includes('/uas/');

export const ELECTRON_BOARD_SESSION_CONFIGS: Record<string, ElectronBoardSessionConfig> = {
  linkedin: {
    id: 'linkedin',
    displayName: 'LinkedIn',
    loginUrl: 'https://www.linkedin.com/login',
    blockPasskeys: true,
    // li_at is LinkedIn's canonical session cookie — set once, after full auth.
    // baseline is ignored; li_at is never present pre-login.
    isAuthenticatedCookies: (cookies) => {
      const liAt = cookies.find((c) => c.name === 'li_at' && c.domain?.includes('linkedin.com'));
      return !!liAt && liAt.value.length > 10;
    },
  },

  indeed: {
    id: 'indeed',
    displayName: 'Indeed',
    loginUrl: 'https://secure.indeed.com/auth',
    // Indeed sends 7+ tracking cookies via HTTP redirect headers before the
    // login page renders — we can't use a raw cookie count. Instead we
    // snapshot the baseline on first page load and detect auth when ≥2 NEW
    // indeed.com cookies appear (the auth token set by the AJAX response).
    // No reload needed: session.cookies.on('changed') fires instantly.
    isAuthenticatedCookies: (cookies, baseline) => {
      if (!baseline) return false; // wait for baseline snapshot
      const seen = new Set(baseline.map((c) => `${c.name}\x00${c.domain}`));
      const newIndeed = cookies.filter(
        (c) => c.domain?.includes('indeed.com') && !seen.has(`${c.name}\x00${c.domain}`)
      );
      return newIndeed.length >= 2;
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

export class ElectronBoardSessionManager {
  private readonly logger;
  private readonly sessionDir: string;
  private loginWindow?: BrowserWindow;

  constructor(
    private readonly userDataDir: string,
    private readonly cfg: ElectronBoardSessionConfig
  ) {
    this.logger = createLogger(`electron-session.${cfg.id}`);
    this.sessionDir = path.join(userDataDir, 'board-sessions', cfg.id);
  }

  async connect(): Promise<{ connected: boolean; accountEmail?: string }> {
    this.logger.info({ board: this.cfg.id }, 'starting Electron connect flow');

    if (this.loginWindow && !this.loginWindow.isDestroyed()) {
      this.loginWindow.close();
      this.loginWindow = undefined;
    }

    const { session } = await import('electron');
    const partition = `login:${this.cfg.id}:${Date.now()}`;
    const sess: Session = session.fromPartition(partition, { cache: false });

    this.loginWindow = new BrowserWindow({
      width: 1366,
      height: 900,
      title: `Sign in to ${this.cfg.displayName}`,
      webPreferences: {
        session: sess,
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: false,
      },
    });
    // webRTCIPHandlingPolicy is set on session to suppress STUN DNS error noise in logs
    this.loginWindow.webContents.session.setWebRTCIPHandlingPolicy('disable_non_proxied_udp');

    return new Promise<{ connected: boolean; accountEmail?: string }>((resolve) => {
      let resolved = false;
      let cookiePollTimer: ReturnType<typeof setInterval> | undefined;
      // Baseline snapshot: cookies present before the user interacts.
      // Captured after the first did-finish-load so we exclude HTTP-redirect
      // tracking cookies that arrive before the page even renders.
      let baseline: Electron.Cookie[] | undefined;

      const finish = (connected: boolean) => {
        if (resolved) return;
        resolved = true;
        if (cookiePollTimer) clearInterval(cookiePollTimer);
        // Remove cookie change listener
        sess.cookies.removeAllListeners('changed');
        if (this.loginWindow && !this.loginWindow.isDestroyed()) {
          this.loginWindow.close();
        }
        resolve({ connected });
      };

      const saveAndFinish = async () => {
        if (resolved) return;
        try {
          const cookies = await sess.cookies.get({});
          await this.writeCookies(cookies);
          this.logger.info(
            { board: this.cfg.id, cookieCount: cookies.length },
            'connect successful — cookies saved'
          );
        } catch (err) {
          this.logger.error({ err }, 'failed to write state.json — resolving anyway');
        }
        finish(true);
      };

      // ── Cookie-based detection ────────────────────────────────────────────
      const checkCookies = async () => {
        if (resolved || !this.cfg.isAuthenticatedCookies) return;
        const cookies = await sess.cookies.get({}).catch(() => [] as Electron.Cookie[]);
        if (this.cfg.isAuthenticatedCookies(cookies, baseline)) {
          this.logger.info({ board: this.cfg.id }, 'auth cookie detected — completing connect');
          await saveAndFinish();
        }
      };

      if (this.cfg.isAuthenticatedCookies) {
        // Real-time: fires the instant any cookie is created or modified.
        // Catches AJAX-based logins (e.g. Indeed) without any page reload.
        sess.cookies.on('changed', (_e, _cookie, _cause, removed) => {
          if (!removed) void checkCookies();
        });
        // Backup poll — catches cases where cookies were set before the
        // listener was attached or via non-standard mechanisms.
        cookiePollTimer = setInterval(() => void checkCookies(), 1_000);
      }

      // ── Baseline snapshot (for Indeed / boards with AJAX login) ───────────
      // Capture loginWindow in a local const so callbacks remain valid even
      // if finish() clears this.loginWindow before they fire.
      const win = this.loginWindow;
      if (!win) {
        finish(false);
        return;
      }
      const wc = win.webContents;
      wc.once('did-finish-load', () => {
        if (baseline !== undefined || resolved) return;
        void sess.cookies.get({}).then((cookies) => {
          baseline = cookies;
          this.logger.debug(
            { board: this.cfg.id, baselineCount: cookies.length },
            'baseline cookies captured'
          );
        });
      });

      // ── URL-based detection (fallback for non-cookie boards) ──────────────
      const isAuthedUrl = this.cfg.isAuthenticatedUrl ?? DEFAULT_IS_AUTHED_URL;
      const handleUrl = (url: string) => {
        if (resolved) return;
        void checkCookies();
        if (!this.cfg.isAuthenticatedCookies && isAuthedUrl(url)) {
          void saveAndFinish();
        }
      };

      // ── Passkey blocking (LinkedIn only) ──────────────────────────────────
      if (this.cfg.blockPasskeys) {
        wc.on('dom-ready', () => {
          void this.loginWindow?.webContents
            .executeJavaScript(DISABLE_PASSKEY_SCRIPT)
            .catch(() => {});
        });
      }

      wc.on('did-navigate', (_, url) => handleUrl(url));
      wc.on('did-redirect-navigation', (_, url) => handleUrl(url));
      wc.on('did-navigate-in-page', (_, url) => handleUrl(url));

      win.on('closed', () => {
        this.loginWindow = undefined;
        if (!resolved) finish(false);
      });

      win.loadURL(this.cfg.loginUrl).catch((err) => {
        this.logger.error({ err }, 'failed to load login URL');
        finish(false);
      });
    });
  }

  private async writeCookies(cookies: Electron.Cookie[]): Promise<void> {
    await fs.mkdir(this.sessionDir, { recursive: true });
    const state = {
      cookies: cookies.map((c) => ({
        name: c.name,
        value: c.value,
        domain: c.domain ?? '',
        path: c.path ?? '/',
        expires: c.expirationDate ?? -1,
        httpOnly: c.httpOnly ?? false,
        secure: c.secure ?? false,
        sameSite: c.sameSite ?? 'Lax',
      })),
    };
    await fs.writeFile(
      path.join(this.sessionDir, 'state.json'),
      JSON.stringify(state, null, 2),
      'utf-8'
    );
  }

  async close(): Promise<void> {
    if (this.loginWindow && !this.loginWindow.isDestroyed()) {
      this.loginWindow.close();
      this.loginWindow = undefined;
    }
  }
}
