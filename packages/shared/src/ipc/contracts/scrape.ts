import type { ScrapeBoardsRequest, ScrapeUrlRequest } from '../../schemas/index.js';
import type { JobPosting } from '../../types/index.js';

export interface ScrapeContract {
  boards(req: ScrapeBoardsRequest): Promise<{ jobId: string }>;

  url(req: ScrapeUrlRequest): Promise<{ jobId: string }>;

  /** Resolve a single posting (incl. full description) from its URL. */
  resolveUrl(req: { url: string }): Promise<JobPosting | null>;

  /**
   * Write a freshly-resolved full description back into the live postings cache
   * by posting id, so the match scorer reads the full text instead of the
   * truncated aggregator snippet. Returns `true` when an entry was updated,
   * `false` when the id is no longer in the live cache.
   */
  updateDescription(req: { id: string; description: string }): Promise<boolean>;

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
  updateDescription: 'scrape:updateDescription',
  listPostings: 'scrape:listPostings',
  persistJob: 'scrape:persistJob',
  clearPostings: 'scrape:clearPostings',
  listInteractions: 'scrape:listInteractions',
} as const;
