import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function apply(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    start: (req: unknown) => cmd('apply', 'start', req),
    catalog: () => cmd('apply', 'catalog'),
  };
}
