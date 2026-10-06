/**
 * The v2 mutual HMAC handshake driver (`hello` → `challenge` → `auth{proof}` →
 * `auth.ok{serverProof}`). The pairing token is used ONLY as an HMAC key — it
 * never goes on the wire. The extension MUST verify the desktop's `serverProof`
 * before the session counts as authenticated; a peer that can't prove it knows
 * the token (rogue / port-squatter) never receives any PII.
 */

import {
  EXTENSION_MESSAGE_TYPES,
  EXTENSION_PROTOCOL_VERSION,
  type ExtensionEnvelope,
} from '@ajh/shared/extension-protocol';

import { computeProof, constantTimeHexEqual, isValidNonceHex, randomNonceHex } from '../handshake';
import { HANDSHAKE_TIMEOUT_MS } from './constants';
import { readHexField } from './guards';
import type { BridgeTransport } from './transport';

export type HandshakeOutcome = 'app_not_running' | 'outdated' | 'bad_token';

/** One step of the handshake: a frame arrived, the socket closed, or timeout. */
type HandshakeStep =
  { kind: 'frame'; env: Partial<ExtensionEnvelope> } | { kind: 'closed' } | { kind: 'timeout' };

/** What the handshake needs from the connection that owns the transport. */
export interface HandshakeHost {
  /** The connection's CURRENT transport (null once it closed or was replaced). */
  transport(): BridgeTransport | null;
  /** Terminal failure: set the phase, drop the transport, maybe schedule a reconnect. */
  finish(outcome: HandshakeOutcome): void;
  /** Mutual auth completed on the current transport. */
  authenticated(): void;
}

export class Handshake {
  /**
   * Set while a step is in flight: `frame` receives every incoming frame (so
   * `run` can advance step by step), `closed` is invoked on socket close so the
   * step resolves as `closed`. Both are cleared when a step settles. NOT an
   * auth check — see `BridgeConnection.authenticated`.
   */
  frame: ((env: Partial<ExtensionEnvelope>) => void) | null = null;
  closed: (() => void) | null = null;

  constructor(private readonly host: HandshakeHost) {}

  clear(): void {
    this.frame = null;
    this.closed = null;
  }

  /**
   * Await the next handshake frame, a socket close, or a per-step timeout. While
   * pending, the connection routes EVERY incoming frame here (no import/profile
   * frames are expected before the socket is authenticated).
   */
  private awaitStep(): Promise<HandshakeStep> {
    return new Promise<HandshakeStep>((resolve) => {
      let done = false;
      const settle = (step: HandshakeStep): void => {
        if (done) return;
        done = true;
        clearTimeout(timer);
        this.clear();
        resolve(step);
      };
      const timer = setTimeout(() => settle({ kind: 'timeout' }), HANDSHAKE_TIMEOUT_MS);
      this.frame = (env) => settle({ kind: 'frame', env });
      this.closed = () => settle({ kind: 'closed' });
    });
  }

