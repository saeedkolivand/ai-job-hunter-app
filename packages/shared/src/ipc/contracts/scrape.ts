import type { ScrapeBoardRequest, ScrapeUrlRequest } from '../../schemas/index.js';
import type { JobPosting } from '../../types/index.js';

export interface ScrapeContract {
  board(req: ScrapeBoardRequest): Promise<{ jobId: string }>;

  url(req: ScrapeUrlRequest): Promise<{ jobId: string }>;

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

  exportData(): Promise<{ success: boolean; filePath?: string; error?: string }>;

  importData(): Promise<{ success: boolean; imported: number; error?: string }>;
}

export const SCRAPE_CHANNELS = {
  board: 'scrape:board',
  url: 'scrape:url',
  listPostings: 'scrape:listPostings',
  persistJob: 'scrape:persistJob',
  clearPostings: 'scrape:clearPostings',
  listInteractions: 'scrape:listInteractions',
  exportData: 'scrape:exportData',
  importData: 'scrape:importData',
} as const;
