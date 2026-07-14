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

import { EXTENSION_MESSAGE_TYPES, EXTENSION_PROTOCOL_VERSION } from '@ajh/shared';

import { BridgeClient } from './bridge';
import { computeProof } from './handshake';

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
  // v2 replies carry no token — the socket is already session-authenticated.
  return JSON.stringify({
    type: EXTENSION_MESSAGE_TYPES.importResult,
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

    const importPromise = client.importJob(makeImportRequest());

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

    const importPromise = client.importJob(makeImportRequest());

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

    const p1 = client.importJob(makeImportRequest());
    const p2 = client.importJob(makeImportRequest());

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
    const importPromise = client.importJob(makeImportRequest());
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
    await vi.advanceTimersByTimeAsync(600);
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

    const importPromise = client.importJob(makeImportRequest());
    await vi.waitFor(() => {
      expect(port.postMessage).toHaveBeenCalled();
    });
    const sent = port.postMessage.mock.calls[0]?.[0] as { reqId: string };
    expect(sent.type).toBe(EXTENSION_MESSAGE_TYPES.importRequest);

    // Reply arrives as a PARSED OBJECT (native auto-parses JSON), not a string.
    port.simulateMessage({
      type: EXTENSION_MESSAGE_TYPES.importResult,
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

// ── v2 mutual HMAC handshake ────────────────────────────────────────────────

describe('BridgeClient – v2 mutual handshake', () => {
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
    vi.useRealTimers();
  });

  /** A fixed, well-formed server nonce (16 bytes = 32 lowercase-hex chars). */
  const SERVER_NONCE = 'ffeeddccbbaa99887766554433221100';

  /** Build a BridgeClient whose getStoredToken returns the given value + open the socket. */
  async function clientWithToken(
    storedToken: string | null
  ): Promise<{ client: BridgeClient; socket: FakeWebSocket; connectPromise: Promise<void> }> {
    const getStoredToken = vi.fn<[], Promise<string | null>>().mockResolvedValue(storedToken);
    const client = new BridgeClient(vi.fn(), getStoredToken);
    const connectPromise = client.ensureConnected();
    await vi.waitFor(() => {
      expect(latestSocket).toBeDefined();
    });
    const socket = latestSocket!;
    socket.simulateOpen();
    return { client, socket, connectPromise };
  }

  const parseSend = (socket: FakeWebSocket, i: number) =>
    JSON.parse(socket.send.mock.calls[i]?.[0] as string) as {
      type: string;
      reqId: string;
      token?: unknown;
      payload: { clientNonce?: string; proof?: string };
    };

  /** Wait for the opening `hello` frame; return its reqId + clientNonce. */
  async function awaitHello(
    socket: FakeWebSocket
  ): Promise<{ helloReqId: string; clientNonce: string }> {
    await vi.waitFor(() => {
      expect(socket.send).toHaveBeenCalled();
    });
    const hello = parseSend(socket, 0);
    expect(hello.type).toBe(EXTENSION_MESSAGE_TYPES.hello);
    expect((hello.payload as { protocol?: number }).protocol).toBe(EXTENSION_PROTOCOL_VERSION);
    // The token is NEVER on the wire.
    expect(hello.token).toBeUndefined();
    return { helloReqId: hello.reqId, clientNonce: hello.payload.clientNonce! };
  }

  function sendChallenge(socket: FakeWebSocket, reqId: string): void {
    socket.simulateMessage(
      JSON.stringify({
        type: EXTENSION_MESSAGE_TYPES.challenge,
        reqId,
        payload: { serverNonce: SERVER_NONCE },
      })
    );
  }

  /** After the challenge, wait for the `auth` frame; return its reqId + proof. */
  async function awaitAuth(socket: FakeWebSocket): Promise<{ authReqId: string; proof: string }> {
    await vi.waitFor(() => {
      expect(socket.send.mock.calls.length).toBeGreaterThanOrEqual(2);
    });
    const auth = parseSend(socket, 1);
    expect(auth.type).toBe(EXTENSION_MESSAGE_TYPES.auth);
    expect(auth.token).toBeUndefined();
    return { authReqId: auth.reqId, proof: auth.payload.proof! };
  }

  async function sendAuthOk(
    socket: FakeWebSocket,
    reqId: string,
    token: string,
    clientNonce: string,
    kind: 'valid' | 'invalid' = 'valid'
  ): Promise<void> {
    const serverProof =
      kind === 'valid'
        ? await computeProof(token, 'server', SERVER_NONCE, clientNonce)
        : '0'.repeat(64);
    socket.simulateMessage(
      JSON.stringify({
        type: EXTENSION_MESSAGE_TYPES.authOk,
        reqId,
        payload: { serverProof },
      })
    );
  }

  it('completes the full handshake (hello→challenge→auth→auth.ok) and reaches connected', async () => {
    const { client, socket, connectPromise } = await clientWithToken(FAKE_TOKEN);

    const { helloReqId, clientNonce } = await awaitHello(socket);
    sendChallenge(socket, helloReqId);

    const { authReqId, proof } = await awaitAuth(socket);
    // The client proof must be the real HMAC(token, CLIENT_MSG) for the issued
    // nonces — proving the token is used as a key, never transmitted.
    expect(proof).toBe(await computeProof(FAKE_TOKEN, 'client', SERVER_NONCE, clientNonce));

    await sendAuthOk(socket, authReqId, FAKE_TOKEN, clientNonce, 'valid');
    await connectPromise;

    expect(client.status().phase).toBe('connected');
    client.dispose();
  });

  it('enters bad_token on an INVALID serverProof and sends NO import/profile frame (no PII)', async () => {
    const { client, socket, connectPromise } = await clientWithToken(FAKE_TOKEN);

    const { helloReqId, clientNonce } = await awaitHello(socket);
    sendChallenge(socket, helloReqId);
    const { authReqId } = await awaitAuth(socket);
    // A rogue/port-squatting peer cannot produce a valid serverProof.
    await sendAuthOk(socket, authReqId, FAKE_TOKEN, clientNonce, 'invalid');
    await connectPromise;

    expect(client.status().phase).toBe('bad_token');
    // CRITICAL: only hello + auth were ever sent — mutual auth failed BEFORE any
    // import/profile frame, so no PII ever left the extension.
    expect(socket.send.mock.calls.length).toBe(2);
    client.dispose();
  });

  // ── unverified-peer race: importJob/getProfile must never race ahead of the
  // handshake (a non-null transport does NOT mean the peer is verified) ────────

  it('a concurrent importJob during a PENDING handshake sends NO import.request, and rejects (sending nothing) once the handshake times out unresolved', async () => {
    // The attack this closes: a port-squatter answers `hello` with a `challenge`
    // then WITHHOLDS `auth.ok` (stays silent). If a concurrent `importJob` (the
    // user clicking Import mid-handshake) were gated on transport liveness
    // (`this.transport !== null`, set by `attach()` BEFORE the peer is verified)
    // instead of the authenticated session, it would ship the active-tab DOM to
    // this unverified peer. It must instead await the SAME handshake and see it
    // fail — sending nothing.
    vi.useFakeTimers();
    const getStoredToken = vi.fn<[], Promise<string | null>>().mockResolvedValue(FAKE_TOKEN);
    const client = new BridgeClient(vi.fn(), getStoredToken);

    // Kick off the initial connection attempt (mirrors the background's
    // module-load / reconnect-timer probe) — NOT awaited, so it is still
    // in-flight when importJob is called below.
    const initialConnect = client.ensureConnected();
    await vi.waitFor(() => {
      expect(latestSocket).toBeDefined();
    });
    const socket = latestSocket!;
    socket.simulateOpen();

    const { helloReqId } = await awaitHello(socket);
    sendChallenge(socket, helloReqId);
    await awaitAuth(socket); // client sent its auth{proof} — 2 frames total so far
    const framesBeforeImport = socket.send.mock.calls.length;
    expect(framesBeforeImport).toBe(2);

    // The user clicks "Import" WHILE the handshake is still awaiting auth.ok —
    // the port-squatter attack window. `this.transport` is already non-null here
    // (set by attach() at open time) — the OLD (unpatched) `ensureConnected()`
    // would see `isOpen()` true and return immediately, letting this send NOW.
    const importPromise = client.importJob(makeImportRequest());
    // Attach the outcome handler SYNCHRONOUSLY (same tick) so vitest never flags
    // a transient "unhandled rejection" while we advance timers below — Node's
    // unhandled-rejection check cares whether a handler is attached, not when the
    // promise actually settles.
    const outcomePromise = importPromise.then(
      () => ({ ok: true as const }),
      (e: unknown) => ({ ok: false as const, error: e })
    );

    // Flush microtasks and assert NO import.request was sent while the peer is
    // still unverified.
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    expect(socket.send.mock.calls.length).toBe(framesBeforeImport);

    // The port-squatter never replies — advance past the handshake timeout.
    await vi.advanceTimersByTimeAsync(8_100);
    await initialConnect;

    const outcome = await outcomePromise;
    expect(outcome.ok).toBe(false);
    if (!outcome.ok) {
      expect(outcome.error).toBeInstanceOf(Error);
      expect((outcome.error as Error).message).toBe(
        'Desktop app not reachable. Is AI Job Hunter running?'
      );
    }
    // CRITICAL: the import.request frame was NEVER sent — only hello + auth.
    expect(socket.send.mock.calls.length).toBe(2);
    expect(client.status().phase).not.toBe('connected');

    client.dispose();
  });

  it('a concurrent getProfile during a PENDING handshake sends NO profile.get, and rejects (sending nothing) if the peer closes without auth.ok', async () => {
    const { client, socket, connectPromise } = await clientWithToken(FAKE_TOKEN);
    const { helloReqId } = await awaitHello(socket);
    sendChallenge(socket, helloReqId);
    await awaitAuth(socket);
    const framesBeforeProfile = socket.send.mock.calls.length;
    expect(framesBeforeProfile).toBe(2);

    // Fetching the Contact Profile mid-handshake — the highest-sensitivity PII
    // this client sends. Must never race ahead of the mutual auth.
    const profilePromise = client.getProfile();
    // Attach synchronously — see the note in the importJob variant above.
    const outcomePromise = profilePromise.then(
      () => ({ ok: true as const }),
      (e: unknown) => ({ ok: false as const, error: e })
    );

    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    expect(socket.send.mock.calls.length).toBe(framesBeforeProfile);

    // The peer closes without ever sending auth.ok (ambiguous silence).
    socket.simulateClose();
    await connectPromise;

    const outcome = await outcomePromise;
    expect(outcome.ok).toBe(false);
    // CRITICAL: profile.get was NEVER sent — only hello + auth.
    expect(socket.send.mock.calls.length).toBe(2);
    expect(client.status().phase).toBe('app_not_running');

    client.dispose();
  });

  it('enters app_not_running (NOT bad_token) when the desktop closes without an auth.ok, and reconnect is allowed', async () => {
    // The Rust `Unauthorized` path closes WITHOUT a reply BY DESIGN (a wrong
    // proof and an app crash look identical on the wire) — this ambiguous
    // silence must never assert a hard wrong-token verdict; it must stay
    // recoverable so a genuine crash/restart doesn't strand the user in a
    // manual-re-pair state.
    vi.useFakeTimers();
    let socketCount = 0;
    restoreWS();
    restoreWS = installFakeWS((ws) => {
      socketCount += 1;
      latestSocket = ws;
    });

    const getStoredToken = vi.fn<[], Promise<string | null>>().mockResolvedValue(FAKE_TOKEN);
    const client = new BridgeClient(vi.fn(), getStoredToken);
    const connectPromise = client.ensureConnected();
    await vi.waitFor(() => {
      expect(latestSocket).toBeDefined();
    });
    const socket = latestSocket!;
    socket.simulateOpen();

    const { helloReqId } = await awaitHello(socket);
    sendChallenge(socket, helloReqId);
    await awaitAuth(socket);
    // Desktop spoke v2 (sent a challenge) but never replies auth.ok — closes.
    socket.simulateClose();
    await connectPromise;

    expect(client.status().phase).toBe('app_not_running');
    expect(client.status().phase).not.toBe('bad_token');
    // Reconnect IS scheduled (recoverable) — advance past backoff[0]=500ms.
    const before = socketCount;
    await vi.advanceTimersByTimeAsync(600);
    expect(socketCount).toBeGreaterThan(before);
    client.dispose();
  });

  it('resolves app_not_running (NOT bad_token) when the socket closes while computing the client proof', async () => {
    // Race: the socket closes in the window between receiving the challenge and
    // `performHandshake` noticing the transport is gone (after `await
    // computeProof(...)` resolves) — the "closed while hashing" branch. Same
    // ambiguity as above: never assert wrong-token from silence alone.
    const { client, socket, connectPromise } = await clientWithToken(FAKE_TOKEN);
    const { helloReqId } = await awaitHello(socket);
    sendChallenge(socket, helloReqId);
    // Close in the SAME synchronous tick as the challenge — `settle()` for step 1
    // clears `handshakeClosed` synchronously before `performHandshake`'s
    // continuation (which awaits `computeProof`) gets a chance to run as a
    // microtask, so this close is only noticed by the post-hash `!this.transport`
    // check — exactly the "closed while hashing" race, not the step-1 close path.
    socket.simulateClose();
    await connectPromise;

    expect(client.status().phase).toBe('app_not_running');
    expect(client.status().phase).not.toBe('bad_token');
    client.dispose();
  });

  it('enters outdated when the desktop closes without a challenge (old desktop)', async () => {
    const { client, socket, connectPromise } = await clientWithToken(FAKE_TOKEN);
    await awaitHello(socket);
    // An old desktop refuses the v2 hello and closes — no challenge ever arrives.
    socket.simulateClose();
    await connectPromise;
    expect(client.status().phase).toBe('outdated');
    client.dispose();
  });

  it('enters outdated when the desktop replies a non-challenge frame (legacy import.result)', async () => {
    const { client, socket, connectPromise } = await clientWithToken(FAKE_TOKEN);
    const { helloReqId } = await awaitHello(socket);
    // An old v1 desktop replies import.result{unauthorized} to our hello (it read
    // an empty token) instead of a challenge → we know it does not speak v2.
    socket.simulateMessage(
      JSON.stringify({
        type: EXTENSION_MESSAGE_TYPES.importResult,
        reqId: helloReqId,
        payload: { error: 'unauthorized' },
      })
    );
    await connectPromise;
    expect(client.status().phase).toBe('outdated');
    client.dispose();
  });

  it('enters outdated when the challenge carries a malformed serverNonce (defense-in-depth)', async () => {
    // Mirrors the Rust `is_valid_nonce` shape check: a serverNonce that is not
    // exactly 32 lowercase-hex chars must never feed the HMAC — reject it as a
    // clean handshake failure before any proof is computed.
    const { client, socket, connectPromise } = await clientWithToken(FAKE_TOKEN);
    const { helloReqId } = await awaitHello(socket);
    socket.simulateMessage(
      JSON.stringify({
        type: EXTENSION_MESSAGE_TYPES.challenge,
        reqId: helloReqId,
        payload: { serverNonce: 'not-hex!!' },
      })
    );
    await connectPromise;
    expect(client.status().phase).toBe('outdated');
    // The malformed nonce must never reach computeProof — only the hello frame
    // was ever sent (no auth frame follows a rejected challenge).
    expect(socket.send.mock.calls.length).toBe(1);
    client.dispose();
  });

  it('does NOT enter bad_token/outdated on a pure handshake timeout — reconnect allowed', async () => {
    vi.useFakeTimers();
    let socketCount = 0;
    restoreWS();
    restoreWS = installFakeWS((ws) => {
      socketCount += 1;
      latestSocket = ws;
    });

    const getStoredToken = vi.fn<[], Promise<string | null>>().mockResolvedValue(FAKE_TOKEN);
    const client = new BridgeClient(vi.fn(), getStoredToken);
    const connectPromise = client.ensureConnected();
    await vi.waitFor(() => {
      expect(latestSocket).toBeDefined();
    });
    const socket = latestSocket!;
    socket.simulateOpen();
    await awaitHello(socket);

    // No challenge, no close — advance just past the handshake timeout (8s). This
    // fires the timeout (→ app_not_running) and SCHEDULES the reconnect (500ms)
    // without firing it yet.
    await vi.advanceTimersByTimeAsync(8_100);
    await connectPromise.catch(() => {
      /* may reject; ignore */
    });

    // A silent timeout is a transport blip, not a token/outdated verdict.
    expect(client.status().phase).not.toBe('bad_token');
    expect(client.status().phase).not.toBe('outdated');
    // Reconnect scheduled — advance past backoff[0]=500ms; a new socket appears.
    const before = socketCount;
    await vi.advanceTimersByTimeAsync(600);
    expect(socketCount).toBeGreaterThan(before);
    client.dispose();
  });

  it('resetForNewToken() clears bad_token so ensureConnected() attempts a fresh socket', async () => {
    vi.useFakeTimers();
    let socketCount = 0;
    restoreWS();
    restoreWS = installFakeWS((ws) => {
      socketCount += 1;
      latestSocket = ws;
    });

    const getStoredToken = vi.fn<[], Promise<string | null>>().mockResolvedValue(FAKE_TOKEN);
    const client = new BridgeClient(vi.fn(), getStoredToken);
    const connectPromise = client.ensureConnected();
    await vi.waitFor(() => {
      expect(latestSocket).toBeDefined();
    });
    const socket = latestSocket!;
    socket.simulateOpen();

    const { helloReqId, clientNonce } = await awaitHello(socket);
    sendChallenge(socket, helloReqId);
    const { authReqId } = await awaitAuth(socket);
    // A genuine, non-ambiguous rejection: the desktop DID reply auth.ok, but the
    // serverProof does not verify → bad_token (a real mismatch, not silence).
    await sendAuthOk(socket, authReqId, FAKE_TOKEN, clientNonce, 'invalid');
    await connectPromise;
    expect(client.status().phase).toBe('bad_token');

    client.resetForNewToken();
    expect(client.status().phase).toBe('searching');

    const before = socketCount;
    void client.ensureConnected();
    await vi.advanceTimersByTimeAsync(0);
    expect(socketCount).toBeGreaterThan(before);
    client.dispose();
  });

  it('does NOT send any frame when no token is stored, and stays not-paired', async () => {
    const { client, socket, connectPromise } = await clientWithToken(null);
    await connectPromise;
    // No hello (nor any frame) is sent when unpaired.
    expect(socket.send).not.toHaveBeenCalled();
    // Phase is 'connected' from the bridge perspective (background → not_paired).
    expect(client.status().phase).toBe('connected');
    client.dispose();
  });
});

// ── assisted-autofill profile.get ↔ profile.result ─────────────────────────────

describe('BridgeClient – getProfile (assisted autofill)', () => {
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

  function makeProfileEnvelope(reqId: string, payload: unknown): string {
    return JSON.stringify({
      type: EXTENSION_MESSAGE_TYPES.profileResult,
      reqId,
      payload,
    });
  }

  /** Start a getProfile and wait until the outgoing frame is sent; return the reqId. */
  async function startProfile(
    client: BridgeClient,
    socket: FakeWebSocket
  ): Promise<{ profilePromise: Promise<unknown>; reqId: string }> {
    const profilePromise = client.getProfile();
    await vi.waitFor(() => {
      expect(socket.send).toHaveBeenCalled();
    });
    const raw = socket.send.mock.calls[socket.send.mock.calls.length - 1]?.[0] as string;
    const frame = JSON.parse(raw) as { type: string; reqId: string; payload: unknown };
    // The outgoing frame is a profile.get with a null payload (authed by token only).
    expect(frame.type).toBe(EXTENSION_MESSAGE_TYPES.profileGet);
    expect(frame.payload).toBeNull();
    return { profilePromise, reqId: frame.reqId };
  }

  it('round-trips a profile.result reply into the resolved profile fields', async () => {
    const { client, socket } = await connectedClient();
    const { profilePromise, reqId } = await startProfile(client, socket);

    const payload = {
      fullName: 'Saeed Kolivand',
      email: 'saeed@example.com',
      phone: '+31 6 1234 5678',
      linkedin: 'https://linkedin.com/in/saeed',
    };
    socket.simulateMessage(makeProfileEnvelope(reqId, payload));

    const result = await profilePromise;
    expect(result).toEqual(payload);

    client.dispose();
  });

  it('round-trips extraLinks (additive/optional field) into the resolved profile', async () => {
    const { client, socket } = await connectedClient();
    const { profilePromise, reqId } = await startProfile(client, socket);

    const payload = {
      email: 'saeed@example.com',
      extraLinks: [
        { label: 'Portfolio', url: 'https://saeed.dev' },
        { label: 'Dribbble', url: 'https://dribbble.com/saeed' },
      ],
    };
    socket.simulateMessage(makeProfileEnvelope(reqId, payload));

    const result = await profilePromise;
    expect(result).toEqual(payload);

    client.dispose();
  });

  it('tolerates the absence of extraLinks (old desktop reply, additive field)', async () => {
    const { client, socket } = await connectedClient();
    const { profilePromise, reqId } = await startProfile(client, socket);

    socket.simulateMessage(makeProfileEnvelope(reqId, { email: 'saeed@example.com' }));

    const result = (await profilePromise) as { email?: string; extraLinks?: unknown };
    expect(result.email).toBe('saeed@example.com');
    expect(result.extraLinks).toBeUndefined();

    client.dispose();
  });

  it('drops an extraLinks entry missing url, keeping the rest of the profile', async () => {
    const { client, socket } = await connectedClient();
    const { profilePromise, reqId } = await startProfile(client, socket);

    socket.simulateMessage(
      makeProfileEnvelope(reqId, {
        email: 'saeed@example.com',
        extraLinks: [{ label: 'Portfolio' }],
      })
    );

    const result = (await profilePromise) as {
      error?: string;
      email?: string;
      extraLinks?: unknown;
    };
    expect(result.error).toBeUndefined();
    expect(result.email).toBe('saeed@example.com');
    expect(result.extraLinks).toEqual([]);

    client.dispose();
  });

  it('resolves with a malformed error (never throws) when extraLinks is not an array', async () => {
    const { client, socket } = await connectedClient();
    const { profilePromise, reqId } = await startProfile(client, socket);

    socket.simulateMessage(makeProfileEnvelope(reqId, { extraLinks: 'https://saeed.dev' }));

    const result = (await profilePromise) as { error?: string };
    expect(result.error).toMatch(/malformed/i);

    client.dispose();
  });

  it('drops an extraLinks entry with a non-http(s) url, keeping the rest of the profile (never a payload-level malformed error)', async () => {
    const { client, socket } = await connectedClient();
    const { profilePromise, reqId } = await startProfile(client, socket);

    socket.simulateMessage(
      makeProfileEnvelope(reqId, {
        email: 'saeed@example.com',
        extraLinks: [{ label: 'Portfolio', url: 'javascript:alert(1)' }],
      })
    );

    const result = (await profilePromise) as {
      error?: string;
      email?: string;
      extraLinks?: unknown;
    };
    expect(result.error).toBeUndefined();
    expect(result.email).toBe('saeed@example.com');
    expect(result.extraLinks).toEqual([]);

    client.dispose();
  });

  it('filters a mixed valid/invalid extraLinks array down to only the valid entries', async () => {
    const { client, socket } = await connectedClient();
    const { profilePromise, reqId } = await startProfile(client, socket);

    socket.simulateMessage(
      makeProfileEnvelope(reqId, {
        extraLinks: [
          { label: 'Portfolio', url: 'https://saeed.dev' },
          { label: 'Bad Scheme', url: 'javascript:alert(1)' },
          { label: 'Missing Url' },
          { label: 'Dribbble', url: 'https://dribbble.com/saeed' },
        ],
      })
    );

    const result = (await profilePromise) as {
      error?: string;
      extraLinks?: { label: string; url: string }[];
    };
    expect(result.error).toBeUndefined();
    expect(result.extraLinks).toEqual([
      { label: 'Portfolio', url: 'https://saeed.dev' },
      { label: 'Dribbble', url: 'https://dribbble.com/saeed' },
    ]);

    client.dispose();
  });

  it('resolves with the refusal error when autofill is opted out on the desktop', async () => {
    const { client, socket } = await connectedClient();
    const { profilePromise, reqId } = await startProfile(client, socket);

    socket.simulateMessage(makeProfileEnvelope(reqId, { error: 'Autofill is off.' }));

    const result = (await profilePromise) as { error?: string };
    expect(result.error).toBe('Autofill is off.');

    client.dispose();
  });

  it('resolves with a malformed error (never throws) when the payload is bad', async () => {
    const { client, socket } = await connectedClient();
    const { profilePromise, reqId } = await startProfile(client, socket);

    // email must be an optional string; a number breaks the guard.
    socket.simulateMessage(makeProfileEnvelope(reqId, { email: 42 }));

    const result = (await profilePromise) as { error?: string; email?: unknown };
    expect(result.error).toMatch(/malformed/i);
    expect(result.email).toBeUndefined();

    client.dispose();
  });
});

// ── "have I already applied?" applied.check ↔ applied.result ──────────────────

describe('BridgeClient – checkApplied', () => {
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

  function makeAppliedEnvelope(reqId: string, payload: unknown): string {
    return JSON.stringify({
      type: EXTENSION_MESSAGE_TYPES.appliedResult,
      reqId,
      payload,
    });
  }

  /** Start a checkApplied and wait until the outgoing frame is sent; return the reqId. */
  async function startAppliedCheck(
    client: BridgeClient,
    socket: FakeWebSocket,
    url: string
  ): Promise<{ resultPromise: Promise<unknown>; reqId: string }> {
    const resultPromise = client.checkApplied(url);
    await vi.waitFor(() => {
      expect(socket.send).toHaveBeenCalled();
    });
    const raw = socket.send.mock.calls[socket.send.mock.calls.length - 1]?.[0] as string;
    const frame = JSON.parse(raw) as { type: string; reqId: string; payload: unknown };
    expect(frame.type).toBe(EXTENSION_MESSAGE_TYPES.appliedCheck);
    expect(frame.payload).toEqual({ url });
    return { resultPromise, reqId: frame.reqId };
  }

  it('round-trips a found+applied result into the resolved payload', async () => {
    const { client, socket } = await connectedClient();
    const url = 'https://jobs.example.com/posting/9';
    const { resultPromise, reqId } = await startAppliedCheck(client, socket, url);

    const payload = {
      found: true,
      applicationId: 'app-1',
      status: 'applied',
      title: 'Senior Rust Engineer',
      appliedAt: 1_718_000_000_000,
    };
    socket.simulateMessage(makeAppliedEnvelope(reqId, payload));

    const result = await resultPromise;
    expect(result).toEqual(payload);

    client.dispose();
  });

  it('round-trips a not-found result', async () => {
    const { client, socket } = await connectedClient();
    const { resultPromise, reqId } = await startAppliedCheck(
      client,
      socket,
      'https://jobs.example.com/posting/none'
    );

    socket.simulateMessage(makeAppliedEnvelope(reqId, { found: false }));

    const result = await resultPromise;
    expect(result).toEqual({ found: false });

    client.dispose();
  });

  it('resolves with a malformed error (never throws) when the payload is bad', async () => {
    const { client, socket } = await connectedClient();
    const { resultPromise, reqId } = await startAppliedCheck(
      client,
      socket,
      'https://jobs.example.com/posting/bad'
    );

    // found must be a boolean; a string breaks the guard.
    socket.simulateMessage(makeAppliedEnvelope(reqId, { found: 'yes' }));

    const result = (await resultPromise) as { found: boolean; error?: string };
    expect(result.found).toBe(false);
    expect(result.error).toMatch(/malformed/i);

    client.dispose();
  });
});

// ── "mark this URL applied" status.update ↔ status.result ─────────────────────

describe('BridgeClient – updateStatus', () => {
  let latestSocket: FakeWebSocket | undefined;
  let createdSockets: FakeWebSocket[] = [];
  let restoreWS: () => void;

  beforeEach(() => {
    latestSocket = undefined;
    createdSockets = [];
    restoreWS = installFakeWS((ws) => {
      latestSocket = ws;
      createdSockets.push(ws);
    });
  });

  afterEach(() => {
    restoreWS();
    vi.useRealTimers();
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

  function makeStatusEnvelope(reqId: string, payload: unknown): string {
    return JSON.stringify({
      type: EXTENSION_MESSAGE_TYPES.statusResult,
      reqId,
      payload,
    });
  }

  /** Start an updateStatus and wait until the outgoing frame is sent; return the reqId. */
  async function startUpdateStatus(
    client: BridgeClient,
    socket: FakeWebSocket,
    url: string
  ): Promise<{ resultPromise: Promise<unknown>; reqId: string }> {
    const resultPromise = client.updateStatus(url);
    await vi.waitFor(() => {
      expect(socket.send).toHaveBeenCalled();
    });
    const raw = socket.send.mock.calls[socket.send.mock.calls.length - 1]?.[0] as string;
    const frame = JSON.parse(raw) as { type: string; reqId: string; payload: unknown };
    expect(frame.type).toBe(EXTENSION_MESSAGE_TYPES.statusUpdate);
    expect(frame.payload).toEqual({ url, to: 'applied' });
    return { resultPromise, reqId: frame.reqId };
  }

  it('round-trips a success result into the resolved payload', async () => {
    const { client, socket } = await connectedClient();
    const url = 'https://jobs.example.com/posting/9';
    const { resultPromise, reqId } = await startUpdateStatus(client, socket, url);

    const payload = { ok: true, applicationId: 'app-1', status: 'applied' };
    socket.simulateMessage(makeStatusEnvelope(reqId, payload));

    const result = await resultPromise;
    expect(result).toEqual(payload);

    client.dispose();
  });

  it('round-trips a desktop-side refusal (ok:false + error) into the resolved payload — never rejects', async () => {
    const { client, socket } = await connectedClient();
    const { resultPromise, reqId } = await startUpdateStatus(
      client,
      socket,
      'https://jobs.example.com/posting/none'
    );

    socket.simulateMessage(
      makeStatusEnvelope(reqId, { ok: false, error: "couldn't find a saved job for this page" })
    );

    const result = await resultPromise;
    expect(result).toEqual({ ok: false, error: "couldn't find a saved job for this page" });

    client.dispose();
  });

  it('resolves with a malformed error (never throws) when the payload is bad', async () => {
    const { client, socket } = await connectedClient();
    const { resultPromise, reqId } = await startUpdateStatus(
      client,
      socket,
      'https://jobs.example.com/posting/bad'
    );

    // ok must be a boolean; a string breaks the guard.
    socket.simulateMessage(makeStatusEnvelope(reqId, { ok: 'yes' }));

    const result = (await resultPromise) as { ok: boolean; error?: string };
    expect(result.ok).toBe(false);
    expect(result.error).toMatch(/malformed/i);

    client.dispose();
  });

  it('rejects when not connected — every port fails and the ws probe exhausts', async () => {
    vi.useFakeTimers();

    const client = new BridgeClient(vi.fn());
    const updatePromise = client.updateStatus('https://jobs.example.com/posting/x');
    // Attach synchronously so vitest never flags a transient "unhandled
    // rejection" while the probe below runs to completion.
    const outcomePromise = updatePromise.then(
      () => ({ ok: true as const }),
      (e: unknown) => ({ ok: false as const, error: e })
    );

    for (let attempt = 0; attempt < 6; attempt += 1) {
      const idx = attempt;
      await vi.waitFor(() => {
        expect(createdSockets.length).toBeGreaterThanOrEqual(idx + 1);
      });
      createdSockets[idx]!.simulateClose();
    }

    const outcome = await outcomePromise;
    expect(outcome.ok).toBe(false);
    if (!outcome.ok) {
      expect(outcome.error).toBeInstanceOf(Error);
      expect((outcome.error as Error).message).toMatch(/not reachable/i);
    }
    expect(client.status().phase).toBe('app_not_running');

    client.dispose();
  });
});
