import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function boards(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    connect: (req: { boardId: string }) => cmd('boards', 'connect', req),
    disconnect: (req: { boardId: string }) => cmd('boards', 'disconnect', req),
    getStatus: (req: { boardId: string }) => cmd('boards', 'getStatus', req),
  };
}
