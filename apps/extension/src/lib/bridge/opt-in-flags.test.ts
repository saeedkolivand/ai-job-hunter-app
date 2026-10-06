// autotrackEnabled / autofillEnabled never reject: every failure (not connected,
// bad reply, send error, timeout, dropped connection) degrades to `false` (OFF).

import { describe, expect, it, vi } from 'vitest';

import { BridgeClient } from '../bridge';
import {
  connectedClient,
  failAllPorts,
  frameAt,
  reply,
  sendAndAwaitFrame,
  setupFakeWebSocket,
  T,
} from './test-support';

vi.mock('@wxt-dev/browser', () => import('./browser-mock'));

const fake = setupFakeWebSocket();

describe.each<[string, (c: BridgeClient) => Promise<boolean>, string, string]>([
  ['autotrackEnabled', (c) => c.autotrackEnabled(), T.autotrackCheck, T.autotrackResult],
  ['autofillEnabled', (c) => c.autofillEnabled(), T.autofillCheck, T.autofillResult],
])('BridgeClient – %s', (_name, call, sentType, replyType) => {
  it('sends a null-payload check and resolves true only on { enabled: true }', async () => {
    const { client, socket } = await connectedClient(fake);
    const { promise, frame } = await sendAndAwaitFrame(socket, () => call(client));
    expect(frame.type).toBe(sentType);
    expect(frame.payload).toBeNull();
    reply(socket, replyType, frame.reqId, { enabled: true });
    await expect(promise).resolves.toBe(true);
    client.dispose();
  });

  it.each<[string, unknown]>([
    ['an empty object', {}],
    ['a non-boolean enabled', { enabled: 'yes' }],
    ['enabled: false', { enabled: false }],
    ['a string payload', 'enabled'],
    ['a null payload', null],
  ])('resolves false on %s', async (_title, payload) => {
    const { client, socket } = await connectedClient(fake);
    const { promise, frame } = await sendAndAwaitFrame(socket, () => call(client));
    reply(socket, replyType, frame.reqId, payload);
    await expect(promise).resolves.toBe(false);
    client.dispose();
  });

  it('resolves false when not connected (every port fails)', async () => {
    vi.useFakeTimers();
    const client = new BridgeClient(vi.fn());
    const promise = call(client);
    await failAllPorts(fake);
    await expect(promise).resolves.toBe(false);
    client.dispose();
  });

  it('resolves false when the send throws', async () => {
    const { client, socket } = await connectedClient(fake);
    socket.send.mockImplementation(() => {
      throw new Error('socket is closing');
    });
    await expect(call(client)).resolves.toBe(false);
    client.dispose();
  });

  it('resolves false when no reply arrives within the request timeout', async () => {
    vi.useFakeTimers();
    const { client, socket } = await connectedClient(fake);
    const { promise } = await sendAndAwaitFrame(socket, () => call(client));
    await vi.advanceTimersByTimeAsync(30_000);
    await expect(promise).resolves.toBe(false);
    expect(frameAt(socket).type).toBe(sentType);
    client.dispose();
  });

  it('resolves false when the connection drops while the check is pending', async () => {
    const { client, socket } = await connectedClient(fake);
    const { promise } = await sendAndAwaitFrame(socket, () => call(client));
    socket.simulateClose();
    await expect(promise).resolves.toBe(false);
    client.dispose();
  });
});
