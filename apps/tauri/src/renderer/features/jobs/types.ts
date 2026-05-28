import type { JobInteraction } from '@ajh/shared';

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
}

export interface JobEvent {
  type: string;
  jobId: string;
  data?: unknown;
  ts: number;
}
