import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function jobPreferences(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    get: () => cmd('jobPreferences', 'get'),
    set: (prefs: unknown) => cmd('jobPreferences', 'set', prefs),
  };
}
