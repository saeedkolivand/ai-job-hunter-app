import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function jobs(opts: WebHttpClientOptions) {
  const { cmd, subscribe } = createHttpClientHelpers(opts);
  return {
    list: () => cmd('jobs', 'list'),
    get: (jobId: string) => cmd('jobs', 'get', { jobId }),
    cancel: (jobId: string) => cmd('jobs', 'cancel', { jobId }),
    retry: (jobId: string) => cmd('jobs', 'retry', { jobId }),
    onEvent: (handler: (event: unknown) => void) => subscribe('jobs.event', handler),
  };
}
