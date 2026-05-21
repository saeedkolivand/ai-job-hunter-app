/**
 * AppClient — the single transport abstraction between the renderer and the
 * app backend.
 *
 * Backed by the Tauri invoke adapter. A future web adapter would implement
 * the same interface over HTTP/WebSocket without touching service hooks.
 *
 * Usage in service hooks:    const api = useAppClient();
 * Usage outside components:  const api = getClient();
 */
import type { IpcContract } from '@ajh/shared/ipc';

export type AppClient = IpcContract;

// Module-level reference for use outside React (e.g. standalone query
// functions like fetchJob). Set once when AppClientProvider mounts.
let _client: AppClient | null = null;

export function _registerClient(c: AppClient): void {
  _client = c;
}

/** For use outside React components. Throws if called before provider mounts. */
export function getClient(): AppClient {
  if (!_client) throw new Error('AppClient not initialized — wrap your app in <AppClientProvider>');
  return _client;
}
