import type { DiscoverySearchRequest, DiscoveryStarRequest } from '../../schemas/index.js';

/**
 * A passively-harvested (or curated-seed) ATS company (ADR-030). `extract_ats_ref`
 * pulls `(atsKind, slug)` out of every scraped/imported posting URL; starred rows
 * are the user's "watched companies" that a `watchedCompaniesOnly` autopilot
 * resolves at run time.
 */
export interface DiscoveredCompany {
  /** Registry board id (`greenhouse`, `lever`, `ashby`, …). */
  atsKind: string;
  /** Company slug — casing is preserved (Ashby tokens are case-sensitive). */
  slug: string;
  /** Display name backfilled from the posting's company, when known. */
  displayName?: string;
  /** How many postings this slug has been seen in. */
  seenCount: number;
  /** Whether the user has starred it (a "watched company"). */
  starred: boolean;
  /** Provenance: `scrape | extension | seed` (free-text for future feeders). */
  source: string;
}

/**
 * Discovery namespace (ADR-030 §f): reads over the discovered-companies store
 * that backs the ScrapeForm slug typeahead and the watched-company autopilot
 * target.
 */
export interface DiscoveryContract {
  /**
   * Typeahead search over slug + display name (case-insensitive), starred first
   * then by most-seen. Debouncing is the UI's job.
   */
  searchCompanies(req: DiscoverySearchRequest): Promise<DiscoveredCompany[]>;

  /**
   * Star / unstar a company (materializing a curated-seed row if it was never
   * organically seen). RESOLVES an `{ error }` union on failure — Tauri turns the
   * backend's `json!({"error": ...})` into a resolved value — so the hook must
   * narrow it and throw (mirrors `DedupContract.markNotDuplicate`; #756 lesson).
   */
  setStarred(req: DiscoveryStarRequest): Promise<{ success: true } | { error: string }>;

  /** Every watched (starred) company. */
  watched(): Promise<DiscoveredCompany[]>;
}

export const DISCOVERY_CHANNELS = {
  searchCompanies: 'discovery:searchCompanies',
  setStarred: 'discovery:setStarred',
  watched: 'discovery:watched',
} as const;
