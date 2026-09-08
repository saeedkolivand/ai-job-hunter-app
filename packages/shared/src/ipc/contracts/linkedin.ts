import type { CookieImportResult } from './boards';

export interface LinkedinContract {
  // `connect`/`disconnect`/`getStatus`/`importCookies` dispatch the SAME Tauri command as their
  // `BoardsContract` namesake (`boardId: 'linkedin'` baked in) — the first sentence below is
  // deliberately IDENTICAL, word for word, to `boards.ts`'s own, not independently written: the
  // agent-CLI catalogue generator publishes ONE description per command, and a command reached
  // from two namespaces with two genuinely different TSDocs fails codegen rather than letting
  // directory read order pick a winner (SECURITY/MEDIUM, CLI review round 1).

  /** Connect to a board by launching a browser for manual login. */
  connect(): Promise<{ connected: boolean; accountEmail?: string }>;

  /** Disconnect a board (closes context only; does not delete profile). */
  disconnect(): Promise<void>;

  /** Get current connection status for a board. */
  getStatus(): Promise<{ connected: boolean; accountEmail?: string; lastConnected?: number }>;

  /** Fetch a LinkedIn profile URL and return extracted resume text. */
  importProfileFromUrl(
    url: string
  ): Promise<{ text: string; name?: string; platform: string } | { error: string }>;

  /**
   * Try to import session cookies from the user's installed Chromium browsers
   * (Chrome, Edge, Brave), so the user can skip the in-app re-login.
   */
  importCookies(): Promise<CookieImportResult>;
}

export const LINKEDIN_CHANNELS = {
  connect: 'linkedin:connect',
  disconnect: 'linkedin:disconnect',
  getStatus: 'linkedin:getStatus',
  importProfileFromUrl: 'linkedin:importProfileFromUrl',
  importCookies: 'linkedin:importCookies',
} as const;
