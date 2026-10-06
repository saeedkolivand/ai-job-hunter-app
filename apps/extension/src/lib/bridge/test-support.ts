/**
 * Shared fixtures for the BridgeClient tests: a fake `WebSocket`, a fake native
 * `Port`, connected-client builders, frame helpers, the v2 handshake script, and
 * the round-trip table runners every verb suite shares.
 *
 * BridgeClient uses `new WebSocket(url)` as a raw global, so `installFakeWS`
 * replaces `globalThis.WebSocket` before any client method runs. Replies are
 * correlated with the reqId captured from the OUTGOING frame (never a stubbed
 * `crypto.randomUUID`). `vi.waitFor` retries while its callback THROWS — always
 * assert with `expect()` inside it.
 */

import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { EXTENSION_MESSAGE_TYPES, EXTENSION_PROTOCOL_VERSION } from '@ajh/shared';

import { BridgeClient } from '../bridge';
import { computeProof } from '../handshake';

export const FAKE_TOKEN = 'a'.repeat(64);
/** A fixed, well-formed server nonce (16 bytes = 32 lowercase-hex chars). */
export const SERVER_NONCE = 'ffeeddccbbaa99887766554433221100';
export const T = EXTENSION_MESSAGE_TYPES;

// ── WebSocket fake ────────────────────────────────────────────────────────────

type WSEventType = 'open' | 'close' | 'error' | 'message';

export interface FakeWebSocket {
  url: string;
  readyState: number;
  close: ReturnType<typeof vi.fn>;
  send: ReturnType<typeof vi.fn>;
  simulateOpen: () => void;
  simulateClose: () => void;
  simulateMessage: (data: string) => void;
}

const WS_STATES = { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 };

function buildFakeWS(url: string): FakeWebSocket {
  const listeners: Partial<Record<WSEventType, Array<(ev?: unknown) => void>>> = {};
  const fire = (type: WSEventType, ev?: unknown): void => listeners[type]?.forEach((cb) => cb(ev));
  const ws = {
    url,
    readyState: WS_STATES.CONNECTING,
    close: vi.fn(() => ws.simulateClose()),
    send: vi.fn(),
    addEventListener: vi.fn((type: WSEventType, cb: (ev?: unknown) => void) => {
      (listeners[type] ??= []).push(cb);
    }),
    simulateOpen() {
      ws.readyState = WS_STATES.OPEN;
      fire('open');
    },
    simulateClose() {
      ws.readyState = WS_STATES.CLOSED;
      fire('close');
    },
    simulateMessage: (data: string) => fire('message', { data }),
  };
  return ws;
}

/** Replace globalThis.WebSocket with a fake factory; returns a restore fn. */
function installFakeWS(onNew: (ws: FakeWebSocket) => void): () => void {
  const FakeConstructor = function (url: string) {
    const ws = buildFakeWS(url);
    onNew(ws);
    return ws;
  } as unknown as typeof WebSocket;
  Object.assign(FakeConstructor, WS_STATES);
  const original = globalThis.WebSocket;
  globalThis.WebSocket = FakeConstructor;
  return () => {
    globalThis.WebSocket = original;
  };
}

export interface FakeWS {
  /** Every socket the client created, in order. */
  sockets: FakeWebSocket[];
  /** Resolves with the next socket the client has created that this helper has not handed out yet. */
  next: () => Promise<FakeWebSocket>;
}

/** Register the fake-WebSocket lifecycle hooks for the enclosing suite. */
export function setupFakeWebSocket(): FakeWS {
  let restore = (): void => {};
  let handedOut = 0;
  const fake: FakeWS = {
    sockets: [],
    async next() {
      const i = handedOut;
      await vi.waitFor(() => {
        expect(fake.sockets.length).toBeGreaterThan(i);
      });
      handedOut = i + 1;
      return fake.sockets[i] as FakeWebSocket;
    },
  };
  beforeEach(() => {
    fake.sockets = [];
    handedOut = 0;
    restore = installFakeWS((ws) => fake.sockets.push(ws));
  });
  afterEach(() => {
    restore();
    vi.useRealTimers();
  });
  return fake;
}

/** Fail every port of the probe range (47615..=47620) so the client lands in `app_not_running`. */
export async function failAllPorts(fake: FakeWS): Promise<void> {
  for (let i = 0; i < 6; i += 1) {
    await vi.waitFor(() => {
      expect(fake.sockets.length).toBeGreaterThanOrEqual(i + 1);
    });
    (fake.sockets[i] as FakeWebSocket).simulateClose();
  }
}

// ── clients + frames ──────────────────────────────────────────────────────────

/** A client with no stored token, connected over the first fake socket. */
export async function connectedClient(
  fake: FakeWS
): Promise<{ client: BridgeClient; socket: FakeWebSocket }> {
  const client = new BridgeClient(vi.fn());
  const connecting = client.ensureConnected();
  const socket = await fake.next();
  socket.simulateOpen();
  await connecting;
  return { client, socket };
}

/** A client whose stored token is `storedToken`; the socket is open but `connectPromise` is pending the handshake. */
export async function clientWithToken(
  fake: FakeWS,
  storedToken: string | null,
  onTokenRevoked?: () => Promise<void> | void
): Promise<{ client: BridgeClient; socket: FakeWebSocket; connectPromise: Promise<void> }> {
  const client = new BridgeClient(vi.fn(), () => Promise.resolve(storedToken), onTokenRevoked);
  const connectPromise = client.ensureConnected();
  const socket = await fake.next();
  socket.simulateOpen();
  return { client, socket, connectPromise };
}

export interface Frame {
  type: string;
  reqId: string;
  token?: unknown;
  payload: unknown;
}

