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

/** Login requirement for a board, sourced from the Rust scraper registry. */
export type BoardAuthRequirement = 'guest' | 'optional' | 'required';

/** One board in the scraper catalog — the source of truth for the jobs picker. */
export interface BoardCatalogEntry {
  id: string;
  displayName: string;
  mode: string;
  auth: BoardAuthRequirement;
  /** Whether the board appears in the manual jobs picker. */
  listed: boolean;
  /**
   * Whether this board requires a company slug to return any results.
   * ATS platforms (Greenhouse, Lever, Ashby, Recruitee, Personio,
   * SmartRecruiters) set this to true. When true, the UI should show a company
   * input field and the engine will skip the board with `skipped: "needs-company"`
   * if no companies are supplied.
   */
  requiresCompany: boolean;
}

export interface BoardsContract {
  /** Full scraper catalog (id, label, mode, auth tier, listed) from the registry. */
  catalog(): Promise<BoardCatalogEntry[]>;

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

/**
 * Per-board outcome from a completed scrape job.
 * - `skipped: "needs-login"` — board bypassed because no session exists.
 * - `skipped: "needs-company"` — ATS board bypassed because no company slug was supplied.
 */
export interface BoardScrapeSummary {
  board: string;
  count: number;
  error?: string;
  skipped?: 'needs-login' | 'needs-company';
}

export const BOARDS_CHANNELS = {
  catalog: 'boards:catalog',
  connect: 'boards:connect',
  disconnect: 'boards:disconnect',
  getStatus: 'boards:getStatus',
  importCookies: 'boards:importCookies',
} as const;
