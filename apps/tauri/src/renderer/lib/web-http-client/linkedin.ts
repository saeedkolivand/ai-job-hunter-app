import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function linkedin(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    connect: () => cmd('linkedin', 'connect'),
    disconnect: () => cmd('linkedin', 'disconnect'),
    getStatus: () => cmd('linkedin', 'getStatus'),
    importProfileFromUrl: (url: string) => cmd('linkedin', 'importProfileFromUrl', { url }),
  };
}
