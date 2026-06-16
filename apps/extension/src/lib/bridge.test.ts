/**
 * Unit tests for BridgeClient (apps/extension/src/lib/bridge.ts).
 *
 * Bridge.ts uses `new WebSocket(url)` as a raw global reference — we replace
 * `globalThis.WebSocket` BEFORE calling any BridgeClient methods so every
 * `new WebSocket` call goes through our fake.
 *
 * For reply correlation we capture the actual reqId from the outgoing
 * socket.send() call (rather than stubbing crypto.randomUUID) so the test
 * stays correct regardless of how reqIds are generated.
 *
 * vi.waitFor retries while the callback THROWS — always wrap assertions
 * in expect() so the condition polls properly.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import { EXTENSION_MESSAGE_TYPES } from '@ajh/shared';

import { BridgeClient } from './bridge';

// Default `connectNative` THROWS so the existing ws describe-blocks (which never
// mock it) go straight to the ws probe — native is treated as "host unavailable
// → fall back to ws". The native describe-block overrides this per-test.
vi.mock('@wxt-dev/browser', () => ({
  browser: {
    runtime: {
      connectNative: vi.fn(() => {
        throw new Error('connectNative not available');
      }),
      lastError: undefined,
    },
  },
}));

// ── WebSocket fake ────────────────────────────────────────────────────────────

type WSEventType = 'open' | 'close' | 'error' | 'message';

interface FakeWebSocket {
  url: string;
  readyState: number;
  close: ReturnType<typeof vi.fn>;
  send: ReturnType<typeof vi.fn>;
  addEventListener: ReturnType<typeof vi.fn>;
  simulateOpen: () => void;
  simulateClose: () => void;
  simulateMessage: (data: string) => void;
}

const WS_CONNECTING = 0;
const WS_OPEN = 1;
const WS_CLOSED = 3;

function buildFakeWS(url: string): FakeWebSocket {
  const listeners: Partial<Record<WSEventType, Array<(ev?: unknown) => void>>> = {};

  const ws: FakeWebSocket = {
    url,
    readyState: WS_CONNECTING,
    close: vi.fn(() => {
      ws.readyState = WS_CLOSED;
      listeners['close']?.forEach((cb) => cb());
    }),
    send: vi.fn(),
    addEventListener: vi.fn((type: WSEventType, cb: (ev?: unknown) => void) => {
      if (!listeners[type]) listeners[type] = [];
      listeners[type]!.push(cb);
    }),
    simulateOpen() {
      ws.readyState = WS_OPEN;
      listeners['open']?.forEach((cb) => cb());
    },
    simulateClose() {
      ws.readyState = WS_CLOSED;
      listeners['close']?.forEach((cb) => cb());
    },
    simulateMessage(data: string) {
      listeners['message']?.forEach((cb) => cb({ data }));
    },
  };
  return ws;
}

/** Replace globalThis.WebSocket with a fake factory; returns a restore fn. */
function installFakeWS(onNew: (ws: FakeWebSocket, url: string) => void): () => void {
  const FakeConstructor = function (url: string) {
    const ws = buildFakeWS(url);
    onNew(ws, url);
    return ws;
  } as unknown as typeof WebSocket;
  (FakeConstructor as unknown as Record<string, number>).CONNECTING = WS_CONNECTING;
  (FakeConstructor as unknown as Record<string, number>).OPEN = WS_OPEN;
  (FakeConstructor as unknown as Record<string, number>).CLOSING = 2;
  (FakeConstructor as unknown as Record<string, number>).CLOSED = WS_CLOSED;

  const original = globalThis.WebSocket;
  globalThis.WebSocket = FakeConstructor;
  return () => {
    globalThis.WebSocket = original;
  };
}

// ── helpers ───────────────────────────────────────────────────────────────────

const FAKE_TOKEN = 'a'.repeat(64);

function makeImportRequest() {
  return { url: 'https://example.com/job/123', applied: false };
}

/**
 * Build a reply envelope using the reqId extracted from the OUTGOING frame.
 * This avoids hard-coding a reqId that may not match the real crypto.randomUUID
 * output.
 */
function extractReqId(sentFrameArg: unknown): string {
  const envelope = JSON.parse(sentFrameArg as string) as { reqId?: string };
  return envelope.reqId ?? '';
}

function makeResultEnvelope(reqId: string, payload: unknown): string {
  return JSON.stringify({
    type: EXTENSION_MESSAGE_TYPES.importResult,
    token: FAKE_TOKEN,
    reqId,
    payload,
  });
}

