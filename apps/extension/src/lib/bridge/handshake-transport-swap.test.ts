import { describe, expect, it, vi } from 'vitest';

import { EXTENSION_MESSAGE_TYPES } from '@ajh/shared/extension-protocol';

import {
  awaitHello,
  clientWithToken,
  FAKE_TOKEN,
  sendChallenge,
  setupFakeWebSocket,
} from './test-support';

vi.mock('@wxt-dev/browser', () => import('./browser-mock'));

const fake = setupFakeWebSocket();

describe('Handshake – transport replaced while the proof is computed', () => {
  it('never sends `auth` on a transport that did not issue the challenge', async () => {
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    const { helloReqId } = await awaitHello(socket);

    // A newer attach replaced the transport; it has not been handshaken at all.
    const replacement = { send: vi.fn(), close: vi.fn(), onMessage: vi.fn(), onClose: vi.fn() };
    // The challenge is consumed synchronously; the proof `await` is still ahead.
    sendChallenge(socket, helloReqId);
    (client as unknown as { transport: unknown }).transport = replacement;

    // Let the proof `await` finish and the handshake act on the swapped transport.
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(replacement.send).not.toHaveBeenCalled();
    // Nor on the original one: only its `hello` ever went out.
    expect(socket.send).toHaveBeenCalledTimes(1);
    expect(JSON.parse(socket.send.mock.calls[0]?.[0] as string).type).toBe(
      EXTENSION_MESSAGE_TYPES.hello
    );
    expect(client.status().authenticated).toBe(false);
    // Settle the (otherwise parked) handshake step so nothing leaks.
    client.dispose();
    await connectPromise;
  });
});
