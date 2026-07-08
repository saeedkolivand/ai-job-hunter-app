import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { EVENT_CHANNELS, type JobPosting, type ScrapeProgressEvent } from '@ajh/shared';
import type { ScrapeBoardsRequest, ScrapeUrlRequest } from '@ajh/shared/schemas';

import { asyncUnsub } from '../../utils.js';

export const scrape = {
  boards: (req: ScrapeBoardsRequest) => invoke<{ jobId: string }>('scrape_boards', { req }),
  onProgress: (handler: (event: ScrapeProgressEvent) => void) =>
    asyncUnsub(() =>
      listen<ScrapeProgressEvent>(EVENT_CHANNELS.scrape.progress, (e) => handler(e.payload))
    ),
  url: (req: ScrapeUrlRequest) => invoke<{ jobId: string }>('scrape_url', { req }),
  resolveUrl: ({ url }: { url: string }) =>
    invoke<JobPosting | null>('scrape_resolve_url', { url }),
  updateDescription: (req: { id: string; description: string }) =>
    invoke<boolean>('scrape_update_description', { req }),
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
