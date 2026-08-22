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
  /**
   * Full scraper catalog (id, label, mode, auth tier, listed) from the
   * registry — `SCRAPERS` in
   * `apps/desktop/src-tauri/src/scraping/boards/mod.rs`, whose per-scraper
   * `Scraper` impl owns the auth tier and the listed flag.
   *
   * The canonical id list is `BOARD_IDS` in
   * `packages/shared/src/schemas/index.ts`; it is not restated here, because
   * every copy of it has gone stale. `indeed`, `stepstone`, `xing`, `workday`
   * and `glassdoor` are no longer direct scrapers (ADR-026) — their postings
   * now arrive through the `aggregator` board.
   */
  catalog(): Promise<BoardCatalogEntry[]>;

  /** Live per-board reliability across runs (Track B1). Boards with no recorded
   *  history are simply absent. */
  health(): Promise<BoardHealthEntry[]>;

  /** Connect to a board by launching a browser for manual login. */
  connect(req: { boardId: string }): Promise<{ connected: boolean; accountEmail?: string }>;

  /** Disconnect a board (closes context only; does not delete profile). */
  disconnect(req: { boardId: string }): Promise<void>;

  /** Get current connection status for a board. */
  getStatus(req: {
    boardId: string;
  }): Promise<{ connected: boolean; accountEmail?: string; lastConnected?: number }>;

  /**
   * Try to import session cookies from the user's installed Chromium browsers
   * (Chrome, Edge, Brave), so the user can skip the in-app re-login.
   *
   * Writes the SAME artifacts the in-app browser login produces, so nothing
   * downstream changes. Best-effort by design and never a regression: a missing
   * browser, a locked profile or a store this cannot decrypt all resolve as
   * non-error outcomes (see `CookieImportOutcome`), not as failures. Which
   * cookie encryption versions are covered, and why, is documented at the
   * implementation: `apps/desktop/src-tauri/src/scraping/board_login/import.rs`.
   */
  importCookies(req: { boardId: string }): Promise<CookieImportResult>;
}

/**
 * Per-board outcome from a completed scrape job.
 * - `skipped: "needs-login"` — board bypassed because no session exists.
 * - `skipped: "needs-company"` — ATS board bypassed because no company slug was supplied.
 * - `skipped: "needs-keys"` — key-backed board bypassed because its API keys
 *   aren't configured; prompt the user to add them in Settings. No board emits
 *   this today: the aggregator was the only one, and its keyless freehire tier
 *   can answer without any key, so it never asks to be skipped.
 * - `truncated` — a paginated board kept a partial harvest after a mid-run page
 *   failure (e.g. `"page 3 of 5 failed: HTTP 429"`); `count` is a partial tally,
 *   not the full result set. Absent when the harvest ran to completion.
 * - `notes` — ZERO OR MORE INFORMATIONAL policies the board/engine applied that
 *   the user did not explicitly request (not a failure; `count` is still
 *   authoritative). Widened from a single optional `note` because up to three
 *   independent signals can legitimately coexist in one run: a board-native
 *   note, the central location filter, and the central work-type filter — a
 *   single slot silently dropped whichever one lost. Order fixes precedence
 *   when more than one applies: the board's own note first, then
 *   `location-filtered`, then `work-type-filtered`. Possible entries:
 *   - `"guessed-market:<cc>"` — no country was supplied, so the `<cc>` market was
 *     guessed and returned an authoritative result set; set a country for
 *     deterministic results.
 *   - `"broadened:<cc>"` — a sparse city search was widened country-wide within
 *     the `<cc>` market.
 *   - `"location-filtered:<n>"` — this board doesn't honor location server-side
 *     (`supportsLocation: false`), so the engine conservatively dropped `<n>`
 *     of its results whose own location clearly mismatched the request; never
 *     drops remote/unknown-location rows.
 *   - `"work-type-filtered:<n>"` — the sibling filter for the requested work
 *     type(s) (remote/hybrid/on-site): this board doesn't honor it server-side
 *     (`supportsWorkType: false`), so the engine conservatively dropped `<n>`
 *     of its results whose own DECLARED work type clearly mismatched; a row
 *     the board never declared a work type for is never dropped.
 *   - `"slugs-invalid:<n>"` — a company-slug ATS board rejected `<n>` of the
 *     supplied slugs pre-fetch (malformed company names) but still returned
 *     results from the valid ones. If EVERY slug was rejected it's an `error`,
 *     not a note.
 *   - `"rows-dropped:<n>"` — a company-slug ATS board dropped `<n>` individual
 *     response rows that failed per-row parsing (schema drift on those rows)
 *     while the rest parsed. If EVERY row of a company dropped it's counted as
 *     a fetch failure, not a note. At most one of `slugs-invalid`/`rows-dropped`
 *     is emitted per board per run (`slugs-invalid` wins when both apply).
 *   - `"companies-failed:<n>"` — a company-slug ATS board (Lever, Ashby) could
 *     not fetch `<n>` of the requested companies (404 on a rotted slug, 403,
 *     429, or a payload over the byte cap) while at least one other company
 *     succeeded, so the run still returned results. If EVERY fetch failed it's
 *     an `error`, not a note. Emitted independently of the
 *     `slugs-invalid`/`rows-dropped` pair above — those come from boards that
 *     validate slugs pre-fetch, this one from the boards that don't — but a
 *     board still reports at most ONE board-native note per run (it can still
 *     coexist with `location-filtered`/`work-type-filtered`, per the ordering
 *     above).
 *   `<cc>` is an ISO country code; an entry never carries the raw location text.
 */
