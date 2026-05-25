import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function updater(opts: WebHttpClientOptions) {
  const { cmd, subscribe } = createHttpClientHelpers(opts);
  return {
    check: () => cmd('updater', 'check'),
    download: () => cmd('updater', 'download'),
    install: () => cmd('updater', 'install'),
    onStatus: (handler: (status: unknown) => void) => subscribe('updater.status', handler),
  };
}
