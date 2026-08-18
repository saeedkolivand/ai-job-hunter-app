import {
  AGGREGATOR_BOARD_ID,
  type DATE_FILTER_OPTIONS,
  type JobInteraction,
  type JobTrustAssessment,
} from '@ajh/shared';

export interface Posting {
  id: string;
  source: string;
  externalId: string;
  url: string;
  title: string;
  company: string;
  location?: string;
  remote?: boolean;
  description: string;
  postedAt?: number;
  capturedAt: number;
  interactions?: JobInteraction[];
  /** Ghost-job trust signal, computed at scrape-time. Absent on postings
   *  captured before this field existed. */
  trust?: JobTrustAssessment;
  /** Scraped salary range (Adzuna only, today) — grounds the salary
   *  application answer before it falls back to a web lookup. Absent when
   *  the board didn't report salary. Round-trips through the backend's
   *  flattened `JobPosting.extra`, so it's already present on the raw IPC
   *  payload this type is cast from; declared here so callers can read it. */
  salaryMin?: number;
  salaryMax?: number;
  /** ISO-4217 currency for `salaryMin`/`salaryMax`. */
  salaryCurrency?: string;
  /** Cross-board cluster id (the canonical member's key), recomputed at every
   *  ingest (ADR-029). Absent on rows not yet clustered. Opaque — the renderer
   *  groups by it and echoes member keys back to `dedup.markNotDuplicate`. */
  clusterId?: string;
  /** Whether this row is its cluster's canonical (displayed) member. Absent →
   *  treat as `true` (a standalone/legacy row is its own canonical). */
  clusterCanonical?: boolean;
  /** Every member of this row's cluster, so the renderer can group + split.
   *  Present on the canonical row; members include self. */
  clusterMembers?: Array<{ key: string; board?: string; url: string }>;
  /** Whether the posting's company is a recruiting/staffing agency (ADR-029 §i). */
  isAgency?: boolean;
}

export interface JobEvent {
  type: string;
  jobId: string;
  data?: unknown;
  ts: number;
}

/**
 * Terminal outcome of a scrape run — `ok` plus an optional human note (a
 * partial-failure summary or a sanitized error message). Kept in the session
 * store so it survives a route change while the backend job keeps running.
 */
export interface ScrapeOutcome {
  ok: boolean;
  note?: string;
}

/**
 * The scrape drawer's search criteria.
 *
 * Feature-level (not `components/ScrapeForm/constants`) because the session
 * store owns `jobs.scrapeForm`: the store reaching into a component's internals
 * would be a level worse than the feature-level type imports it already makes.
 * `ScrapeForm/constants` re-exports this for the component subtree.
 */
export interface ScrapeFormState {
  boards: string[];
  query: string;
  location: string;
  /** Structured location captured from a picked geocode suggestion (#49/#40). */
  countryCode?: string;
  latitude?: number;
  longitude?: number;
  /** Search radius in km; 0 = exact location (no radius). */
  radiusKm: number;
  /** Target number of jobs to fetch (#41); sent as the scrape `amount` (backend clamps to 100). */
  amount: number;
  dateFilter: '' | (typeof DATE_FILTER_OPTIONS)[number];
  /**
   * Company slugs for ATS boards (greenhouse, lever, ashby, etc.) whose APIs
   * require a company identifier. Comma-separated in the UI, stored as an array.
   * Empty array = no filter; backend skips ATS boards with `needs-company`.
   */
  companies: string[];
}

/**
 * A FRESH initial scrape form. A factory, not a shared constant: `boards` and
 * `companies` are arrays, and a single shared instance spread into every store
 * reset would alias them across resets.
 */
export function makeScrapeFormDefaults(): ScrapeFormState {
  return {
    boards: [AGGREGATOR_BOARD_ID],
    query: '',
    location: '',
    radiusKm: 0,
    amount: 25,
    dateFilter: '',
    companies: [],
  };
}
