import type { JobInteraction, JobTrustAssessment } from '@ajh/shared';

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
