import { describe, expect, it, vi } from 'vitest';

import { BridgeClient } from '../bridge';
import {
  connectedClient,
  exchange,
  failAllPorts,
  flushMicrotasks,
  frameAt,
  itRoundTrips,
  reply,
  sendAndAwaitFrame,
  setupFakeWebSocket,
  T,
  type VerbCase,
} from './test-support';

vi.mock('@wxt-dev/browser', () => import('./browser-mock'));

const fake = setupFakeWebSocket();

const IMPORT = { url: 'https://example.com/job/123', applied: false };
const importVerb: VerbCase = {
  call: (c) => c.importJob(IMPORT),
  sentType: T.importRequest,
  sentPayload: IMPORT,
  replyType: T.importResult,
};

describe('BridgeClient – port probe', () => {
  it('resolves on the first port that opens (47615) and does not probe further', async () => {
    const client = new BridgeClient(vi.fn());
    const connecting = client.ensureConnected();
    const first = await fake.next();
    expect(first.url).toBe('ws://127.0.0.1:47615');

    first.simulateOpen();
    await connecting;

    // Only one socket — probe stopped at first success.
    expect(fake.sockets).toHaveLength(1);
    expect(client.isOpen()).toBe(true);
    expect(client.status()).toMatchObject({ phase: 'connected', port: 47615 });
    client.dispose();
  });

  it('skips a failing port and connects on the next available one', async () => {
    const client = new BridgeClient(vi.fn());
    const connecting = client.ensureConnected();
    (await fake.next()).simulateClose();
    const second = await fake.next();
    expect(second.url).toBe('ws://127.0.0.1:47616');
    second.simulateOpen();
    await connecting;

    // Two sockets created; probe stopped after the second succeeded.
    expect(fake.sockets).toHaveLength(2);
    expect(client.status()).toMatchObject({ phase: 'connected', port: 47616 });
    client.dispose();
  });

  it('enters app_not_running and schedules a reconnect when all ports fail', async () => {
    vi.useFakeTimers();
    const client = new BridgeClient(vi.fn());
    const connecting = client.ensureConnected();
    await failAllPorts(fake);
    await connecting;
    expect(client.status().phase).toBe('app_not_running');

    // Advance past backoff[0]=500ms — reconnect probe must create a new socket.
    const countBefore = fake.sockets.length;
    vi.advanceTimersByTime(600);
    await vi.waitFor(() => {
      expect(fake.sockets.length).toBeGreaterThan(countBefore);
    });
    client.dispose();
  });
});

describe('BridgeClient – request/reply correlation', () => {
  it('resolves the correct pending promise when a matching reqId reply arrives', async () => {
    const replyPayload = { applicationId: 'app-xyz', status: 'saved' };
    expect(await exchange(fake, importVerb, replyPayload)).toEqual(replyPayload);
  });

  it('ignores a reply whose reqId does not match any pending request', async () => {
    vi.useFakeTimers();
    const { client, socket } = await connectedClient(fake);
    const { promise } = await sendAndAwaitFrame(socket, () => client.importJob(IMPORT));

    // Stray reply with a different reqId — must NOT resolve the pending promise.
    reply(socket, T.importResult, 'stray-id-999', { applicationId: 'should-not-resolve' });

    // Deterministic check: attach a .then spy, flush all microtasks (no timer
    // advancement) and assert it never fired.
    const thenSpy = vi.fn();
    promise.then(thenSpy).catch(() => {
      // ignore — we only care whether then fired
    });
    await flushMicrotasks();
    expect(thenSpy).not.toHaveBeenCalled();

    vi.useRealTimers();
    client.dispose();
  });

  it('ignores a reply of the wrong TYPE for a pending reqId, then still resolves on the right one', async () => {
    const { client, socket } = await connectedClient(fake);
    const { promise, frame } = await sendAndAwaitFrame(socket, () => client.importJob(IMPORT));

    // A profile.result echoing the import's reqId must not resolve the import.
    reply(socket, T.profileResult, frame.reqId, { email: 'x@example.com' });
    const settled = vi.fn();
    void promise.then(settled, settled);
    await flushMicrotasks();
    expect(settled).not.toHaveBeenCalled();

    reply(socket, T.importResult, frame.reqId, { applicationId: 'a' });
    expect(await promise).toEqual({ applicationId: 'a' });
    client.dispose();
  });

  it('resolves two concurrent requests independently via their reqIds', async () => {
    const { client, socket } = await connectedClient(fake);
    const p1 = client.importJob(IMPORT);
    const p2 = client.importJob(IMPORT);
    await vi.waitFor(() => {
      // hello + auth precede the two requests.
      expect(socket.send.mock.calls.length).toBeGreaterThanOrEqual(4);
    });

    const reqId1 = frameAt(socket, 2).reqId;
    const reqId2 = frameAt(socket, 3).reqId;
    // Two distinct reqIds must have been generated.
    expect(reqId1).not.toBe(reqId2);

    // Reply to second request first.
    reply(socket, T.importResult, reqId2, { applicationId: 'b', status: 'saved' });
    expect(await p2).toMatchObject({ applicationId: 'b' });
    reply(socket, T.importResult, reqId1, { applicationId: 'a', status: 'saved' });
    expect(await p1).toMatchObject({ applicationId: 'a' });
    client.dispose();
  });
});

describe('BridgeClient – payload validation on incoming import.result', () => {
  it('returns a malformed-result error when payload fails the import-result guard', async () => {
    // applicationId must be an optional string; a number breaks the guard.
    const result = (await exchange(fake, importVerb, { applicationId: 12345, status: false })) as {
      error?: string;
      applicationId?: unknown;
    };
    // Must resolve (not throw) with a malformed error — never trust bad data.
    expect(result.error).toMatch(/malformed/i);
    expect(result.applicationId).toBeUndefined();
  });

  it('does NOT return a success payload when the whole payload is a primitive', async () => {
    const result = (await exchange(fake, importVerb, 'this-is-a-string-not-an-object')) as {
      error?: string;
    };
    expect(result.error).toBeDefined();
    // The error must come from the bridge guard, not be the raw payload string.
    expect(result.error).not.toBe('this-is-a-string-not-an-object');
  });

  itRoundTrips(fake, importVerb, [
    [
      'resolves cleanly when a well-formed payload passes schema validation',
      { applicationId: 'app-good', status: 'saved' },
    ],
  ]);
});

describe('BridgeClient – reconnect/backoff on close', () => {
  it('schedules a reconnect timer after the connected socket closes unexpectedly', async () => {
    const { client, socket } = await connectedClient(fake);
    expect(client.status().phase).toBe('connected');

    vi.useFakeTimers();
    const countBefore = fake.sockets.length;
    socket.simulateClose();
    expect(client.status().phase).toBe('app_not_running');

    // Advance past backoff[0]=500ms — reconnect probe must create a new socket.
    await vi.advanceTimersByTimeAsync(600);
    expect(fake.sockets.length).toBeGreaterThan(countBefore);
    client.dispose();
  });

  it('does NOT schedule a reconnect after dispose() is called', async () => {
    const { client, socket } = await connectedClient(fake);
    client.dispose(); // BEFORE the socket closes

    vi.useFakeTimers();
    const countBefore = fake.sockets.length;
    socket.simulateClose();
    vi.advanceTimersByTime(2_000);

    // No new probes — dispose prevented the reconnect.
    expect(fake.sockets.length).toBe(countBefore);
  });
});
