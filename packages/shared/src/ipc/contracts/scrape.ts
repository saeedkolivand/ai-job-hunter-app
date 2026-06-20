import type { ScrapeBoardsRequest, ScrapeUrlRequest } from '../../schemas/index.js';
import type { JobPosting } from '../../types/index.js';

export interface ScrapeContract {
  boards(req: ScrapeBoardsRequest): Promise<{ jobId: string }>;

  url(req: ScrapeUrlRequest): Promise<{ jobId: string }>;

  /** Resolve a single posting (incl. full description) from its URL. */
  resolveUrl(req: { url: string }): Promise<JobPosting | null>;

  listPostings(): Promise<JobPosting[]>;

  clearPostings(): Promise<void>;

  listInteractions(filter?: { interactionType?: string }): Promise<
    Array<{
      jobId: string;
      interactionType: string;
      timestamp: number;
      title: string;
      company: string;
      url: string;
      source: string;
      location: string;
    }>
  >;

  persistJob(req: { job: Record<string, unknown>; interactionType: string }): Promise<void>;
}

export const SCRAPE_CHANNELS = {
  boards: 'scrape:boards',
  url: 'scrape:url',
  resolveUrl: 'scrape:resolveUrl',
  listPostings: 'scrape:listPostings',
  persistJob: 'scrape:persistJob',
  clearPostings: 'scrape:clearPostings',
  listInteractions: 'scrape:listInteractions',
} as const;
