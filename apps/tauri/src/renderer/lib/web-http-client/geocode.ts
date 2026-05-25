import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function geocode(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    suggest: (query: string) =>
      cmd('geocode', 'suggest', { query }) as Promise<Array<{ display: string }>>,
  };
}
