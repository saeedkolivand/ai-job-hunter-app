/**
 * Vitest setup for @ajh/extension.
 *
 * Stubs globals the extension code needs so tests can run under jsdom
 * without a real browser or service-worker environment.
 *
 * The `browser` namespace comes from @wxt-dev/browser, which reads
 * `globalThis.browser ?? globalThis.chrome` lazily and does NOT throw on
 * import. Tests that exercise browser-API calls mock '@wxt-dev/browser'
 * directly; the chrome global stub here is a fallback for code paths that
 * reference `chrome` directly or resolve the namespace through this global.
 */

import { vi } from 'vitest';

// ── chrome global stub ───────────────────────────────────────────────────────
// Minimal chrome surface for code that references chrome directly, and the
// lazy fallback @wxt-dev/browser resolves to when 'browser' is absent.

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

// Provide the chrome global so @wxt-dev/browser has a namespace to resolve to
// (it reads `globalThis.browser ?? globalThis.chrome` lazily).
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
