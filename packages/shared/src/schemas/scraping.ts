import { z } from 'zod';

/**
 * Every board id the app knows, in catalog display order.
 *
 * This list MIRRORS the Rust registry (`scraping/boards/mod.rs::SCRAPERS`) —
 * which is the real catalog the UI renders from — so the two can drift, and did:
 * `jobicy` shipped as a registered, listed board with en/de labels and was
 * missing here entirely. `pnpm gen:ipc` now emits this list to
 * `ipc_contracts/board_ids.rs` and a Rust test compares it against `SCRAPERS`
 * in BOTH directions, so a board added on either side fails the build until it
 * is added on the other.
 */
export const BOARD_IDS = [
  // Major
  'linkedin',
  // German / DACH
  'arbeitsagentur',
  'berlinstartupjobs',
  'germantechjobs',
  // ATS platforms
  'greenhouse',
  'lever',
  'ashby',
  'smartrecruiters',
  'recruitee',
  'personio',
  'pinpoint',
  'rippling',
  'breezy',
  'bamboohr',
  'workable',
  'comeet',
  // Remote-first / aggregators
  'aggregator',
  'freehire',
  'remoteok',
  'remotive',
  'arbeitnow',
  'jobicy',
  'themuse',
  'wwr',
  'ycombinator',
] as const;
export type BoardId = (typeof BOARD_IDS)[number];

/** Stable catalog id for the Adzuna-powered aggregator board. */
export const AGGREGATOR_BOARD_ID = 'aggregator' satisfies BoardId;

export const DATE_FILTER_OPTIONS = [
  '15m',
  '30m',
  '1h',
  '2h',
  '4h',
  '8h',
  '24h',
  'week',
  'month',
] as const;
export type DateFilterOption = (typeof DATE_FILTER_OPTIONS)[number];

// The one normalised work-arrangement vocabulary this app uses everywhere — the
// manual search filter, the autopilot target, and (a later phase) each board's
// own declared data. Replaces two previously-incompatible ones: the scrape
// schema's LinkedIn raw codes ('1'/'2'/'3') and the autopilot's own ad-hoc
// enum. Multi-select: an empty/absent set means "no filter" ("any").
export const WORK_TYPE_OPTIONS = ['remote', 'hybrid', 'on-site'] as const;
export type WorkTypeOption = (typeof WORK_TYPE_OPTIONS)[number];

/**
 * Match-score band cut points — the SINGLE source for the renderer's
 * `scoreTier` badge bands AND the Rust `autopilot_best_matches` qualification
 * bar (emitted to `ipc_contracts/match_tiers.rs` by `pnpm gen:ipc`; CI's
 * `gen:ipc:check` guards drift). `coverage` gates the embedding-free
 * keyword-coverage kernel's 0–100 score; `combined` gates the semantic+ATS
 * kernel's — coverage clusters lower than combined, so its cut points are
 * relaxed.
 * // ponytail: heuristic starting values, not calibrated
 */
export const MATCH_TIER_CUTS = {
  coverage: { high: 55, medium: 30 },
  combined: { high: 75, medium: 50 },
} as const;

export const ScrapeBoardsRequestSchema = z.object({
  // Bounded by the catalog size (not a fixed number) so selecting every listed
  // board always validates and adding a board needs no schema edit.
  //
  // The `z.enum` constrains the TypeScript type and this schema's own `parse`.
  // It does NOT reach the wire: `pnpm gen:ipc` emits `boards` as `Vec<String>`,
  // because the codegen lowers enums to strings — so the id an IPC call
  // actually sends is checked by the Rust registry lookup (`boards::get`), not
  // here. The real dedup+truncate defense against a request-amplification
  // payload likewise lives server-side in the Rust engine (registry-size cap
  // over the deduped set).
  boards: z.array(z.enum(BOARD_IDS)).min(1).max(BOARD_IDS.length),
  query: z.string().min(1),
  location: z.string().optional(),
  // Target number of postings to collect per board. The backend paginates each
  // board at its real page size until it has ~amount results (or hits the
  // per-board page budget), then stops.
  amount: z.number().int().min(1).max(100).default(25),
  // When true (a NEW search, not "show more"), the backend replaces the live
  // postings cache the instant the first new result streams in — so a failed or
  // empty search keeps the previous results. Omitted/false = append.
  replace: z.boolean().optional(),
  dateFilter: z.enum(DATE_FILTER_OPTIONS).optional(),
  // Structured location (from a picked geocode suggestion) — lets boards filter
  // by precise place/country/radius instead of fuzzy free text (#49/#40).
  // ISO 3166-1 alpha-2 (the geocode suggestion's countryCode is always 2 letters);
  // validated here so a malformed value can't propagate through IPC/scraping.
  countryCode: z
    .string()
    .trim()
    .regex(/^[A-Za-z]{2}$/)
    .optional(),
  latitude: z.number().optional(),
  longitude: z.number().optional(),
  radiusKm: z.number().int().min(0).max(200).optional(),
  // Structured search filters consumed by LinkedIn's `search_paginated` (and
  // ignored by boards without such filters). Free-text codes so new LinkedIn
  // filter values work without a schema change; validated server-side.
  // `jobType`: 'F' (Full-time), 'P' (Part-time), 'C' (Contract), … ;
  // `sortBy`: 'DD' (Date Descending), 'R' (Relevance).
  jobType: z.string().optional(),
  // Requested work arrangement(s) — the normalised `WORK_TYPE_OPTIONS`
  // vocabulary, not a per-board code (LinkedIn's own `f_WT` encoding now lives
  // inside the LinkedIn board module, not this shared contract). Multi-select;
  // empty/absent = no filter. `z.enum` bounds VALUES only, never length/
  // multiplicity (CWE-770) — `.max` caps a duplicated/hostile payload at the
  // vocabulary size; the Rust side dedupes too (`BoardSearchInput::work_type_spec`).
  workTypes: z.array(z.enum(WORK_TYPE_OPTIONS)).max(WORK_TYPE_OPTIONS.length).optional(),
  experienceLevel: z.string().optional(),
  easyApply: z.boolean().optional(),
  activelyHiring: z.boolean().optional(),
  verified: z.boolean().optional(),
  sortBy: z.string().optional(),
  // Company / board identifiers for ATS boards (greenhouse, lever, ashby,
  // recruitee, personio, smartrecruiters, pinpoint, rippling, breezy,
  // bamboohr) whose public APIs have no global keyword search — they require
  // a company slug (e.g. Greenhouse
  // `boards-api.greenhouse.io/v1/boards/{company}/jobs`). Absent/empty = no
  // company filter; only ATS boards read it, every other board ignores it.
  companies: z.array(z.string().trim().min(1)).optional(),
});