/** The `i`-th (default: last) frame the client sent on `socket`, parsed. */
export function frameAt(socket: FakeWebSocket, i = -1): Frame {
  return JSON.parse(socket.send.mock.calls.at(i)?.[0] as string) as Frame;
}

/** Run `start`, wait for the frame it sends, and return the pending promise + that frame. */
export async function sendAndAwaitFrame<R>(
  socket: FakeWebSocket,
  start: () => Promise<R>
): Promise<{ promise: Promise<R>; frame: Frame }> {
  const before = socket.send.mock.calls.length;
  const promise = start();
  await vi.waitFor(() => {
    expect(socket.send.mock.calls.length).toBeGreaterThan(before);
  });
  return { promise, frame: frameAt(socket) };
}

/** Deliver a reply frame from the desktop. */
export function reply(socket: FakeWebSocket, type: string, reqId: string, payload: unknown): void {
  socket.simulateMessage(JSON.stringify({ type, reqId, payload }));
}

/** Attach the outcome handler in the same tick so vitest never flags a transient unhandled rejection. */
export function outcomeOf(promise: Promise<unknown>) {
  return promise.then(
    () => ({ ok: true as const }),
    (e: unknown) => ({ ok: false as const, error: e })
  );
}

/** Let queued microtasks run without advancing macrotask timers. */
export async function flushMicrotasks(): Promise<void> {
  for (let i = 0; i < 3; i += 1) await Promise.resolve();
}

// ── per-verb round trips ──────────────────────────────────────────────────────

export interface VerbCase {
  call: (client: BridgeClient) => Promise<unknown>;
  sentType: string;
  sentPayload: unknown;
  replyType: string;
}

/** Connect, send the verb (asserting its frame), answer with `replyPayload`, return what the caller sees. */
export async function exchange(fake: FakeWS, verb: VerbCase, replyPayload: unknown) {
  const { client, socket } = await connectedClient(fake);
  const { promise, frame } = await sendAndAwaitFrame(socket, () => verb.call(client));
  expect(frame.type).toBe(verb.sentType);
  expect(frame.payload).toEqual(verb.sentPayload);
  reply(socket, verb.replyType, frame.reqId, replyPayload);
  const result = await promise;
  client.dispose();
  return result;
}

/** One `it` per case: the caller sees `expected` (default: the reply payload itself). */
export function itRoundTrips(
  fake: FakeWS,
  verb: VerbCase,
  cases: Array<[title: string, payload: unknown, expected?: unknown]>
): void {
  it.each(cases)('%s', async (_title, payload, expected = payload) => {
    expect(await exchange(fake, verb, payload)).toEqual(expected);
  });
}

/** One `it` per case: a bad payload resolves (never throws) with a /malformed/ `error` (and `[flag]: false` when the verb has one). */
export function itMalformed(
  fake: FakeWS,
  verb: VerbCase,
  cases: Array<[title: string, payload: unknown]>,
  flag?: string
): void {
  it.each(cases)('%s', async (_title, payload) => {
    expect(await exchange(fake, verb, payload)).toMatchObject({
      ...(flag ? { [flag]: false } : {}),
      error: expect.stringMatching(/malformed/i),
    });
  });
}

// ── v2 handshake script ───────────────────────────────────────────────────────

/** Wait for the opening `hello` frame (token NEVER on the wire); return its reqId + clientNonce. */
export async function awaitHello(
  socket: FakeWebSocket
): Promise<{ helloReqId: string; clientNonce: string }> {
  await vi.waitFor(() => {
    expect(socket.send).toHaveBeenCalled();
  });
  const hello = frameAt(socket, 0);
  expect(hello.type).toBe(T.hello);
  const helloPayload = hello.payload as { protocol: number; clientNonce: string };
  expect(helloPayload.protocol).toBe(EXTENSION_PROTOCOL_VERSION);
  expect(hello.token).toBeUndefined();
  return { helloReqId: hello.reqId, clientNonce: helloPayload.clientNonce };
}

export function sendChallenge(socket: FakeWebSocket, reqId: string): void {
  reply(socket, T.challenge, reqId, { serverNonce: SERVER_NONCE });
}

/** After the challenge, wait for the `auth` frame; return its reqId + proof. */
export async function awaitAuth(
  socket: FakeWebSocket
): Promise<{ authReqId: string; proof: string }> {
  await vi.waitFor(() => {
    expect(socket.send.mock.calls.length).toBeGreaterThanOrEqual(2);
  });
  const auth = frameAt(socket, 1);
  expect(auth.type).toBe(T.auth);
  expect(auth.token).toBeUndefined();
  return { authReqId: auth.reqId, proof: (auth.payload as { proof: string }).proof };
}

export async function sendAuthOk(
  socket: FakeWebSocket,
  reqId: string,
  clientNonce: string,
  kind: 'valid' | 'invalid' = 'valid'
): Promise<void> {
  const serverProof =
    kind === 'valid'
      ? await computeProof(FAKE_TOKEN, 'server', SERVER_NONCE, clientNonce)
      : '0'.repeat(64);
  reply(socket, T.authOk, reqId, { serverProof });
}

/** hello → challenge → auth, stopping before the desktop answers `auth.ok`. */
export async function handshakeUpToAuth(socket: FakeWebSocket) {
  const { helloReqId, clientNonce } = await awaitHello(socket);
  sendChallenge(socket, helloReqId);
  return { clientNonce, ...(await awaitAuth(socket)) };
}

/** The full handshake with the desktop answering `auth.ok` (valid or forged serverProof). */
export async function runHandshake(socket: FakeWebSocket, kind: 'valid' | 'invalid' = 'valid') {
  const steps = await handshakeUpToAuth(socket);
  await sendAuthOk(socket, steps.authReqId, steps.clientNonce, kind);
  return steps;
}