// ── port probe ────────────────────────────────────────────────────────────────

describe('BridgeClient – port probe', () => {
  let createdSockets: FakeWebSocket[] = [];
  let restoreWS: () => void;

  beforeEach(() => {
    createdSockets = [];
    restoreWS = installFakeWS((ws) => createdSockets.push(ws));
  });

  afterEach(() => {
    restoreWS();
    vi.useRealTimers();
  });

  it('resolves on the first port that opens (47615) and does not probe further', async () => {
    const client = new BridgeClient(vi.fn());
    const connectPromise = client.ensureConnected();

    await vi.waitFor(() => {
      expect(createdSockets.length).toBeGreaterThanOrEqual(1);
    });

    const firstSocket = createdSockets[0]!;
    expect(firstSocket.url).toBe('ws://127.0.0.1:47615');

    firstSocket.simulateOpen();
    await connectPromise;

    // Only one socket — probe stopped at first success.
    expect(createdSockets).toHaveLength(1);
    expect(client.isOpen()).toBe(true);
    expect(client.status()).toMatchObject({ phase: 'connected', port: 47615 });

    client.dispose();
  });

  it('skips a failing port and connects on the next available one', async () => {
    const client = new BridgeClient(vi.fn());
    const connectPromise = client.ensureConnected();

    await vi.waitFor(() => {
      expect(createdSockets.length).toBeGreaterThanOrEqual(1);
    });
    createdSockets[0]!.simulateClose();

    await vi.waitFor(() => {
      expect(createdSockets.length).toBeGreaterThanOrEqual(2);
    });
    expect(createdSockets[1]!.url).toBe('ws://127.0.0.1:47616');
    createdSockets[1]!.simulateOpen();

    await connectPromise;

    // Two sockets created; probe stopped after the second succeeded.
    expect(createdSockets).toHaveLength(2);
    expect(client.status()).toMatchObject({ phase: 'connected', port: 47616 });

    client.dispose();
  });

  it('enters app_not_running and schedules a reconnect when all ports fail', async () => {
    vi.useFakeTimers();

    const client = new BridgeClient(vi.fn());
    const connectPromise = client.ensureConnected();

    for (let attempt = 0; attempt < 6; attempt += 1) {
      const idx = attempt;
      await vi.waitFor(() => {
        expect(createdSockets.length).toBeGreaterThanOrEqual(idx + 1);
      });
      createdSockets[idx]!.simulateClose();
    }

    await connectPromise;
    expect(client.status().phase).toBe('app_not_running');

    // Advance past backoff[0]=500ms — reconnect probe must create a new socket.
    const countBefore = createdSockets.length;
    vi.advanceTimersByTime(600);

    await vi.waitFor(() => {
      expect(createdSockets.length).toBeGreaterThan(countBefore);
    });

    client.dispose();
  });
});

// ── request/reply correlation ─────────────────────────────────────────────────

