export type CookieImportOutcome =
  'Imported' | 'NoSession' | 'Undecryptable' | 'BrowserNotFound' | 'Error';

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
  /**
   * Whether the board narrows results by the requested location server-side.
   * When `false`, the engine conservatively post-filters this board's results
   * against the requested location (dropping only clear city mismatches; never
   * remote/unknown-location rows), so the picker can indicate which boards will
   * genuinely honor a location. Optional so older/absent payloads read as `false`.
   */
  supportsLocation?: boolean;
  /**
   * Curated companies this company-scoped ATS board will query when the user
   * supplies none; empty/absent for boards without a seed.
   */
  seededCompanies?: string[];
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
 * - `skipped: "needs-keys"` — key-backed board (the aggregator) bypassed because its
 *   API keys aren't configured; prompt the user to add them in Settings.
 * - `truncated` — a paginated board kept a partial harvest after a mid-run page
 *   failure (e.g. `"page 3 of 5 failed: HTTP 429"`); `count` is a partial tally,
 *   not the full result set. Absent when the harvest ran to completion.
 * - `note` — an INFORMATIONAL location policy the board applied that the user did
 *   not explicitly request (not a failure; `count` is still authoritative). One of:
 *   - `"guessed-market:<cc>"` — no country was supplied, so the `<cc>` market was
 *     guessed and returned an authoritative result set; set a country for
 *     deterministic results.
 *   - `"broadened:<cc>"` — a sparse city search was widened country-wide within
 *     the `<cc>` market.
 *   - `"location-filtered:<n>"` — this board doesn't honor location server-side
 *     (`supportsLocation: false`), so the engine conservatively dropped `<n>`
 *     of its results whose own location clearly mismatched the request; never
 *     drops remote/unknown-location rows.
 *   - `"slugs-invalid:<n>"` — a company-slug ATS board rejected `<n>` of the
 *     supplied slugs pre-fetch (malformed company names) but still returned
 *     results from the valid ones. If EVERY slug was rejected it's an `error`,
 *     not a note.
 *   - `"rows-dropped:<n>"` — a company-slug ATS board dropped `<n>` individual
 *     response rows that failed per-row parsing (schema drift on those rows)
 *     while the rest parsed. If EVERY row of a company dropped it's counted as
 *     a fetch failure, not a note. At most one of `slugs-invalid`/`rows-dropped`
 *     is emitted per board per run (`slugs-invalid` wins when both apply).
 *   `<cc>` is an ISO country code; the field never carries the raw location text.
 */
export interface BoardScrapeSummary {
  board: string;
  count: number;
  error?: string;
  skipped?: 'needs-login' | 'needs-company' | 'needs-keys';
  truncated?: string;
  note?: string;
}

export const BOARDS_CHANNELS = {
  catalog: 'boards:catalog',
  connect: 'boards:connect',
  disconnect: 'boards:disconnect',
  getStatus: 'boards:getStatus',
  importCookies: 'boards:importCookies',
} as const;
