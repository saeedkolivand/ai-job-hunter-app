/**
 * Connection lifecycle of the desktop bridge client: transport selection
 * (native first, ws fallback), the v2 handshake, reconnect/backoff, and the
 * `token.revoked` rule. The verbs built on top live in `client.ts`.
 */

import { EXTENSION_MESSAGE_TYPES, type ExtensionEnvelope } from '@ajh/shared/extension-protocol';

import { Handshake, type HandshakeOutcome } from './auth-handshake';
import { BACKOFF_MS } from './constants';
import { RequestTable } from './requests';
import { isAssistChunkPayload } from './results-answers';
import {
  type BridgeTransport,
  connectNative,
  NATIVE_APP_DOWN,
  type NativeMessagingTransport,
  probePorts,
  WebSocketTransport,
} from './transport';

export type BridgePhase = 'searching' | 'connected' | 'app_not_running' | 'outdated' | 'bad_token';

export interface BridgeStatus {
  phase: BridgePhase;
  port: number | null;
  /**
   * Whether the v2 mutual handshake actually completed on the CURRENT
   * transport — distinct from `phase === 'connected'`, which is ALSO reached
   * with zero handshake when no token is stored (see `attach`'s no-token
   * branch). `background/bridge-client.ts`'s `computeStatus()` gates the "Connected" popup
   * state on this, not on `phase` alone (#1267).
   */
  authenticated: boolean;
}

export class BridgeConnection {
  protected transport: BridgeTransport | null = null;
  protected phase: BridgePhase = 'searching';
  protected readonly requests = new RequestTable();
  private port: number | null = null;
  private backoffIndex = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private disposed = false;
  /**
   * The in-flight `doConnect()` promise (transport probe THROUGH the full v2
   * handshake), or `null` when no connection attempt is running. A concurrent
   * `ensureConnected()` call AWAITS this SAME promise instead of returning early
   * — this is the fix for the "app frame reaches an unverified peer" race: the
   * transport can be non-null while the handshake is still verifying the
   * peer's `serverProof`, so `isOpen()` alone is never a safe "ready" signal for
   * a concurrent caller. The verbs additionally gate on `this.phase ===
   * 'connected'` (the authenticated session), never on transport liveness.
   */
  private connectPromise: Promise<void> | null = null;
  /** When true the last close was an auth rejection — suppress normal reconnect. */
  private authRejected = false;
  /**
   * When true the desktop is too old for the v2 handshake (it never sent a
   * `challenge`) — suppress the reconnect loop so we don't hammer an old app; the
   * next popup open / Retry re-probes and recovers once the desktop is updated.
   */
  private outdated = false;
  /**
   * Whether THIS transport completed the v2 mutual handshake — i.e. the peer
   * proved it knows the pairing token (its `serverProof` verified). The ONLY
   * safe gate for a peer-initiated, state-destroying frame like
   * `token.revoked`.
   *
   * Deliberately NOT any of the cheaper-looking signals:
   * - `handshake.frame` is a step-in-flight LATCH, not an auth check — it is
   *   nulled the instant a step's frame is consumed, so every frame arriving
   *   between steps (notably while `computeProof` is awaited) sails past it. A
   *   port-squatter needs zero token knowledge to send a syntactically-valid
   *   `challenge` and then walk through that window.
   * - `phase === 'connected'` is reached WITHOUT any handshake when no token is
   *   stored (see `attach`), and is also live during the whole pre-`hello`
   *   window (`attach` wires `onMessage` before it reads the stored token).
   *
   * Set only at the mutual-auth completion point; cleared on every fresh
   * `attach` and on close, so it can never outlive the transport that earned it.
   */
  private authenticated = false;
  /**
   * Set between a `token.revoked` frame and the socket close that follows it.
   * The desktop rotated its pairing secret (Settings → "Regenerate", or a
   * factory reset), so the stored token is dead — retrying it would loop on the
   * handshake's deliberately silent close (which can only be read as the
   * recoverable `app_not_running`), leaving the popup stuck on "app not running"
   * forever. Instead the close re-probes ONCE, unpaired: an attach with no token
   * reports `connected`, which the background's `computeStatus` folds into
   * `not_paired` — the pairing view.
   */
  private revoked = false;
  /**
   * Resolves once `onTokenRevoked` has settled, reporting whether the stored
   * token was actually dropped. The close handler awaits it before re-probing so
   * the reconnect can never race the now-dead token back onto the wire — and so
   * a FAILED clear (storage error) is handled explicitly rather than silently
   * degrading into a retry loop on the dead secret.
   */
  private revokeCleared: Promise<boolean> | null = null;
  private readonly handshake = new Handshake({
    transport: () => this.transport,
    finish: (outcome) => this.finishHandshake(outcome),
    authenticated: () => {
      this.authenticated = true;
      this.backoffIndex = 0;
      this.setPhase('connected');
    },
  });

