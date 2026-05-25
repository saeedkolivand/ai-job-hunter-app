import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function scrape(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    board: (req: unknown) => cmd('scrape', 'board', req),
    url: (req: unknown) => cmd('scrape', 'url', req),
    persistJob: (req: unknown) => cmd('scrape', 'persistJob', req),
    listPostings: () => cmd('scrape', 'listPostings'),
    clearPostings: () => cmd('scrape', 'clearPostings'),
    listInteractions: (filter?: unknown) => cmd('scrape', 'listInteractions', filter),
    exportData: () => cmd('scrape', 'exportData'),
    importData: () => cmd('scrape', 'importData'),
  };
}
