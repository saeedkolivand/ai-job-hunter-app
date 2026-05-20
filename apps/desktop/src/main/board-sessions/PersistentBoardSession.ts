/**
 * PersistentBoardSession
 *
 * Manages one board's authenticated Chromium session using a dedicated
 * `persist:<id>` Electron partition. Chromium stores all cookies,
 * localStorage, and cached data under <userData>/Partitions/persist_<id>/
 * automatically — the user logs in once and the session survives restarts.
 *
 * ── Responsibilities ────────────────────────────────────────────────────────
 *  connect()       Open a visible login BrowserWindow. Detect auth via cookie
 *                  change events or URL navigation. Close window + export
 *                  state.json (for scraper compatibility) when done.
 *
 *  getStatus()     Check the persistent session without opening any window.
 *                  Returns connected:true if auth cookies are present.
 *
 *  disconnect()    Clear all session data (cookies, storage, cache).
 *
 *  exportCookies() Write <boardId>/state.json for the HTTP-based scrapers
 *                  that still read cookies from disk. Called after every
 *                  successful connect AND on app startup if already logged in.
 *
 *  getSession()    Return the Electron Session object so callers can use
 *                  net.request({ session }) for authenticated HTTP calls.
 *
 * ── Security model ──────────────────────────────────────────────────────────
 *  - contextIsolation: true in login window (renderer is untrusted)
 *  - sandbox: false — login pages need full browser APIs (captcha, etc.)
 *  - nodeIntegration: false
 *  - No credentials stored in plaintext; session cookies live in Chromium's
 *    encrypted cookie store (OS keychain-backed on supported platforms)
 *  - Login window has no preload — completely isolated from app IPC
 */

import { BrowserWindow, session, type Session } from 'electron';
import path from 'node:path';
import fs from 'node:fs/promises';
import { createLogger } from '@ajh/core';
import { BOARD_CONFIGS, DISABLE_PASSKEY_SCRIPT } from './configs.js';
import type { BoardConfig, BoardSessionStatus } from './types.js';

const DEFAULT_IS_AUTHED_URL = (url: string) =>
  !url.includes('/login') &&
  !url.includes('/auth') &&
  !url.includes('/signin') &&
  !url.includes('/checkpoint') &&
  !url.includes('/uas/');

export class PersistentBoardSession {
  private readonly logger;
  private readonly cfg: BoardConfig;
  private readonly stateDir: string;
  private loginWindow?: BrowserWindow;

  constructor(
    private readonly userDataDir: string,
    boardId: string
  ) {
    const cfg = BOARD_CONFIGS[boardId];
    if (!cfg) throw new Error(`No board config for: ${boardId}`);
    this.cfg = cfg;
    this.logger = createLogger(`board-session.${boardId}`);
    this.stateDir = path.join(userDataDir, 'board-sessions', boardId);
  }

  // ── Public API ──────────────────────────────────────────────────────────────

  /** The Electron Session for this board's partition. */
  getSession(): Session {
    return session.fromPartition(this.cfg.partition);
  }

  /**
   * Check whether the persistent session has valid auth cookies.
   * Does not open any window; reads from the on-disk Chromium profile.
   */
  async getStatus(): Promise<BoardSessionStatus> {
    const cookies = await this.getSession()
      .cookies.get({})
      .catch(() => []);
    if (this.cfg.isAuthenticatedCookies?.(cookies, cookies)) {
      return { connected: true };
    }
    // For URL-based boards, fall back to checking if state.json exists
    // (written after the last successful login).
    try {
      await fs.access(path.join(this.stateDir, 'state.json'));
      return { connected: true };
    } catch {
      return { connected: false };
    }
  }

