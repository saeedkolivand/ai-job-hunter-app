import { invoke } from '@tauri-apps/api/core';

import type { JobPosting } from '@ajh/shared';
import type { ScrapeBoardRequest, ScrapeUrlRequest } from '@ajh/shared/schemas';

export const scrape = {
  board: (req: ScrapeBoardRequest) => invoke<{ jobId: string }>('scrape_board', { req }),
  url: (req: ScrapeUrlRequest) => invoke<{ jobId: string }>('scrape_url', { req }),
  resolveUrl: ({ url }: { url: string }) =>
    invoke<JobPosting | null>('scrape_resolve_url', { url }),
  persistJob: (req: unknown) => invoke<void>('scrape_persist_job', { req }),
  listPostings: () => invoke<JobPosting[]>('scrape_list_postings'),
  clearPostings: () => invoke<void>('scrape_clear_postings'),
  listInteractions: (filter?: unknown) =>
    invoke<
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
    >('scrape_list_interactions', { filter }),
};