/**
 * Verdict of a board's cross-run history (Track B1). Mirrors the Rust
 * `scraping::board_health::BoardHealthStatus`.
 *   - `unknown` — never actually contacted (only ever skipped).
 *   - `healthy` — the last run that contacted it succeeded, recently.
 *   - `failing` — it is in a failure streak.
 *   - `stale`   — not failing, but its last confirmed success is over a
 *     fortnight old (in practice: skipped ever since).
 *   - `flaky`   — working right now, but failing a meaningful share of its
 *     recent verified runs (`failedRuns` / `verifiedRuns`) — the alternating
 *     ok/fail pattern a consecutive-failure streak can't see.
 */
export type BoardHealthStatus = 'unknown' | 'healthy' | 'failing' | 'stale' | 'flaky';

/**
 * One board's reliability across runs, derived in Rust from every previous
 * run's `BoardScrapeSummary` and attached to the current one — what lets the UI
 * tell "this board found nothing today" apart from "this board has been broken
 * since Tuesday".
 *
 * Only an UNHEALTHY board carries this (see `BoardHealth::is_noteworthy`): a
 * healthy board's chip is unchanged. Timestamps are epoch-ms.
 */
export interface BoardHealth {
  status: BoardHealthStatus;
  /** Length of the current failure streak. Skipped runs neither extend nor
   *  break it — a skip is not a failure. */
  consecutiveFailures: number;
  /** Last run that returned results (or an empty-but-successful answer).
   *  Absent = the board has never succeeded. */
  lastSuccessAt?: number;
  /** Last run that contacted the board at all (success OR error). Absent =
   *  only ever skipped, so nothing has been verified. */
  lastVerifiedAt?: number;
  /** Start of the CURRENT failure streak — the "broken since" timestamp. */
  failingSince?: number;
  /** Why the streak is failing, capped in Rust. Still sanitized at display
   *  time like every other persisted reason. */
  lastError?: string;
  /** The scrape `jobId` of the run that produced this state — the per-board
   *  correlation id for the logs. Only ever set by a run that actually
   *  contacted the board, so a skipped run never claims authorship. */
  lastRunId?: string;
  /** Runs that actually contacted the board (ok + error) within a decayed
   *  rolling window (Rust bounds it, not the board's entire history); skips
   *  are excluded because they verify nothing. */
  verifiedRuns: number;
  /** Failures among those windowed runs. `failedRuns / verifiedRuns` is the
   *  flapping signal a consecutive-failure counter structurally cannot express. */
  failedRuns: number;
}

/** One board's live verdict, as returned by `boards_health`. */
export interface BoardHealthEntry {
  board: string;
  health: BoardHealth;
}

export interface BoardScrapeSummary {
  board: string;
  count: number;
  error?: string;
  skipped?: 'needs-login' | 'needs-company' | 'needs-keys';
  truncated?: string;
  notes?: string[];
  /** Cross-run reliability, present only when the board is unhealthy AND this
   *  is a LIVE scrape response. Never persisted (it is cross-run state, not part
   *  of this run) — a stored run record carries none, and the Autopilot card
   *  reads the current verdict from `boards_health` instead. */
  health?: BoardHealth;
}

export const BOARDS_CHANNELS = {
  catalog: 'boards:catalog',
  health: 'boards:health',
  connect: 'boards:connect',
  disconnect: 'boards:disconnect',
  getStatus: 'boards:getStatus',
  importCookies: 'boards:importCookies',
} as const;
