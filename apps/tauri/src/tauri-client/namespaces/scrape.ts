import { invoke } from '@tauri-apps/api/core';

import type { ScrapeBoardRequest, ScrapeUrlRequest } from '@ajh/shared/schemas';

export const scrape = {
  board: (req: ScrapeBoardRequest) => invoke('scrape_board', { req }),
  url: (req: ScrapeUrlRequest) => invoke('scrape_url', { req }),
  persistJob: (req: unknown) => invoke('scrape_persist_job', { req }),
  listPostings: () => invoke('scrape_list_postings'),
  clearPostings: () => invoke('scrape_clear_postings'),
  listInteractions: (filter?: unknown) => invoke('scrape_list_interactions', { filter }),
};