describe('BridgeClient – request/reply correlation', () => {
  let latestSocket: FakeWebSocket | undefined;
  let restoreWS: () => void;

  beforeEach(() => {
    latestSocket = undefined;
    restoreWS = installFakeWS((ws) => {
      latestSocket = ws;
    });
  });

  afterEach(() => {
    restoreWS();
  });

  async function connectedClient(): Promise<{ client: BridgeClient; socket: FakeWebSocket }> {
    const client = new BridgeClient(vi.fn());
    const p = client.ensureConnected();
    await vi.waitFor(() => {
      expect(latestSocket).toBeDefined();
    });
    const socket = latestSocket!;
    socket.simulateOpen();
    await p;
    return { client, socket };
  }

  it('resolves the correct pending promise when a matching reqId reply arrives', async () => {
    const { client, socket } = await connectedClient();

    const importPromise = client.importJob(FAKE_TOKEN, makeImportRequest());

    // Wait for the outgoing frame, then extract the real reqId and echo a reply.
    await vi.waitFor(() => {
      expect(socket.send).toHaveBeenCalled();
    });
    const reqId = extractReqId(socket.send.mock.calls[0]?.[0]);

    const replyPayload = { applicationId: 'app-xyz', status: 'saved' };
    socket.simulateMessage(makeResultEnvelope(reqId, replyPayload));

    const result = await importPromise;
    expect(result).toEqual(replyPayload);

    client.dispose();
  });

  it('ignores a reply whose reqId does not match any pending request', async () => {
    vi.useFakeTimers();

    const { client, socket } = await connectedClient();

    const importPromise = client.importJob(FAKE_TOKEN, makeImportRequest());

    await vi.waitFor(() => {
      expect(socket.send).toHaveBeenCalled();
    });

    // Stray reply with a different reqId — must NOT resolve the pending promise.
    socket.simulateMessage(
      makeResultEnvelope('stray-id-999', { applicationId: 'should-not-resolve' })
    );

    // Deterministic check: attach a .then spy, flush all microtasks (no
    // additional time advancement needed — microtask queue drains here), and
    // assert the spy was never called.  If the stray reply incorrectly resolved
    // the promise the spy would have been invoked after the `await` above.
    const thenSpy = vi.fn();
    importPromise.then(thenSpy).catch(() => {
      // ignore — we only care whether then fired
    });

    // Drain microtasks without advancing macrotask timers.
    await Promise.resolve();
    await Promise.resolve();

    expect(thenSpy).not.toHaveBeenCalled();

    vi.useRealTimers();
    client.dispose();
  });

  it('resolves two concurrent requests independently via their reqIds', async () => {
    const { client, socket } = await connectedClient();

    const p1 = client.importJob(FAKE_TOKEN, makeImportRequest());
    const p2 = client.importJob(FAKE_TOKEN, makeImportRequest());

    // Wait for both frames to be sent.
    await vi.waitFor(() => {
      expect(socket.send.mock.calls.length).toBeGreaterThanOrEqual(2);
    });

    const reqId1 = extractReqId(socket.send.mock.calls[0]?.[0]);
    const reqId2 = extractReqId(socket.send.mock.calls[1]?.[0]);
    // Two distinct reqIds must have been generated.
    expect(reqId1).not.toBe(reqId2);

    // Reply to second request first.
    socket.simulateMessage(makeResultEnvelope(reqId2, { applicationId: 'b', status: 'saved' }));
    const r2 = await p2;
    expect(r2).toMatchObject({ applicationId: 'b' });

    socket.simulateMessage(makeResultEnvelope(reqId1, { applicationId: 'a', status: 'saved' }));
    const r1 = await p1;
    expect(r1).toMatchObject({ applicationId: 'a' });

    client.dispose();
  });
});

// ── Zod validation ────────────────────────────────────────────────────────────

describe('BridgeClient – Zod schema validation on incoming payload', () => {
  let latestSocket: FakeWebSocket | undefined;
  let restoreWS: () => void;

  beforeEach(() => {
    latestSocket = undefined;
    restoreWS = installFakeWS((ws) => {
      latestSocket = ws;
    });
  });

  afterEach(() => {
    restoreWS();
  });

  async function connectedClient(): Promise<{ client: BridgeClient; socket: FakeWebSocket }> {
    const client = new BridgeClient(vi.fn());
    const p = client.ensureConnected();
    await vi.waitFor(() => {
      expect(latestSocket).toBeDefined();
    });
    const socket = latestSocket!;
    socket.simulateOpen();
    await p;
    return { client, socket };
  }

  /** Start an importJob and wait until the outgoing frame is sent; return the reqId. */
  async function startImport(
    client: BridgeClient,
    socket: FakeWebSocket
  ): Promise<{ importPromise: Promise<unknown>; reqId: string }> {
    const importPromise = client.importJob(FAKE_TOKEN, makeImportRequest());
    await vi.waitFor(() => {
      expect(socket.send).toHaveBeenCalled();
    });
    const reqId = extractReqId(socket.send.mock.calls[socket.send.mock.calls.length - 1]?.[0]);
    return { importPromise, reqId };
  }

  it('returns a malformed-result error when payload fails ExtensionImportResultSchema', async () => {
    const { client, socket } = await connectedClient();
    const { importPromise, reqId } = await startImport(client, socket);

    // applicationId must be optional string; a number breaks the schema.
    const badPayload = { applicationId: 12345, status: false };
    socket.simulateMessage(makeResultEnvelope(reqId, badPayload));

    // Must resolve (not throw) with a malformed error — never trust bad data.
    const result = (await importPromise) as { error?: string; applicationId?: unknown };
    expect(result.error).toMatch(/malformed/i);
    expect(result.applicationId).toBeUndefined();

    client.dispose();
  });

  it('does NOT return a success payload when the whole payload is a primitive', async () => {
    const { client, socket } = await connectedClient();
    const { importPromise, reqId } = await startImport(client, socket);

    socket.simulateMessage(makeResultEnvelope(reqId, 'this-is-a-string-not-an-object'));

    const result = (await importPromise) as { error?: string };
    expect(result.error).toBeDefined();
    // The error must come from the bridge guard, not be the raw payload string.
    expect(result.error).not.toBe('this-is-a-string-not-an-object');

    client.dispose();
  });

  it('resolves cleanly when a well-formed payload passes schema validation', async () => {
    const { client, socket } = await connectedClient();
    const { importPromise, reqId } = await startImport(client, socket);

    const goodPayload = { applicationId: 'app-good', status: 'saved' };
    socket.simulateMessage(makeResultEnvelope(reqId, goodPayload));

    const result = (await importPromise) as {
      error?: string;
      applicationId?: string;
      status?: string;
    };
    expect(result.error).toBeUndefined();
    expect(result.applicationId).toBe('app-good');
    expect(result.status).toBe('saved');

    client.dispose();
  });
});

