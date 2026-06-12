import type { JobPosting } from '../types/index.js';

export const SCRAPE_EVENTS = {
  progress: 'scrape:progress',
  item: 'scrape:item',
} as const;

export interface ScrapeProgressEvent {
  jobId: string;
  progress: number;
}

export interface ScrapeItemEvent {
  jobId: string;
  item: JobPosting;
}
