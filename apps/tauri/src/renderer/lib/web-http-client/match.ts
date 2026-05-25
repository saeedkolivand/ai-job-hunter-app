import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function match(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    resume: (req: unknown) => cmd('match', 'resume', req),
  };
}
