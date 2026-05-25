import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function system(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    health: () => cmd('system', 'health'),
    getVersion: () => cmd('system', 'getVersion'),
    getLocale: () => cmd('system', 'getLocale'),
    setLocale: (locale: string) => cmd('system', 'setLocale', { locale }),
    getPlatform: () => cmd('system', 'getPlatform'),
    openExternal: (url: string) => cmd('system', 'openExternal', { url }),
    setPerformanceMode: (mode: 'low-memory' | 'balanced' | 'performance') =>
      cmd('system', 'setPerformanceMode', { mode }),
    getMetrics: () => cmd('system', 'getMetrics'),
    checkBrowser: () => cmd('system', 'checkBrowser'),
  };
}
