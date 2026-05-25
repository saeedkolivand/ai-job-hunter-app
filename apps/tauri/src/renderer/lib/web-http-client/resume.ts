import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function resume(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    extractText: (req: unknown) => cmd('resume', 'extractText', req),
  };
}