  /** Notified on every phase change so the background can broadcast status. */
  constructor(
    private readonly onPhaseChange: (status: BridgeStatus) => void,
    /** Optional: called on connect to retrieve the stored pairing token for the auth handshake. */
    private readonly getStoredToken?: () => Promise<string | null>,
    /**
     * Optional: called when the desktop sends `token.revoked` — it rotated the
     * pairing secret, so the owner must DROP the stored token (the background
     * un-pairs locally). Awaited before the post-revoke reconnect so the dead
     * token is never handshaked again.
     */
    private readonly onTokenRevoked?: () => Promise<void> | void
  ) {}

  status(): BridgeStatus {
    return { phase: this.phase, port: this.port, authenticated: this.authenticated };
  }

  /** Whether a transport is currently live. */
  isOpen(): boolean {
    return this.transport !== null;
  }

  /**
   * Ensure a connection: no-op if already open; otherwise try native then ws
   * — INCLUDING the full v2 handshake. Safe to call repeatedly (popup-open
   * wake, reconnect button, a concurrent verb).
   *
   * "Already open" here is always either authenticated or genuinely unpaired
   * (no token ever stored) — an open-but-never-authenticated transport (the
   * no-token attach branch) is force-replaced by `resetForNewToken()` the
   * moment a token is actually stored (#1267).
   *
   * A concurrent call while a connection attempt is already running AWAITS the
   * SAME promise rather than short-circuiting — critical because `attach()` sets
   * `this.transport` before the handshake has verified the peer's `serverProof`.
   * Without this, a second caller would see `isOpen()` and treat an unverified
   * transport as ready, sending an app frame (e.g. the active-tab DOM) to a peer
   * that has not yet proven it knows the pairing token.
   */
  async ensureConnected(): Promise<void> {
    if (this.disposed || this.phase === 'bad_token') return;
    if (this.connectPromise) return this.connectPromise;
    if (this.isOpen()) return;
    this.connectPromise = this.doConnect();
    try {
      await this.connectPromise;
    } finally {
      this.connectPromise = null;
    }
  }

  /** The actual connect-then-handshake attempt tracked by {@link connectPromise}. */
  private async doConnect(): Promise<void> {
    this.setPhase('searching');
    // Native first. `connectNative()` THROWS SYNCHRONOUSLY when the host isn't
    // registered, so building the readiness promise is separated from awaiting
    // it — that keeps the ws fallback probe firing in the SAME tick (no
    // microtask hop) when native is unavailable, which the ws reconnect test
    // relies on.
    let readyPromise: Promise<NativeMessagingTransport> | null;
    try {
      readyPromise = connectNative();
    } catch {
      readyPromise = null; // host not registered → straight to ws, same tick.
    }
    if (readyPromise) {
      try {
        const native = await readyPromise;
        this.port = null; // native has no port number; diagnostics only.
        await this.attach(native);
        return;
      } catch (err) {
        if (err instanceof Error && err.message === NATIVE_APP_DOWN) {
          // Host reachable, app down — do NOT fall back to ws.
          this.setPhase('app_not_running');
          this.scheduleReconnect();
          return;
        }
        // NATIVE_UNAVAILABLE → fall through to the ws probe.
      }
    }

    this.setPhase('searching');
    const found = await probePorts();
    this.port = found?.port ?? null;
    if (found) {
      await this.attach(new WebSocketTransport(found.socket));
    } else {
      this.setPhase('app_not_running');
      this.scheduleReconnect();
    }
  }

  /**
   * Clear the bad-token block so `ensureConnected()` will attempt a fresh
   * connection after the user pastes a new token. Call this whenever the stored
   * token is updated or cleared.
   */
  resetForNewToken(): void {
    this.authRejected = false;
    this.outdated = false;
    if (this.phase === 'bad_token' || this.phase === 'outdated') {
      this.setPhase('searching');
    }
    // `phase === 'connected'` is reached with ZERO handshake by `attach()`'s
    // no-token branch — an open transport that has proven nothing. Pasting a
    // token onto that transport must not sit there forever unauthenticated
    // (#1267): force a fresh connect by dropping it, so the next
    // `ensureConnected()` opens a NEW transport and runs the full v2 handshake
    // with the new token. Gated on `phase === 'connected'` (not `isOpen()`
    // generally) so this never interrupts an in-flight handshake. An
    // ALREADY-authenticated transport is left alone: re-pasting the same token
    // on a healthy session must not drop it.
    if (this.transport && this.phase === 'connected' && !this.authenticated) {
      const stale = this.transport;
      this.transport = null;
      this.setPhase('searching');
      // `stale`'s `onClose` (wired in `attach()`) is a no-op for this close:
      // `this.transport` is already cleared above, so its identity check
      // (`this.transport !== transport`) short-circuits before it can surface
      // `app_not_running`, arm the backoff reconnect, or null out whatever
      // `ensureConnected()` attaches next.
      stale.close();
    }
  }

