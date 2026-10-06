// `token.revoked` (the desktop rotated its pairing secret). Without this frame a
// rotation stranded the extension: its reconnect fails the proof check, the
// desktop closes WITHOUT a reply (no token oracle, by design), and the handshake
// can only read that silence as the recoverable `app_not_running` — so it
// retried the dead token forever and the popup never returned to the pairing view.

import { describe, expect, it, vi } from 'vitest';

import { BridgeClient } from '../bridge';
import {
  awaitHello,
  clientWithToken,
  FAKE_TOKEN,
  type FakeWebSocket,
  reply,
  runHandshake,
  sendChallenge,
  setupFakeWebSocket,
  T,
} from './test-support';

vi.mock('@wxt-dev/browser', () => import('./browser-mock'));

const fake = setupFakeWebSocket();

const sendTokenRevoked = (socket: FakeWebSocket): void =>
  reply(socket, T.tokenRevoked, 'token-revoked', null);

/** An authenticated client whose stored token is dropped by `onTokenRevoked` (as the background does). */
async function authenticatedClient() {
  let stored: string | null = FAKE_TOKEN;
  const onTokenRevoked = vi.fn(() => {
    stored = null; // mirrors the background's `unpairLocally` (clears storage)
    return Promise.resolve();
  });
  const client = new BridgeClient(vi.fn(), () => Promise.resolve(stored), onTokenRevoked);
  const connectPromise = client.ensureConnected();
  const socket = await fake.next();
  socket.simulateOpen();
  await runHandshake(socket);
  await connectPromise;
  expect(client.status().phase).toBe('connected');
  return { client, socket, onTokenRevoked };
}

