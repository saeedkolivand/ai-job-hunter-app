import { invoke } from '@tauri-apps/api/core';

export const scrape = {
  board: (req: unknown) => invoke('scrape_board', { req }),
  url: (req: unknown) => invoke('scrape_url', { req }),
  persistJob: (req: unknown) => invoke('scrape_persist_job', { req }),
  listPostings: () => invoke('scrape_list_postings'),
  clearPostings: () => invoke('scrape_clear_postings'),
  listInteractions: (filter?: unknown) => invoke('scrape_list_interactions', { filter }),
  exportData: () => invoke('scrape_export_data'),
  importData: () => invoke('scrape_import_data'),
};