  async run(token: string): Promise<void> {
    const transport = this.host.transport();
    if (!transport) return;
    const finish = (outcome: HandshakeOutcome): void => this.host.finish(outcome);

    const clientNonce = randomNonceHex();

    // Step 1: hello (no token) → await the desktop's challenge.
    transport.send({
      type: EXTENSION_MESSAGE_TYPES.hello,
      reqId: crypto.randomUUID(),
      payload: { protocol: EXTENSION_PROTOCOL_VERSION, clientNonce },
    });
    const step1 = await this.awaitStep();
    if (step1.kind === 'timeout') {
      // Total silence — treat as a transient transport failure (recoverable).
      return finish('app_not_running');
    }
    if (step1.kind === 'closed' || step1.env.type !== EXTENSION_MESSAGE_TYPES.challenge) {
      // The desktop closed without a challenge, or replied a non-challenge frame
      // (e.g. an old desktop's import.result / update.required). Either way it
      // does not speak v2 → the user must update the desktop app.
      return finish('outdated');
    }
    const serverNonce = readHexField(step1.env.payload, 'serverNonce');
    // Defense-in-depth (mirrors the Rust `is_valid_nonce` shape check on the
    // client nonce): reject a malformed/oversized serverNonce as a clean
    // handshake failure BEFORE it feeds the HMAC — never silently proceed with
    // attacker-shaped input. Grouped with "missing" as `outdated`: a peer whose
    // very first reply doesn't carry a well-formed nonce does not properly speak
    // v2.
    if (!serverNonce || !isValidNonceHex(serverNonce)) {
      return finish('outdated');
    }

    // Step 3: prove we know the token (HMAC over the client role) → await auth.ok.
    // BOTH proofs are computed here, before the `auth` frame goes out, even
    // though the server one is not needed until step 5. That is deliberate: it
    // leaves ZERO `await` between receiving `auth.ok` and setting
    // `authenticated` below. With the server proof computed lazily at step 5
    // instead, that ~sub-ms crypto await sat exactly where a legitimate
    // `token.revoked` can arrive — the desktop marks the socket authenticated
    // when it sends `auth.ok`, so it may rotate and revoke immediately after —
    // and the revoke would be DROPPED for being "unauthenticated", stranding
    // the extension on a dead token. Frames are delivered as separate tasks, so
    // a fully synchronous step 5 is guaranteed to run before the next one.
    const [clientProof, expectedServerProof] = await Promise.all([
      computeProof(token, 'client', serverNonce, clientNonce),
      computeProof(token, 'server', serverNonce, clientNonce),
    ]);
    const live = this.host.transport();
    // Same stale-transport rule as the final authenticated-set below: the
    // challenge was issued on `transport`, so the proof is only valid there. A
    // newer attach may have replaced it during the await — never send this
    // proof (or this token's handshake) onto a different socket.
    if (live && live !== transport) return;
    if (!live) {
      // Socket closed while we were computing the proof. AMBIGUOUS: the Rust
      // `Unauthorized` path closes WITHOUT a reply BY DESIGN (see
      // extension_bridge/mod.rs), so this is indistinguishable from a genuine
      // app crash/restart. Never assert a hard wrong-token verdict from silence
      // alone — recoverable, consistent with the step-1 timeout above.
      return finish('app_not_running');
    }
    transport.send({
      type: EXTENSION_MESSAGE_TYPES.auth,
      reqId: crypto.randomUUID(),
      payload: { proof: clientProof },
    });
    const step2 = await this.awaitStep();
    if (step2.kind === 'closed' || step2.kind === 'timeout') {
      // No auth.ok arrived — silence (closed or timed out) after we already sent
      // our proof. Same ambiguity as above: the Rust `Unauthorized` path closes
      // without a reply, indistinguishable from a crash. Recoverable.
      return finish('app_not_running');
    }
    if (step2.env.type !== EXTENSION_MESSAGE_TYPES.authOk) {
      // The peer DID reply — with something other than auth.ok. Unlike silence,
      // this is an actual, non-ambiguous response from a peer that spoke v2 far
      // enough to send a challenge; treat it as untrusted → bad_token (re-pair).
      return finish('bad_token');
    }

    // Step 5: verify the desktop's serverProof CONSTANT-TIME before trusting it.
    // Fully SYNCHRONOUS from here to the `authenticated` set — see the
    // both-proofs-up-front note at step 3. Do not reintroduce an `await` in
    // this stretch: it would reopen the window where a legitimate
    // `token.revoked` is dropped as unauthenticated.
    const serverProof = readHexField(step2.env.payload, 'serverProof');
    if (!serverProof || !constantTimeHexEqual(serverProof, expectedServerProof)) {
      // The peer cannot prove it knows the token (rogue/port-squatter). We have
      // sent NO PII (import/profile only happen after 'connected'); drop the
      // socket and surface bad_token.
      return finish('bad_token');
    }

    // The verdict is only valid for the transport that EARNED it. The awaits
    // EARLIER in this handshake yield, so the socket can close (or be replaced
    // by a newer attach) at any of them — and `onClose` clears `authenticated`.
    // Setting it here regardless would resurrect the flag on a transport that no
    // longer exists (stale-true until the next attach), and `setPhase('connected')`
    // would claim a live session over a null transport.
    //
    // Plain `return`, deliberately NOT `finish`: whatever ended this transport
    // already set its own phase and armed its own reconnect, and `finish` would
    // null + close `this.transport` — which, in the replaced case, is a
    // DIFFERENT and perfectly healthy socket.
    if (this.host.transport() !== transport) return;

    // Mutual auth complete — the socket is authenticated. This is the ONE place
    // that may set `authenticated`: everything before it (including a peer that
    // sent a well-formed `challenge`) has proven nothing about the token.
    this.clear();
    this.host.authenticated();
  }
}