export const ScrapeUrlRequestSchema = z.object({
  url: z.string().url(),
});

/**
 * Request for `scrape:hybridSearch` — rank the live postings cache (or a
 * renderer-supplied eligible subset of it) by lexical + optional dense
 * relevance to `query`. See `commands::hybrid_search` (Rust) for the
 * lexical/dense/fusion/rerank pipeline this drives.
 */
export const PostingsHybridSearchRequestSchema = z.object({
  /**
   * Client-minted id for this search — MUST start with `"search-"` (e.g.
   * `` `search-${crypto.randomUUID()}` ``). Pass the SAME value to
   * `jobs.cancel(queryId)` to abort a superseded search before it finishes
   * embedding/reranking — hybrid search registers against the app-wide
   * `CancelRegistry` every job kind already cancels through, so no separate
   * cancel channel exists for this command. The prefix is the safety net
   * `jobs::cancel::CancelRegistry`'s Rust doc names by name: every other id
   * sharing that registry is Rust-minted (`job-{uuid}`/`run-{uuid}`), and an
   * unprefixed caller-chosen id could otherwise NAME a live run's own id,
   * replacing (then deleting) its cancellation token. Rejected server-side
   * (`commands::hybrid_search::QUERY_ID_PREFIX`) if this is ever bypassed.
   */
  queryId: z.string().min(1).max(64).startsWith('search-'),
  query: z.string().trim().min(1).max(200),
  /**
   * The renderer's own eligible-posting-id allowlist — e.g. `JobsPage`'s
   * cluster-canonical / agency / work-type filter chain already applied —
   * so ranking runs over exactly the set the user can see, not the whole
   * cache before those filters. Absent/empty ranks the whole live cache
   * (used by the agent CLI and by tests). Capped well above any realistic
   * multi-board live cache (`commands::autopilot::rerank`'s own per-board
   * scrape cap is ~100 postings) so a hostile/buggy caller can't force an
   * unbounded rank pass; ids absent from the live cache are ignored rather
   * than trusted, so this can never resurrect a cleared corpus.
   */
  eligibleIds: z.array(z.string().min(1)).max(2000).optional(),
  /** How many ranked ids to return, best first. Defaults to 20 when omitted. */
  limit: z.number().int().min(1).max(50).optional(),
});

// Cross-board dedup "split" request (ADR-029 §h): mark `memberKey` as NOT a
// duplicate of each of `otherKeys` (opaque canonical job keys the renderer
// echoes back from a cluster's members). `autopilotId` scopes an autopilot
// found-jobs split so that record's annotations are recomputed too.
export const DedupMarkNotDuplicateRequestSchema = z.object({
  memberKey: z.string().trim().min(1),
  otherKeys: z.array(z.string().trim().min(1)).min(1).max(32),
  autopilotId: z.string().optional(),
});

// ─── Discovery (passively-harvested ATS company slugs) — ADR-030 §f ───────────

// Typeahead search over discovered/seeded company slugs + display names. The
// query may be empty (returns the top rows); the ~100-char ceiling is a generous
// sanity bound — the Rust command re-clamps server-side (renderer Zod is not a
// trust boundary). `atsKind` optionally scopes to a single ATS (unused today).
export const DiscoverySearchRequestSchema = z.object({
  query: z.string().trim().min(0).max(100),
  atsKind: z.string().optional(),
});

// Star / unstar a discovered (or curated-seed) company — the user's "watched
// companies" set that a `watchedCompaniesOnly` autopilot resolves at run time.
export const DiscoveryStarRequestSchema = z.object({
  atsKind: z.string().trim().min(1),
  slug: z.string().trim().min(1),
  starred: z.boolean(),
});

export type DiscoverySearchRequest = z.infer<typeof DiscoverySearchRequestSchema>;
export type DiscoveryStarRequest = z.infer<typeof DiscoveryStarRequestSchema>;
export type DedupMarkNotDuplicateRequest = z.infer<typeof DedupMarkNotDuplicateRequestSchema>;
export type ScrapeBoardsRequest = z.infer<typeof ScrapeBoardsRequestSchema>;
export type ScrapeUrlRequest = z.infer<typeof ScrapeUrlRequestSchema>;
export type PostingsHybridSearchRequest = z.infer<typeof PostingsHybridSearchRequestSchema>;