// ── reconnect/backoff ─────────────────────────────────────────────────────────

describe('BridgeClient – reconnect/backoff on close', () => {
  let createdCount = 0;
  let latestSocket: FakeWebSocket | undefined;
  let restoreWS: () => void;

  beforeEach(() => {
    createdCount = 0;
    latestSocket = undefined;
    restoreWS = installFakeWS((ws) => {
      createdCount += 1;
      latestSocket = ws;
    });
  });

  afterEach(() => {
    restoreWS();
    vi.useRealTimers();
  });

  async function connectedClientRealTimers(): Promise<{
    client: BridgeClient;
    socket: FakeWebSocket;
  }> {
    const client = new BridgeClient(vi.fn());
    const p = client.ensureConnected();
    await vi.waitFor(() => {
      expect(latestSocket).toBeDefined();
    });
    const socket = latestSocket!;
    socket.simulateOpen();
    await p;
    return { client, socket };
  }

  it('schedules a reconnect timer after the connected socket closes unexpectedly', async () => {
    const { client, socket } = await connectedClientRealTimers();
    expect(client.status().phase).toBe('connected');

    vi.useFakeTimers();
    const countBefore = createdCount;

    socket.simulateClose();
    expect(client.status().phase).toBe('app_not_running');

    // Advance past backoff[0]=500ms — reconnect probe must create a new socket.
    vi.advanceTimersByTime(600);
    expect(createdCount).toBeGreaterThan(countBefore);

    client.dispose();
  });

  it('does NOT schedule a reconnect after dispose() is called', async () => {
    const { client, socket } = await connectedClientRealTimers();

    // Dispose BEFORE the socket closes.
    client.dispose();

    vi.useFakeTimers();
    const countBefore = createdCount;

    socket.simulateClose();
    vi.advanceTimersByTime(2_000);

    // No new probes — dispose prevented the reconnect.
    expect(createdCount).toBe(countBefore);
  });
});

// ── native messaging transport ─────────────────────────────────────────────────

type PortListener = (msg: unknown, port?: unknown) => void;

interface FakePort {
  postMessage: ReturnType<typeof vi.fn>;
  disconnect: ReturnType<typeof vi.fn>;
  onMessage: { addListener: (cb: PortListener) => void };
  onDisconnect: { addListener: (cb: () => void) => void };
  simulateMessage: (obj: unknown) => void;
  simulateDisconnect: (lastError?: { message: string }) => void;
}

function buildFakePort(): FakePort {
  const msgListeners: PortListener[] = [];
  const discListeners: Array<() => void> = [];
  return {
    postMessage: vi.fn(),
    disconnect: vi.fn(),
    onMessage: { addListener: (cb: PortListener) => void msgListeners.push(cb) },
    onDisconnect: { addListener: (cb: () => void) => void discListeners.push(cb) },
    simulateMessage(obj: unknown) {
      msgListeners.forEach((cb) => cb(obj));
    },
    simulateDisconnect(lastError?: { message: string }) {
      (browser.runtime as { lastError?: unknown }).lastError = lastError;
      discListeners.forEach((cb) => cb());
    },
  };
}

