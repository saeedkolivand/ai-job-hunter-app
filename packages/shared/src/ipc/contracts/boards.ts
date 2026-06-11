export type CookieImportOutcome =
  | 'Imported'
  | 'NoSession'
  | 'Undecryptable'
  | 'BrowserNotFound'
  | 'Error';

export interface CookieImportResult {
  outcome: CookieImportOutcome;
  imported: number;
}

export interface BoardsContract {
  /** Connect to a board by launching a browser for manual login. */
  connect(req: { boardId: string }): Promise<{ connected: boolean; accountEmail?: string }>;

  /** Disconnect a board (closes context only; does not delete profile). */
  disconnect(req: { boardId: string }): Promise<void>;

  /** Get current connection status for a board. */
  getStatus(req: {
    boardId: string;
  }): Promise<{ connected: boolean; accountEmail?: string; lastConnected?: number }>;

  /** Try to import session cookies from the user's installed Chromium browsers. */
  importCookies(req: { boardId: string }): Promise<CookieImportResult>;
}

export const BOARDS_CHANNELS = {
  connect: 'boards:connect',
  disconnect: 'boards:disconnect',
  getStatus: 'boards:getStatus',
  importCookies: 'boards:importCookies',
} as const;
