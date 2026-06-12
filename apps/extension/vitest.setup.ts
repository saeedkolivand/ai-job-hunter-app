/**
 * Vitest setup for @ajh/extension.
 *
 * Stubs globals the extension code needs so tests can run under jsdom
 * without a real browser or service-worker environment.
 *
 * IMPORTANT: webextension-polyfill throws "This script should only be loaded
 * in a browser extension" when imported outside of a browser extension context.
 * Any test file that imports a module depending on webextension-polyfill must
 * add `vi.mock('webextension-polyfill', ...)` at the top of the test file.
 * The chrome global stub here is a fallback for modules that reference chrome
 * directly (not via the polyfill).
 */

import { vi } from 'vitest';

// ── chrome global stub ───────────────────────────────────────────────────────
// Minimal chrome surface for code that references chrome directly.
// Tests that import storage.ts must mock 'webextension-polyfill' instead.

export const chromeStorageStore: Record<string, unknown> = {};

export const chromeStorageLocal = {
  get: vi.fn(async (key: string) => {
    return { [key]: chromeStorageStore[key] };
  }),
  set: vi.fn(async (items: Record<string, unknown>) => {
    Object.assign(chromeStorageStore, items);
  }),
  remove: vi.fn(async (key: string) => {
    delete chromeStorageStore[key];
  }),
};

const chromeMock = {
  storage: {
    local: chromeStorageLocal,
    sync: chromeStorageLocal,
    onChanged: { addListener: vi.fn() },
  },
  runtime: {
    id: 'test-extension-id',
    sendMessage: vi.fn(),
    onMessage: { addListener: vi.fn() },
  },
};

// Provide the chrome global so webextension-polyfill doesn't throw on
// "no chrome" check (used by some code paths before polyfill import).
if (typeof globalThis.chrome === 'undefined') {
  // Assign directly — vi.stubGlobal would work too but this is more explicit.
  Object.defineProperty(globalThis, 'chrome', {
    value: chromeMock,
    writable: true,
    configurable: true,
  });
}

// ── WebSocket stub ───────────────────────────────────────────────────────────
// jsdom does not implement WebSocket. Bridge tests override this with their
// own fine-grained fake in beforeEach; this baseline prevents "WebSocket is
// not defined" errors from leaking across test files.
if (typeof globalThis.WebSocket === 'undefined') {
  // Will be overridden per-test in bridge.test.ts.
  class StubWebSocket {
    static CONNECTING = 0;
    static OPEN = 1;
    static CLOSING = 2;
    static CLOSED = 3;
    readyState = StubWebSocket.CONNECTING;
    close = vi.fn();
    send = vi.fn();
    addEventListener = vi.fn();
    removeEventListener = vi.fn();
  }
  Object.defineProperty(globalThis, 'WebSocket', {
    value: StubWebSocket,
    writable: true,
    configurable: true,
  });
}