describe('BridgeClient – native messaging transport', () => {
  const connectNativeMock = vi.mocked(browser.runtime.connectNative);
  let createdSockets: FakeWebSocket[] = [];
  let restoreWS: () => void;

  beforeEach(() => {
    createdSockets = [];
    restoreWS = installFakeWS((ws) => createdSockets.push(ws));
    (browser.runtime as { lastError?: unknown }).lastError = undefined;
  });

  afterEach(() => {
    restoreWS();
    connectNativeMock.mockReset();
    // Restore the suite default (throw) for any later file.
    connectNativeMock.mockImplementation(() => {
      throw new Error('connectNative not available');
    });
    vi.useRealTimers();
  });

  it('connects native-first on bridge.ready{ok:true} and round-trips an import via the port', async () => {
    const port = buildFakePort();
    connectNativeMock.mockReturnValue(port as never);

    const client = new BridgeClient(vi.fn());
    const connectPromise = client.ensureConnected();
    port.simulateMessage({ type: 'bridge.ready', ok: true });
    await connectPromise;

    expect(client.status().phase).toBe('connected');
    expect(createdSockets).toHaveLength(0); // never touched ws

    const importPromise = client.importJob(FAKE_TOKEN, makeImportRequest());
    await vi.waitFor(() => {
      expect(port.postMessage).toHaveBeenCalled();
    });
    const sent = port.postMessage.mock.calls[0]?.[0] as { reqId: string };
    expect(sent.type).toBe(EXTENSION_MESSAGE_TYPES.importRequest);

    // Reply arrives as a PARSED OBJECT (native auto-parses JSON), not a string.
    port.simulateMessage({
      type: EXTENSION_MESSAGE_TYPES.importResult,
      token: FAKE_TOKEN,
      reqId: sent.reqId,
      payload: { applicationId: 'native-1', status: 'saved' },
    });

    const result = await importPromise;
    expect(result).toEqual({ applicationId: 'native-1', status: 'saved' });

    client.dispose();
  });

  it('enters app_not_running on bridge.ready{ok:false} with NO ws fallback', async () => {
    vi.useFakeTimers();
    const port = buildFakePort();
    connectNativeMock.mockReturnValue(port as never);

    const client = new BridgeClient(vi.fn());
    const connectPromise = client.ensureConnected();
    port.simulateMessage({ type: 'bridge.ready', ok: false });
    await connectPromise;

    expect(port.disconnect).toHaveBeenCalled(); // ok:false closes the native port
    expect(client.status().phase).toBe('app_not_running');
    expect(createdSockets).toHaveLength(0); // app down ≠ fall back to ws

    // Reconnect scheduled — advance past backoff[0]=500ms; it re-tries native.
    const callsBefore = connectNativeMock.mock.calls.length;
    vi.advanceTimersByTime(600);
    expect(connectNativeMock.mock.calls.length).toBeGreaterThan(callsBefore);

    client.dispose();
  });

  it('falls back to the ws probe when connectNative throws', async () => {
    connectNativeMock.mockImplementation(() => {
      throw new Error('host not registered');
    });

    const client = new BridgeClient(vi.fn());
    const connectPromise = client.ensureConnected();

    await vi.waitFor(() => {
      expect(createdSockets.length).toBeGreaterThanOrEqual(1);
    });
    expect(createdSockets[0]!.url).toBe('ws://127.0.0.1:47615');
    createdSockets[0]!.simulateOpen();
    await connectPromise;

    expect(client.status()).toMatchObject({ phase: 'connected', port: 47615 });

    client.dispose();
  });

  it('falls back to ws when the port disconnects (lastError) before any bridge.ready', async () => {
    const port = buildFakePort();
    connectNativeMock.mockReturnValue(port as never);

    const client = new BridgeClient(vi.fn());
    const connectPromise = client.ensureConnected();

    // Host not registered: onDisconnect fires with lastError, before any ready.
    port.simulateDisconnect({ message: 'Native host has exited.' });

    await vi.waitFor(() => {
      expect(createdSockets.length).toBeGreaterThanOrEqual(1);
    });
    createdSockets[0]!.simulateOpen();
    await connectPromise;

    expect(client.status()).toMatchObject({ phase: 'connected', port: 47615 });

    client.dispose();
  });

  it('falls back to ws when no bridge.ready arrives within the ready timeout', async () => {
    vi.useFakeTimers();
    const port = buildFakePort();
    connectNativeMock.mockReturnValue(port as never);

    const client = new BridgeClient(vi.fn());
    const connectPromise = client.ensureConnected();

    // No ready frame — advance past READY_TIMEOUT_MS (1500ms) → fall back.
    await vi.advanceTimersByTimeAsync(1_600);

    await vi.waitFor(() => {
      expect(createdSockets.length).toBeGreaterThanOrEqual(1);
    });
    createdSockets[0]!.simulateOpen();
    await connectPromise;

    expect(port.disconnect).toHaveBeenCalled(); // timeout closes the native port
    expect(client.status()).toMatchObject({ phase: 'connected', port: 47615 });

    client.dispose();
  });
});
