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
}

export interface JobEvent {
  type: string;
  jobId: string;
  data?: unknown;
  ts: number;
}