  /**
   * Open the board's login page in a visible BrowserWindow.
   * The window uses the persistent `persist:<id>` partition so:
   *   - If the session is still valid, the user may already be logged in
   *     and the window closes automatically within a second.
   *   - If expired, the user logs in normally and the window closes once
   *     auth is detected.
   * After auth is confirmed, cookies are exported to state.json.
   */
  async connect(): Promise<BoardSessionStatus> {
    this.logger.info({ board: this.cfg.id }, 'starting connect flow');

    // Close any stale window from a previous attempt.
    if (this.loginWindow && !this.loginWindow.isDestroyed()) {
      this.loginWindow.close();
      this.loginWindow = undefined;
    }

    const sess = this.getSession();

    this.loginWindow = new BrowserWindow({
      width: 1366,
      height: 900,
      title: `Sign in to ${this.cfg.displayName}`,
      webPreferences: {
        session: sess,
        contextIsolation: true,
        nodeIntegration: false,
        // sandbox:false — login pages need full browser APIs (captcha, etc.)
        sandbox: false,
        // No preload — login window is fully isolated from app IPC
      },
      // Disable WebRTC to suppress STUN DNS error noise in logs.
    });
    // webRTCIPHandlingPolicy is set on session to suppress STUN DNS error noise in logs
    this.loginWindow.webContents.session.setWebRTCIPHandlingPolicy('disable_non_proxied_udp');

    return new Promise<BoardSessionStatus>((resolve) => {
      let resolved = false;
      let pollTimer: ReturnType<typeof setInterval> | undefined;
      // Baseline: cookies present before user interaction.
      // Captured after first page render to exclude HTTP-redirect tracking
      // cookies set before the login form even appears.
      let baseline: Electron.Cookie[] | undefined;

      const finish = async (connected: boolean) => {
        if (resolved) return;
        resolved = true;
        if (pollTimer) clearInterval(pollTimer);
        sess.cookies.removeAllListeners('changed');
        if (this.loginWindow && !this.loginWindow.isDestroyed()) {
          this.loginWindow.close();
          this.loginWindow = undefined;
        }
        if (connected) {
          // Export cookies for scraper compatibility (state.json).
          await this.exportCookies().catch((err) =>
            this.logger.error({ err }, 'exportCookies failed')
          );
          this.logger.info({ board: this.cfg.id }, 'connect successful');
        }
        resolve({ connected });
      };

      // ── Cookie-based detection ────────────────────────────────────────────
      const checkCookies = async () => {
        if (resolved || !this.cfg.isAuthenticatedCookies) return;
        const cookies = await sess.cookies.get({}).catch(() => [] as Electron.Cookie[]);
        if (this.cfg.isAuthenticatedCookies(cookies, baseline)) {
          await finish(true);
        }
      };

      if (this.cfg.isAuthenticatedCookies) {
        // Real-time detection: fires the instant any cookie is set/modified,
        // including from AJAX response headers (e.g. Indeed's AJAX login).
        sess.cookies.on('changed', (_e, _c, _cause, removed) => {
          if (!removed) void checkCookies();
        });
        // Backup poll to catch cookies set before listener attached.
        pollTimer = setInterval(() => void checkCookies(), 1_000);
      }

      // ── Baseline snapshot ─────────────────────────────────────────────────
      // Captured once after the first full page render.
      // Capture loginWindow in a local const — this.loginWindow may be cleared
      // by finish() before the async callbacks fire.
      const win = this.loginWindow;
      if (!win) {
        void finish(false);
        return;
      }
      const wc = win.webContents;
      wc.once('did-finish-load', () => {
        if (baseline !== undefined || resolved) return;
        void sess.cookies.get({}).then((c) => {
          baseline = c;
          this.logger.debug({ count: c.length }, 'baseline captured');
          // Run an immediate check — user may already be logged in
          // (persistent session from previous connect still valid).
          void checkCookies();
        });
      });

      // ── URL-based detection ───────────────────────────────────────────────
      const isAuthedUrl = this.cfg.isAuthenticatedUrl ?? DEFAULT_IS_AUTHED_URL;
      const handleUrl = (url: string) => {
        if (resolved) return;
        void checkCookies();
        if (!this.cfg.isAuthenticatedCookies && isAuthedUrl(url)) {
          void finish(true);
        }
      };

      // ── Passkey blocking (LinkedIn only) ──────────────────────────────────
      if (this.cfg.blockPasskeys) {
        wc.on('dom-ready', () => {
          void wc.executeJavaScript(DISABLE_PASSKEY_SCRIPT).catch(() => {});
        });
      }

      wc.on('did-navigate', (_, url) => handleUrl(url));
      wc.on('did-redirect-navigation', (_, url) => handleUrl(url));
      wc.on('did-navigate-in-page', (_, url) => handleUrl(url));

      win.on('closed', () => {
        this.loginWindow = undefined;
        if (!resolved) void finish(false);
      });

      win.loadURL(this.cfg.loginUrl).catch((err) => {
        this.logger.error({ err }, 'failed to load login URL');
        void finish(false);
      });
    });
  }

  /**
   * Clear all session data for this board (cookies, storage, cache).
   * The user will need to log in again after calling this.
   */
  async disconnect(): Promise<void> {
    this.logger.info({ board: this.cfg.id }, 'disconnecting — clearing session');
    if (this.loginWindow && !this.loginWindow.isDestroyed()) {
      this.loginWindow.close();
      this.loginWindow = undefined;
    }
    const sess = this.getSession();
    await sess.clearStorageData().catch(() => {});
    await sess.clearCache().catch(() => {});
    await sess.clearAuthCache().catch(() => {});
    // Remove state.json so scrapers also see the session as disconnected.
    await fs.rm(this.stateDir, { recursive: true, force: true }).catch(() => {});
    this.logger.info({ board: this.cfg.id }, 'session cleared');
  }

  /**
   * Write <stateDir>/state.json with the current session cookies.
   * Called automatically after every successful connect.
   *
   * Scrapers in packages/data read this file to get cookies for HTTP
   * requests. This is a compatibility layer — long-term scrapers should
   * use getSession() + net.request({ session }) instead.
   */
  async exportCookies(): Promise<void> {
    const cookies = await this.getSession().cookies.get({});
    await fs.mkdir(this.stateDir, { recursive: true });
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
      path.join(this.stateDir, 'state.json'),
      JSON.stringify(state, null, 2),
      'utf-8'
    );
    this.logger.debug({ count: cookies.length }, 'state.json exported');
  }

  /** Close any open login window without resolving the session state. */
  async close(): Promise<void> {
    if (this.loginWindow && !this.loginWindow.isDestroyed()) {
      this.loginWindow.close();
      this.loginWindow = undefined;
    }
  }
}
