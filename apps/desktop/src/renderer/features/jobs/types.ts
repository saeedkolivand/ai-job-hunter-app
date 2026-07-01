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
}

export interface JobEvent {
  type: string;
  jobId: string;
  data?: unknown;
  ts: number;
}
