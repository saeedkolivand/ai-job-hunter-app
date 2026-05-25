import type { WebHttpClientOptions } from './utils.js';

export function dialog(_opts: WebHttpClientOptions) {
  // Native file picker is a desktop capability. On web, use the browser's
  // <input type="file"> directly in the component — no IPC needed.
  return {
    openFiles: async () => [],
  };
}
