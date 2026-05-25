import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function credentials(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    available: () => cmd('credentials', 'available'),
    list: () => cmd('credentials', 'list'),
    set: (req: unknown) => cmd('credentials', 'set', req),
    remove: (req: { boardId: string }) => cmd('credentials', 'remove', req),
  };
}