  /** Tear down the transport and cancel timers (worker shutdown / manual reset). */
  dispose(): void {
    this.disposed = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    // Settle any in-flight handshake step so its timeout timer is cleared.
    this.handshake.closed?.();
    this.handshake.clear();
    // Settle every in-flight request BEFORE closing the transport — `failAll`
    // already resolves + clears every pending entry/timer/listener, so closing
    // first would let the transport's own `onClose` handler find nothing left
    // to settle: each promise would hang forever instead of resolving.
    this.requests.failAll('Bridge client disposed.');
    this.transport?.close();
    this.transport = null;
  }

  protected setPhase(phase: BridgePhase): void {
    if (this.phase === phase) return;
    this.phase = phase;
    this.onPhaseChange(this.status());
  }

  /** Wire an opened transport, then run the v2 handshake if a token is stored. */
  private async attach(transport: BridgeTransport): Promise<void> {
    this.transport = transport;
    // `backoffIndex` is NOT reset here: an open transport proves nothing. The
    // desktop bridge accepts the socket before it judges our HMAC proof, so
    // resetting on attach pins a connect-then-fail loop to the ladder's floor
    // forever. Only a genuinely usable connection (below, and on handshake
    // success) clears the ladder.
    this.authRejected = false;
    this.outdated = false;
    // A fresh transport has proven nothing yet — including the no-token attach
    // below, which reaches `connected` without ever running a handshake.
    this.authenticated = false;
    // Same discipline as the sibling latches above: a revoke belongs to the
    // transport that received it. Leaving these set would let a PREVIOUS
    // socket's revoke divert this connection's close into the re-probe branch
    // (and leak a settled promise).
    this.revoked = false;
    this.revokeCleared = null;

    transport.onMessage((env) => this.onMessage(env));
    transport.onClose(() => {
      // Stale transport: something (a later `attach()`, or `resetForNewToken()`
      // replacing an unauthenticated one — #1267) already moved `this.transport`
      // on. This late close must not null out / clobber whatever is now live.
      if (this.transport !== transport) return;
      this.transport = null;
      // The session dies with the socket. A late/queued frame can still be
      // delivered on the dead transport's listener BEFORE any re-attach runs,
      // and without this it would still be treated as authenticated.
      this.authenticated = false;
      this.requests.failAll('Connection to the desktop app closed.');
      // If a handshake is in flight, let it settle (it decides outdated /
      // bad_token / reconnect) — don't set a phase or reconnect from here.
      if (this.handshake.closed) {
        const notify = this.handshake.closed;
        this.handshake.clear();
        notify();
        return;
      }
      if (this.disposed) return;
      if (this.authRejected) {
        // Desktop rejected our proof (wrong token) — do NOT reconnect in a loop;
        // the user must re-pair with a new token.
        this.setPhase('bad_token');
      } else if (this.outdated) {
        // Desktop too old to speak v2 — recover on the next popup open / Retry.
        this.setPhase('outdated');
      } else if (this.revoked) {
        // The desktop revoked this pairing (it rotated the token) and closed
        // the socket. No backoff loop with the dead secret: wait for it to be
        // cleared, then re-probe ONCE. That attach finds no token, reports
        // `connected`, and `computeStatus` folds "connected + no token" into
        // `not_paired` — the popup's pairing view.
        this.revoked = false;
        this.backoffIndex = 0;
        this.setPhase('searching');
        const cleared = this.revokeCleared ?? Promise.resolve(true);
        this.revokeCleared = null;
        void cleared.then((tokenCleared) => {
          if (this.disposed) return;
          if (!tokenCleared) {
            // Storage refused to drop the token. Never reconnect with a secret
            // we KNOW is revoked — surface `bad_token`, whose popup view is the
            // re-pair prompt, instead of a silent retry loop.
            this.authRejected = true;
            this.setPhase('bad_token');
            return;
          }
          void this.ensureConnected();
        });
      } else {
        this.setPhase('app_not_running');
        this.scheduleReconnect();
      }
    });

    // v2 mutual handshake: run it only if a token is stored. Without a token the
    // socket is open but unpaired — computeStatus() surfaces that as 'not_paired'.
    const token = this.getStoredToken ? await this.getStoredToken() : null;
    if (!token) {
      this.backoffIndex = 0;
      this.setPhase('connected');
      return;
    }
    await this.handshake.run(token);
  }

