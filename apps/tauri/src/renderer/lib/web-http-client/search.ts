import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function search(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    hybrid: (req: unknown) => cmd('search', 'hybrid', req),
  };
}
