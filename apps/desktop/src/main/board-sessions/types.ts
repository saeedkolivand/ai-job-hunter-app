/**
 * Shared types for the persistent board session architecture.
 */

export interface BoardSessionStatus {
  connected: boolean;
  /** Display name of the account if we can read it from cookies. */
  accountEmail?: string;
  /** Unix ms of last successful auth. */
  lastConnected?: number;
}

/**
 * Configuration for each job board's auth flow.
 *
 * Detection strategy (pick one per board):
 *  - isAuthenticatedUrl:     URL-based  — window navigates to main site after login
 *  - isAuthenticatedCookies: Cookie-based — a specific cookie appears after login
 *
 * Boards that use AJAX login (no navigation) must use isAuthenticatedCookies.
 * The baseline param contains cookies captured after the first page-load so
 * tracking cookies set during initial redirect are excluded from the check.
 */
export interface BoardConfig {
  id: string;
  displayName: string;
  loginUrl: string;

  /** Partition name used for persistent Chromium session storage. */
  partition: string;

  /**
   * If true, inject a script on dom-ready to block WebAuthn/passkey prompts.
   * Only needed for boards that actively surface passkey dialogs (LinkedIn).
   */
  blockPasskeys?: boolean;

  /** URL indicates the user has left the auth flow and reached the main site. */
  isAuthenticatedUrl?: (url: string) => boolean;

  /**
   * A specific cookie (or set of cookies) that only appears after successful
   * authentication. Called after every cookie-change event AND on a 1 s poll.
   * Return false while baseline is undefined (still waiting for page-load).
   */
  isAuthenticatedCookies?: (
    cookies: Electron.Cookie[],
    baseline: Electron.Cookie[] | undefined
  ) => boolean;
}
