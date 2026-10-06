import { describe, expect, it, vi } from 'vitest';

import { BridgeClient } from '../bridge';
import { computeProof } from '../handshake';
import {
  awaitHello,
  clientWithToken,
  FAKE_TOKEN,
  flushMicrotasks,
  handshakeUpToAuth,
  outcomeOf,
  reply,
  runHandshake,
  sendAuthOk,
  sendChallenge,
  SERVER_NONCE,
  setupFakeWebSocket,
  T,
} from './test-support';

vi.mock('@wxt-dev/browser', () => import('./browser-mock'));

const fake = setupFakeWebSocket();
const NOT_REACHABLE = 'Desktop app not reachable. Is AI Job Hunter running?';

/** Advance past backoff[0]=500ms and assert the client opened a NEW socket (a reconnect IS allowed). */
async function expectReconnects(): Promise<void> {
  const before = fake.sockets.length;
  await vi.advanceTimersByTimeAsync(600);
  expect(fake.sockets.length).toBeGreaterThan(before);
}

describe('BridgeClient – v2 mutual handshake', () => {
  it('completes the full handshake (hello→challenge→auth→auth.ok) and reaches connected', async () => {
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    const { clientNonce, authReqId, proof } = await handshakeUpToAuth(socket);
    // The client proof must be the real HMAC(token, CLIENT_MSG) for the issued
    // nonces — proving the token is used as a key, never transmitted.
    expect(proof).toBe(await computeProof(FAKE_TOKEN, 'client', SERVER_NONCE, clientNonce));

    await sendAuthOk(socket, authReqId, clientNonce);
    await connectPromise;
    expect(client.status().phase).toBe('connected');
    client.dispose();
  });

  it('enters bad_token on an INVALID serverProof and sends NO import/profile frame (no PII)', async () => {
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    // A rogue/port-squatting peer cannot produce a valid serverProof.
    await runHandshake(socket, 'invalid');
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
    // then WITHHOLDS `auth.ok`. If a concurrent `importJob` (the user clicking
    // Import mid-handshake) were gated on transport liveness (set by `attach()`
    // BEFORE the peer is verified) it would ship the active-tab DOM to this
    // unverified peer. It must instead await the SAME handshake and see it fail.
    vi.useFakeTimers();
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    await handshakeUpToAuth(socket); // 2 frames so far: hello + auth

    const outcomePromise = outcomeOf(
      client.importJob({ url: 'https://example.com/job/123', applied: false })
    );
    await flushMicrotasks();
    expect(socket.send.mock.calls.length).toBe(2);

    // The port-squatter never replies — advance past the handshake timeout.
    await vi.advanceTimersByTimeAsync(8_100);
    await connectPromise;

    const outcome = await outcomePromise;
    expect(outcome.ok).toBe(false);
    if (!outcome.ok) {
      expect(outcome.error).toBeInstanceOf(Error);
      expect((outcome.error as Error).message).toBe(NOT_REACHABLE);
    }
    // CRITICAL: the import.request frame was NEVER sent — only hello + auth.
    expect(socket.send.mock.calls.length).toBe(2);
    expect(client.status().phase).not.toBe('connected');
    client.dispose();
  });

  it('a concurrent getProfile during a PENDING handshake sends NO profile.get, and rejects (sending nothing) if the peer closes without auth.ok', async () => {
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    await handshakeUpToAuth(socket);

    // The Contact Profile is the highest-sensitivity PII this client sends.
    const outcomePromise = outcomeOf(client.getProfile());
    await flushMicrotasks();
    expect(socket.send.mock.calls.length).toBe(2);

    // The peer closes without ever sending auth.ok (ambiguous silence).
    socket.simulateClose();
    await connectPromise;

    expect((await outcomePromise).ok).toBe(false);
    // CRITICAL: profile.get was NEVER sent — only hello + auth.
    expect(socket.send.mock.calls.length).toBe(2);
    expect(client.status().phase).toBe('app_not_running');
    client.dispose();
  });

  it('enters app_not_running (NOT bad_token) when the desktop closes without an auth.ok, and reconnect is allowed', async () => {
    // The Rust `Unauthorized` path closes WITHOUT a reply BY DESIGN (a wrong
    // proof and an app crash look identical on the wire) — this ambiguous
    // silence must never assert a hard wrong-token verdict; it must stay
    // recoverable so a genuine crash/restart doesn't strand the user.
    vi.useFakeTimers();
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    await handshakeUpToAuth(socket);
    socket.simulateClose(); // spoke v2 (sent a challenge) but never replies auth.ok
    await connectPromise;

    expect(client.status().phase).toBe('app_not_running');
    expect(client.status().phase).not.toBe('bad_token');
    await expectReconnects();
    client.dispose();
  });

  it('resolves app_not_running (NOT bad_token) when the socket closes while computing the client proof', async () => {
    // Race: the socket closes between receiving the challenge and the driver
    // noticing the transport is gone (after `await computeProof(...)`) — the
    // "closed while hashing" branch. Same ambiguity: never assert wrong-token
    // from silence alone.
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    const { helloReqId } = await awaitHello(socket);
    sendChallenge(socket, helloReqId);
    // Close in the SAME synchronous tick as the challenge — step 1's `settle()`
    // clears the close hook synchronously, so this close is only noticed by the
    // post-hash transport check, not the step-1 close path.
    socket.simulateClose();
    await connectPromise;

    expect(client.status().phase).toBe('app_not_running');
    expect(client.status().phase).not.toBe('bad_token');
    client.dispose();
  });

  it('does NOT authenticate when the socket closes in the same tick as the auth.ok', async () => {
    // Step 5 is synchronous after auth.ok, but the continuation still runs a
    // microtask later — a close landing in between must not resurrect the
    // session (the verdict belongs to the transport that earned it).
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    const { clientNonce, authReqId } = await handshakeUpToAuth(socket);
    const serverProof = await computeProof(FAKE_TOKEN, 'server', SERVER_NONCE, clientNonce);
    reply(socket, T.authOk, authReqId, { serverProof });
    socket.simulateClose();
    await connectPromise;

    expect(client.status()).toMatchObject({ phase: 'app_not_running', authenticated: false });
    client.dispose();
  });

  it('enters outdated when the desktop closes without a challenge (old desktop)', async () => {
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    await awaitHello(socket);
    // An old desktop refuses the v2 hello and closes — no challenge ever arrives.
    socket.simulateClose();
    await connectPromise;
    expect(client.status().phase).toBe('outdated');
    client.dispose();
  });

  it('enters outdated when the desktop replies a non-challenge frame (legacy import.result)', async () => {
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    const { helloReqId } = await awaitHello(socket);
    // An old v1 desktop replies import.result{unauthorized} to our hello (it read
    // an empty token) instead of a challenge → we know it does not speak v2.
    reply(socket, T.importResult, helloReqId, { error: 'unauthorized' });
    await connectPromise;
    expect(client.status().phase).toBe('outdated');
    client.dispose();
  });

  it('enters outdated when the challenge carries a malformed serverNonce (defense-in-depth)', async () => {
    // Mirrors the Rust `is_valid_nonce` shape check: a serverNonce that is not
    // exactly 32 lowercase-hex chars must never feed the HMAC — reject it as a
    // clean handshake failure before any proof is computed.
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    const { helloReqId } = await awaitHello(socket);
    reply(socket, T.challenge, helloReqId, { serverNonce: 'not-hex!!' });
    await connectPromise;
    expect(client.status().phase).toBe('outdated');
    // The malformed nonce must never reach computeProof — only the hello frame
    // was ever sent (no auth frame follows a rejected challenge).
    expect(socket.send.mock.calls.length).toBe(1);
    client.dispose();
  });

  it('does NOT enter bad_token/outdated on a pure handshake timeout — reconnect allowed', async () => {
    vi.useFakeTimers();
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
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
    await expectReconnects();
    client.dispose();
  });

  // #1216: the desktop accepts the socket BEFORE it judges our HMAC proof, so a
  // stale token produces connect-then-silent-close forever. Resetting the ladder
  // on attach pinned that loop to BACKOFF_MS[0] (one probe every 500ms).
  it('escalates the backoff across consecutive connect-then-fail handshakes', async () => {
    vi.useFakeTimers();
    const client = new BridgeClient(vi.fn(), () => Promise.resolve(FAKE_TOKEN));
    void client.ensureConnected();

    /** Open the newest socket, run the handshake to our `auth` proof, then close silently (the Rust `Unauthorized` path). */
    const connectThenFail = async (): Promise<void> => {
      const socket = await fake.next();
      socket.simulateOpen();
      await handshakeUpToAuth(socket);
      socket.simulateClose();
      await vi.advanceTimersByTimeAsync(0);
    };

    // Cycle 1 → reconnect armed at BACKOFF_MS[0] = 500ms.
    await connectThenFail();
    expect(client.status().phase).toBe('app_not_running');
    await expectReconnects();

    // Cycle 2 must wait at BACKOFF_MS[1] = 1000ms, NOT the floor again.
    await connectThenFail();
    const before = fake.sockets.length;
    await vi.advanceTimersByTimeAsync(600);
    expect(fake.sockets.length).toBe(before); // still waiting — the ladder escalated
    await vi.advanceTimersByTimeAsync(500);
    expect(fake.sockets.length).toBeGreaterThan(before);
    client.dispose();
  });

  it('resetForNewToken() clears bad_token so ensureConnected() attempts a fresh socket', async () => {
    vi.useFakeTimers();
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    // A genuine, non-ambiguous rejection: the desktop DID reply auth.ok, but the
    // serverProof does not verify → bad_token (a real mismatch, not silence).
    await runHandshake(socket, 'invalid');
    await connectPromise;
    expect(client.status().phase).toBe('bad_token');

    client.resetForNewToken();
    expect(client.status().phase).toBe('searching');

    const before = fake.sockets.length;
    void client.ensureConnected();
    await vi.advanceTimersByTimeAsync(0);
    expect(fake.sockets.length).toBeGreaterThan(before);
    client.dispose();
  });

  // ── #1267 — an already-open, unauthenticated transport (the no-token attach
  // path) must be replaced when a token is finally saved, so the next connect
  // actually runs the v2 handshake instead of sitting "connected" forever ────

  /** Attach with NO token (reaches 'connected' with zero handshake), then save a token the way background.ts's `setToken` does. */
  async function pasteTokenOnUnpairedTransport() {
    let stored: string | null = null;
    const client = new BridgeClient(vi.fn(), () => Promise.resolve(stored));
    const firstConnect = client.ensureConnected();
    const first = await fake.next();
    first.simulateOpen();
    await firstConnect;
    expect(client.status().phase).toBe('connected');
    expect(first.send).not.toHaveBeenCalled();

    stored = FAKE_TOKEN;
    client.resetForNewToken();
    void client.ensureConnected();
    // A NEW transport must be opened — on unmodified code `ensureConnected()`
    // no-ops on the already-open transport and this never happens.
    const second = await fake.next();
    second.simulateOpen();
    return { client, first, second };
  }

  it('re-handshakes on an already-open UNAUTHENTICATED transport once a token is saved (#1267)', async () => {
    const { client, second } = await pasteTokenOnUnpairedTransport();
    // ...and the full v2 handshake runs on it, with the NEW token.
    const { clientNonce, authReqId, proof } = await handshakeUpToAuth(second);
    expect(proof).toBe(await computeProof(FAKE_TOKEN, 'client', SERVER_NONCE, clientNonce));
    await sendAuthOk(second, authReqId, clientNonce);

    await vi.waitFor(() => {
      expect(client.status().phase).toBe('connected');
    });
    client.dispose();
  });

  it('a late close on the REPLACED transport does not clobber the new one, set app_not_running, or arm a reconnect', async () => {
    vi.useFakeTimers();
    const { client, first, second } = await pasteTokenOnUnpairedTransport();
    await runHandshake(second);
    await vi.waitFor(() => {
      expect(client.status().phase).toBe('connected');
    });
    const socketsBeforeLateClose = fake.sockets.length;

    // A LATE close event on the already-replaced, stale first socket (mirrors
    // the real-world race where the actual close arrives well after
    // `resetForNewToken()` already moved on) must be a total no-op.
    first.simulateClose();
    expect(client.status().phase).toBe('connected');
    expect(client.isOpen()).toBe(true);

    // No reconnect got armed for the stale close — advance well past every
    // backoff rung; no new socket should ever appear.
    await vi.advanceTimersByTimeAsync(10_000);
    expect(fake.sockets.length).toBe(socketsBeforeLateClose);
    client.dispose();
  });

  it('does NOT close an ALREADY-authenticated transport on the new-token path (re-pasting a token while connected keeps the session)', async () => {
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    await runHandshake(socket);
    await connectPromise;
    expect(client.status().phase).toBe('connected');

    client.resetForNewToken();

    expect(socket.close).not.toHaveBeenCalled();
    expect(client.isOpen()).toBe(true);
    expect(client.status().phase).toBe('connected');
    client.dispose();
  });

  it('does NOT send any frame when no token is stored, and stays not-paired', async () => {
    const { client, socket, connectPromise } = await clientWithToken(fake, null);
    await connectPromise;
    // No hello (nor any frame) is sent when unpaired.
    expect(socket.send).not.toHaveBeenCalled();
    // Phase is 'connected' from the bridge perspective (background → not_paired).
    expect(client.status().phase).toBe('connected');
    client.dispose();
  });

  it('ignores an UNKNOWN message type (how already-published extensions survive new verbs)', async () => {
    // The compatibility contract that lets the desktop send new verbs without a
    // protocol bump: an extension that predates a verb must ignore it silently.
    const { client, socket, connectPromise } = await clientWithToken(fake, FAKE_TOKEN);
    await runHandshake(socket);
    await connectPromise;

    const framesBefore = socket.send.mock.calls.length;
    socket.simulateMessage(
      JSON.stringify({ type: 'some.future.verb', reqId: 'x', payload: { anything: true } })
    );

    expect(client.status().phase).toBe('connected');
    expect(socket.send.mock.calls.length).toBe(framesBefore);
    client.dispose();
  });
});