  /**
   * Terminal handshake outcome: clear the handshake hooks, set the phase, and
   * drop the transport. `bad_token` / `outdated` suppress the reconnect loop;
   * `app_not_running` schedules a reconnect (a transient transport blip).
   */
  private finishHandshake(phase: HandshakeOutcome): void {
    this.handshake.clear();
    if (phase === 'bad_token') this.authRejected = true;
    if (phase === 'outdated') this.outdated = true;
    const transport = this.transport;
    this.transport = null;
    transport?.close();
    if (this.disposed) return;
    this.setPhase(phase);
    if (phase === 'app_not_running') this.scheduleReconnect();
  }

  /** Route a parsed frame: handshake step, `token.revoked`, stream chunk, or a reply by `reqId`. */
  private onMessage(parsed: unknown): void {
    if (typeof parsed !== 'object' || parsed === null) return;
    const env = parsed as Partial<ExtensionEnvelope>;

    // While the v2 handshake is in flight, EVERY frame goes to the handshake
    // driver (challenge / auth.ok / an unexpected non-v2 reply). No import or
    // profile frames are expected before the socket is authenticated.
    if (this.handshake.frame) {
      this.handshake.frame(env);
      return;
    }

    // token.revoked → the desktop rotated its pairing secret, so ours is dead.
    // Unlike every other frame below this is not a reply: it correlates to no
    // request and carries no payload, and acting on it DESTROYS the stored
    // pairing credential — so it is honored ONLY from a session that completed
    // the mutual handshake (see {@link authenticated}). The desktop already
    // refuses to send it to an unauthenticated peer; this is the client-side
    // half of the same rule, and it does not depend on the desktop being the
    // one on the other end.
    //
    // Sits BELOW the handshake intercept on purpose: the two protections
    // compose. The intercept routes frames away while a step is parked, and
    // the flag covers the windows the intercept cannot see — between steps,
    // before the first `hello`, and the no-token attach that never handshakes.
    if (env.type === EXTENSION_MESSAGE_TYPES.tokenRevoked) {
      if (this.authenticated) this.handleTokenRevoked();
      return;
    }

    const reqId = typeof env.reqId === 'string' ? env.reqId : '';

    // assist.chunk → one incremental delta of a streaming reply. Best-effort:
    // silently dropped when no listener is registered for this `reqId` (the
    // request already settled/timed out) — a chunk is never itself a complete
    // answer, so there's nothing to fall back to.
    if (env.type === EXTENSION_MESSAGE_TYPES.assistChunk) {
      if (isAssistChunkPayload(env.payload)) this.requests.chunk(reqId, env.payload.delta);
      return;
    }

    // assist.done → the stream for `reqId` has ended; the verb's own terminal
    // reply carries the actual outcome. Just retire the chunk listener so a
    // stray late chunk can never fire.
    if (env.type === EXTENSION_MESSAGE_TYPES.assistDone) {
      this.requests.unlisten(reqId);
      return;
    }

    // Anything else is a reply (matched by its verb's reply type) or an
    // unknown type, which already-published extensions must ignore silently.
    if (typeof env.type === 'string') this.requests.reply(env.type, reqId, env.payload);
  }

  /**
   * Handle a `token.revoked` frame: drop the stored pairing token and arm the
   * un-paired re-probe the close that follows will run (see `attach`'s
   * `onClose`). We do NOT close the transport here — the desktop closes it
   * immediately after this frame, and even if it somehow didn't, an open socket
   * with no stored token already reports `not_paired`, which is the correct end
   * state either way.
   *
   * Idempotent: a duplicate frame (or a second rotation racing the close) must
   * not clear the token twice or arm two re-probes.
   */
  private handleTokenRevoked(): void {
    if (this.revoked) return;
    this.revoked = true;
    // `false` = the owner could NOT drop the token (a storage error). The close
    // handler turns that into `bad_token` rather than reconnecting: retrying a
    // secret we know is dead is the exact forever-loop this frame exists to end.
    this.revokeCleared = Promise.resolve(this.onTokenRevoked?.()).then(
      () => true,
      () => false
    );
  }

  private scheduleReconnect(): void {
    if (this.disposed || this.reconnectTimer) return;
    const delay = BACKOFF_MS[Math.min(this.backoffIndex, BACKOFF_MS.length - 1)] ?? 5_000;
    this.backoffIndex += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      void this.ensureConnected();
    }, delay);
  }
}