describe('BridgeClient – token.revoked', () => {
  it('drops the stored token on `token.revoked` and re-probes UNPAIRED (pairing view)', async () => {
    const { client, socket, onTokenRevoked } = await authenticatedClient();
    const socketsBefore = fake.sockets.length;

    // The desktop revokes this session, then closes the socket (exactly what
    // `handle_connection`'s revoke arm does).
    sendTokenRevoked(socket);
    socket.simulateClose();
    expect(onTokenRevoked).toHaveBeenCalledTimes(1);

    // ONE immediate re-probe — no backoff loop hammering the dead secret.
    const reprobe = await fake.next();
    expect(fake.sockets.length).toBe(socketsBefore + 1);
    reprobe.simulateOpen();
    await vi.waitFor(() => {
      expect(client.status().phase).toBe('connected');
    });
    // The critical assertion: NOT ONE frame on the new socket. The dead token is
    // never handshaked again — an open socket with no stored token is what the
    // background's `computeStatus` folds into `not_paired`, i.e. the popup shows
    // the pairing view instead of a permanent "app not running".
    expect(reprobe.send).not.toHaveBeenCalled();

    // …and that unpaired socket reached `connected` WITHOUT any handshake, so it
    // must not itself be revokable: `authenticated` was cleared on the close and
    // nothing re-earned it. Otherwise whoever holds the port could keep firing
    // revokes at the re-probe socket forever.
    sendTokenRevoked(reprobe);
    expect(onTokenRevoked).toHaveBeenCalledTimes(1);
    client.dispose();
  });

  it('does NOT authenticate a handshake whose socket died during the serverProof await', async () => {
    // `authenticated` is set AFTER `await computeProof(...)`. If the socket closes
    // inside that await, `onClose` clears the flag and the continuation would set
    // it straight back — leaving `authenticated === true` with no transport until
    // the next `attach`, so the next frame on that dead transport would be trusted.
    const onTokenRevoked = vi.fn(() => Promise.resolve());
    const { client, socket, connectPromise } = await clientWithToken(
      fake,
      FAKE_TOKEN,
      onTokenRevoked
    );
    await runHandshake(socket); // a genuine auth.ok — then the socket dies before the verdict is applied
    socket.simulateClose();
    await connectPromise;

    // The verdict belonged to a transport that no longer exists: it must not
    // resurrect the session, nor claim `connected` on a null transport.
    expect(client.status().phase).not.toBe('connected');
    sendTokenRevoked(socket);
    expect(onTokenRevoked).not.toHaveBeenCalled();
    client.dispose();
  });

  it('ignores `token.revoked` from a REPLACED transport while the live session stays authenticated', async () => {
    const { client, socket, onTokenRevoked } = await authenticatedClient();
    // A later attach replaced the transport; `socket` is now an orphan.
    (client as unknown as { transport: unknown }).transport = {
      send: vi.fn(),
      close: vi.fn(),
      onMessage: vi.fn(),
      onClose: vi.fn(),
    };

    sendTokenRevoked(socket);

    expect(onTokenRevoked).not.toHaveBeenCalled();
    expect(client.status().authenticated).toBe(true);
  });

  it('ignores a stale `token.revoked` delivered after the socket already closed', async () => {
    // The session dies with the socket, so a late/queued frame on the dead
    // transport's listener must not still count as authenticated.
    const { client, socket, onTokenRevoked } = await authenticatedClient();

    socket.simulateClose(); // ordinary close, no revoke
    sendTokenRevoked(socket); // late frame on the now-dead transport

    expect(onTokenRevoked).not.toHaveBeenCalled();
    client.dispose();
  });

  it('stays coherent if the desktop sends `token.revoked` but never closes', async () => {
    // We deliberately do NOT close the transport ourselves. If the close never
    // comes, the socket stays open with no stored token — which `computeStatus`
    // already folds into `not_paired`, the same end state, so nothing is stranded.
    const { client, socket, onTokenRevoked } = await authenticatedClient();

    sendTokenRevoked(socket);

    expect(onTokenRevoked).toHaveBeenCalledTimes(1);
    expect(client.status().phase).toBe('connected'); // + no token ⇒ not_paired
    client.dispose();
  });

  it('IGNORES `token.revoked` from an UNAUTHENTICATED peer mid-handshake', async () => {
    // A port-squatter needs ZERO token knowledge to send a syntactically-valid
    // `challenge`. The handshake frame hook is a step-in-flight LATCH, not an
    // auth check — it is cleared the instant the challenge is consumed, so the
    // very NEXT frame, delivered while `computeProof` is still awaiting, falls
    // through to the type dispatch. Honoring `token.revoked` there would let any
    // process that wins the loopback port destroy the stored pairing credential
    // in a tight loop. Only a session that completed mutual auth may revoke.
    const onTokenRevoked = vi.fn(() => Promise.resolve());
    const { client, socket, connectPromise } = await clientWithToken(
      fake,
      FAKE_TOKEN,
      onTokenRevoked
    );

    const { helloReqId } = await awaitHello(socket);
    sendChallenge(socket, helloReqId); // valid SHAPE, proves nothing
    sendTokenRevoked(socket); // lands in the post-settle / pre-auth.ok window

    // Let the handshake run past the `computeProof` await (the `auth` frame is
    // the observable proof we got through that window).
    await vi.waitFor(() => {
      expect(socket.send.mock.calls.length).toBeGreaterThanOrEqual(2);
    });

    expect(onTokenRevoked).not.toHaveBeenCalled();
    client.dispose();
    await connectPromise.catch(() => {
      /* handshake abandoned by dispose */
    });
  });

  it('honors a `token.revoked` that arrives right behind auth.ok', async () => {
    // The mirror of the unauthenticated-drop guard: the desktop marks a socket
    // authenticated when it SENDS `auth.ok`, so it can rotate and revoke in the
    // very next frame. If any `await` sits between our receipt of `auth.ok` and
    // the `authenticated` set, that legitimate revoke is dropped and the
    // extension is stranded on a dead token.
    const onTokenRevoked = vi.fn(() => Promise.resolve());
    const { client, socket, connectPromise } = await clientWithToken(
      fake,
      FAKE_TOKEN,
      onTokenRevoked
    );
    await runHandshake(socket);
    // Exactly ONE microtask turn — NOT `await connectPromise`, which would let
    // the whole handshake finish and make this pass either way. This is the gap a
    // real socket leaves between two consecutive messages: step 5 must be fully
    // synchronous to have completed inside it.
    await Promise.resolve();
    sendTokenRevoked(socket);

    expect(onTokenRevoked).toHaveBeenCalledTimes(1);
    await connectPromise;
    client.dispose();
  });

  it('surfaces bad_token when the stored token could NOT be cleared', async () => {
    // If storage refuses the delete, the token we KNOW is dead is still on disk —
    // reconnecting would handshake it, get the silent failed-proof close, and
    // resume the forever-retry loop. So: no re-probe at all, and `bad_token`,
    // whose popup view is the re-pair prompt.
    const onTokenRevoked = vi.fn(() => Promise.reject(new Error('storage')));
    const { client, socket, connectPromise } = await clientWithToken(
      fake,
      FAKE_TOKEN,
      onTokenRevoked
    );
    await runHandshake(socket);
    await connectPromise;

    const socketsBefore = fake.sockets.length;
    sendTokenRevoked(socket);
    socket.simulateClose();

    await vi.waitFor(() => {
      expect(client.status().phase).toBe('bad_token');
    });
    // No re-probe: the dead secret is never put back on the wire.
    expect(fake.sockets.length).toBe(socketsBefore);
    client.dispose();
  });

  it('ignores a duplicate `token.revoked` (un-pairs once, not per frame)', async () => {
    const { client, socket, onTokenRevoked } = await authenticatedClient();

    sendTokenRevoked(socket);
    sendTokenRevoked(socket);

    expect(onTokenRevoked).toHaveBeenCalledTimes(1);
    client.dispose();
  });
});
