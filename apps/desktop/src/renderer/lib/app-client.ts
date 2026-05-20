/**
 * AppClient — the single transport abstraction between the renderer and the
 * app backend.
 *
 * On desktop it is backed by window.api (Electron IPC via contextBridge).
 * A future web adapter would implement the same interface over HTTP/WebSocket
 * without touching a single service hook or feature component.
 *
 * Usage in service hooks:    const api = useAppClient();
 * Usage outside components:  const api = getClient();
 */
import type { Api } from '../../preload/index.js';

export type AppClient = Api;

/** Create the desktop IPC adapter. Called once by AppClientProvider. */
export function createDesktopIpcClient(): AppClient {
  return window.api;
}

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
