import type { WebHttpClientOptions } from './utils.js';

export function shortcuts(_opts: WebHttpClientOptions) {
  // Keyboard shortcuts on the web are handled entirely in the renderer —
  // no shell IPC needed. The handler is registered here as a no-op so the
  // feature code can still call onCommandPalette() without branching.
  return {
    onCommandPalette: (_handler: () => void) => () => {},
  };
}
