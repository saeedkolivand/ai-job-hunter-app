/**
 * Client to the desktop bridge, native-messaging first with a `ws` fallback.
 *
 * Two transports behind one {@link BridgeTransport} seam:
 *
 * 1. **Native messaging** (preferred). The browser spawns the desktop exe as a
 *    native host (`app.aijobhunter.bridge`) which relays stdio ↔ the running
 *    app's loopback bridge. Survives Firefox HTTPS-Only Mode (which upgrades the
 *    extension's `ws://127.0.0.1` to `wss://` and breaks the socket path).
 * 2. **WebSocket** (fallback). The desktop binds `127.0.0.1` on the first free
 *    port in the range below (see
 *    `apps/desktop/src-tauri/src/extension_bridge/mod.rs::PORT_RANGE`). Used when
 *    the native host isn't registered (old/never-installed app).
 *
 * Either way we hold a SINGLE transport. On connect we run the v2 mutual HMAC
 * handshake (`hello` → `challenge` → `auth{proof}` → `auth.ok{serverProof}`) —
 * the pairing token is used only as an HMAC key and is NEVER put on the wire —
 * then send token-free `import.request` / `profile.get` envelopes over the
 * now-authenticated socket, correlating replies by `reqId`.
 *
 * Lifecycle note (MV3): the background service worker can be evicted at any
 * time, tearing down this client. The background entry re-creates it on wake
 * and when the popup opens, so this class assumes it may be short-lived and
 * keeps no cross-eviction state beyond the in-flight `reqId` map (which dies
 * with the worker — callers re-issue on the fresh instance).
 */

import { type Browser, browser } from '@wxt-dev/browser';

import {
  EXTENSION_MESSAGE_TYPES,
  EXTENSION_PROTOCOL_VERSION,
  type ExtensionAnswerPair,
  type ExtensionAnswersSaveResult,
  type ExtensionAnswersSuggestResult,
  type ExtensionAnswerSuggestion,
  type ExtensionAppliedCheckResult,
  type ExtensionEnvelope,
  type ExtensionImportRequest,
  type ExtensionImportResult,
  type ExtensionProfileResult,
  type ExtensionStatusUpdateResult,
} from '@ajh/shared/extension-protocol';

import { computeProof, constantTimeHexEqual, isValidNonceHex, randomNonceHex } from './handshake';

/** Inclusive probe range — mirrors the desktop `PORT_RANGE` (47615..=47620). */
const PORT_START = 47615;
const PORT_END = 47620;

/** Native host name — MUST match the Rust `extension_bridge::mod::NATIVE_HOST_NAME`. */
const HOST_NAME = 'app.aijobhunter.bridge';

/** Per-request timeout (ms) — the desktop fetch+parse for URL mode can be slow. */
const REQUEST_TIMEOUT_MS = 30_000;

/** Backoff schedule (ms) for reconnect attempts; the last value repeats. */
const BACKOFF_MS = [500, 1_000, 2_000, 5_000, 10_000];

/** WS handshake/open timeout per port probe. */
const OPEN_TIMEOUT_MS = 1_500;

/**
 * Per-step timeout for the v2 handshake (await challenge / await auth.ok). On
 * loopback each step is a fast round-trip; a total silence (no frame, no close)
 * is treated as a transient transport failure (reconnect), while an explicit
 * close / a non-`challenge` reply is the outdated-desktop signal.
 */
const HANDSHAKE_TIMEOUT_MS = 8_000;

/** How long to wait for the native host's `bridge.ready` before falling back to ws. */
const READY_TIMEOUT_MS = 1_500;

type PendingResolver = (result: ExtensionImportResult) => void;
type ProfileResolver = (result: ExtensionProfileResult) => void;
type AppliedResolver = (result: ExtensionAppliedCheckResult) => void;
type StatusResolver = (result: ExtensionStatusUpdateResult) => void;
type AnswersResolver = (result: ExtensionAnswersSaveResult) => void;
type SuggestResolver = (result: ExtensionAnswersSuggestResult) => void;

export type BridgePhase = 'searching' | 'connected' | 'app_not_running' | 'outdated' | 'bad_token';

/** One step of the v2 handshake: a frame arrived, the socket closed, or timeout. */
type HandshakeStep =
  { kind: 'frame'; env: Partial<ExtensionEnvelope> } | { kind: 'closed' } | { kind: 'timeout' };

/** Read a non-empty string field (a nonce / proof) off a handshake payload. */
function readHexField(payload: unknown, key: string): string | null {
  if (typeof payload !== 'object' || payload === null) return null;
  const v = (payload as Record<string, unknown>)[key];
  return typeof v === 'string' && v.length > 0 ? v : null;
}

export interface BridgeStatus {
  phase: BridgePhase;
  port: number | null;
}

/** A short id for `reqId` correlation. `crypto.randomUUID` exists in SW + DOM. */
function newReqId(): string {
  return crypto.randomUUID();
}

/**
 * Hand-written guard for an `import.result` payload — replaces the zod schema so
 * the extension bundle stays zod-free (zod v4's JIT probe trips AMO's
 * `DANGEROUS_EVAL` lint). Mirrors `ExtensionImportResultSchema`: every field is
 * optional; the string fields must be strings and `matchScore` a number when
 * present.
 */
function isExtensionImportResult(v: unknown): v is ExtensionImportResult {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  const optionalString = (x: unknown): boolean => x === undefined || typeof x === 'string';
  return (
    optionalString(o.applicationId) &&
    optionalString(o.status) &&
    optionalString(o.title) &&
    optionalString(o.company) &&
    optionalString(o.error) &&
    (o.matchScore === undefined || typeof o.matchScore === 'number') &&
    (o.partial === undefined || typeof o.partial === 'boolean')
  );
}

/** True when `x` is a plain `{label: string, url: string}` link entry — `url`
 *  must additionally be `http(s)://`. This is defense-in-depth against a
 *  buggy/older desktop sending a malformed scheme (e.g. `javascript:`): the
 *  bridge's own auth/allowlist model doesn't cover payload content, and a URL
 *  is the one field here that gets set into a form's `value` and dispatched
 *  as an event, so it self-defends rather than trusting the server. */
function isLinkEntry(x: unknown): x is { label: string; url: string } {
  if (typeof x !== 'object' || x === null) return false;
  const o = x as Record<string, unknown>;
  return typeof o.label === 'string' && typeof o.url === 'string' && /^https?:\/\//i.test(o.url);
}

/**
 * Hand-written guard for a `profile.result` payload (extension stays zod-free).
 * Mirrors `ExtensionProfileResultSchema`: every field is an optional string,
 * except `extraLinks` — an optional array, additive to the payload (an old
 * desktop's reply simply never carries the key). Only the array shape is
 * gated here; individual entries are validated/dropped one-by-one in
 * {@link normalizeProfileResult} via {@link isLinkEntry} so one malformed
 * link (e.g. a `javascript:` scheme) never rejects the whole profile.
 */
function isExtensionProfileResult(v: unknown): v is ExtensionProfileResult {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  const optStr = (x: unknown): boolean => x === undefined || typeof x === 'string';
  return (
    optStr(o.fullName) &&
    optStr(o.email) &&
    optStr(o.phone) &&
    optStr(o.location) &&
    optStr(o.linkedin) &&
    optStr(o.github) &&
    optStr(o.website) &&
    (o.extraLinks === undefined || Array.isArray(o.extraLinks)) &&
    optStr(o.error)
  );
}

/** Rebuild an {@link ExtensionProfileResult} from only the known, defined keys.
 *  `extraLinks` is filtered (not `.every`-gated) so a single malformed entry
 *  is dropped rather than failing the entire profile. */
function normalizeProfileResult(payload: unknown): ExtensionProfileResult {
  if (!isExtensionProfileResult(payload)) {
    return { error: 'The desktop app sent a malformed profile result.' };
  }
  const src = payload;
  const out: ExtensionProfileResult = {};
  if (src.fullName !== undefined) out.fullName = src.fullName;
  if (src.email !== undefined) out.email = src.email;
  if (src.phone !== undefined) out.phone = src.phone;
  if (src.location !== undefined) out.location = src.location;
  if (src.linkedin !== undefined) out.linkedin = src.linkedin;
  if (src.github !== undefined) out.github = src.github;
  if (src.website !== undefined) out.website = src.website;
  if (src.extraLinks !== undefined) out.extraLinks = src.extraLinks.filter(isLinkEntry);
  if (src.error !== undefined) out.error = src.error;
  return out;
}

/**
 * Hand-written guard for an `applied.result` payload (extension stays
 * zod-free). Mirrors `ExtensionAppliedCheckResultSchema`: `found` must be a
 * boolean; every other field is an optional string, except `appliedAt` (an
 * optional number, epoch ms).
 */
function isExtensionAppliedCheckResult(v: unknown): v is ExtensionAppliedCheckResult {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  const optStr = (x: unknown): boolean => x === undefined || typeof x === 'string';
  return (
    typeof o.found === 'boolean' &&
    optStr(o.applicationId) &&
    optStr(o.status) &&
    optStr(o.title) &&
    (o.appliedAt === undefined || typeof o.appliedAt === 'number') &&
    optStr(o.error)
  );
}

/** Rebuild an {@link ExtensionAppliedCheckResult} from only the known, defined
 *  keys — mirrors `normalizeProfileResult`. */
function normalizeAppliedCheckResult(payload: unknown): ExtensionAppliedCheckResult {
  if (!isExtensionAppliedCheckResult(payload)) {
    return { found: false, error: 'The desktop app sent a malformed applied-check result.' };
  }
  const src = payload;
  const out: ExtensionAppliedCheckResult = { found: src.found };
  if (src.applicationId !== undefined) out.applicationId = src.applicationId;
  if (src.status !== undefined) out.status = src.status;
  if (src.title !== undefined) out.title = src.title;
  if (src.appliedAt !== undefined) out.appliedAt = src.appliedAt;
  if (src.error !== undefined) out.error = src.error;
  return out;
}

/**
 * Hand-written guard for a `status.update` payload (extension stays
 * zod-free). Mirrors `ExtensionStatusUpdateResultSchema`'s discriminated
 * union: `ok:true` requires a string `applicationId` + the literal
 * `status: 'applied'`; `ok:false` requires a string `error`. Success and
 * failure fields can never mix.
 */
function isExtensionStatusUpdateResult(v: unknown): v is ExtensionStatusUpdateResult {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  if (o.ok === true) return typeof o.applicationId === 'string' && o.status === 'applied';
  if (o.ok === false) return typeof o.error === 'string';
  return false;
}

/** Rebuild an {@link ExtensionStatusUpdateResult} from only the known,
 *  defined keys — mirrors `normalizeAppliedCheckResult`. UNLIKE that
 *  malformed-reply fallback, this verb's errors are surfaced to the user, so
 *  the fallback text is written the same way: `ok:false` + a plain `error`. */
function normalizeStatusUpdateResult(payload: unknown): ExtensionStatusUpdateResult {
  if (!isExtensionStatusUpdateResult(payload)) {
    return { ok: false, error: 'The desktop app sent a malformed status-update result.' };
  }
  return payload.ok
    ? { ok: true, applicationId: payload.applicationId, status: payload.status }
    : { ok: false, error: payload.error };
}

/**
 * Hand-written guard for an `answers.save` payload (extension stays
 * zod-free). Mirrors `ExtensionAnswersSaveResultSchema`'s discriminated
 * union: `ok:true` requires a string `applicationId` + numeric
 * `saved`/`skipped` (title/company optional strings); `ok:false` requires a
 * string `error`.
 */
function isExtensionAnswersSaveResult(v: unknown): v is ExtensionAnswersSaveResult {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  const optStr = (x: unknown): boolean => x === undefined || typeof x === 'string';
  if (o.ok === true) {
    return (
      typeof o.applicationId === 'string' &&
      typeof o.saved === 'number' &&
      typeof o.skipped === 'number' &&
      optStr(o.title) &&
      optStr(o.company)
    );
  }
  if (o.ok === false) return typeof o.error === 'string';
  return false;
}

/** Rebuild an {@link ExtensionAnswersSaveResult} from only the known, defined
 *  keys — mirrors `normalizeStatusUpdateResult`. This verb's errors are
 *  surfaced to the user (like `status.update`), so the fallback text is
 *  written the same way: `ok:false` + a plain `error`. */
function normalizeAnswersSaveResult(payload: unknown): ExtensionAnswersSaveResult {
  if (!isExtensionAnswersSaveResult(payload)) {
    return { ok: false, error: 'The desktop app sent a malformed answers-save result.' };
  }
  if (!payload.ok) return { ok: false, error: payload.error };
  const out: ExtensionAnswersSaveResult = {
    ok: true,
    applicationId: payload.applicationId,
    saved: payload.saved,
    skipped: payload.skipped,
  };
  if (payload.title !== undefined) out.title = payload.title;
  if (payload.company !== undefined) out.company = payload.company;
  return out;
}

/** Hand-written guard for one `answers.suggest` suggestion entry (extension
 *  stays zod-free). Mirrors `ExtensionAnswerSuggestionSchema`. */
function isExtensionAnswerSuggestion(v: unknown): v is ExtensionAnswerSuggestion {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  const optStr = (x: unknown): boolean => x === undefined || typeof x === 'string';
  return (
    typeof o.question === 'string' &&
    typeof o.answer === 'string' &&
    optStr(o.sourceCompany) &&
    optStr(o.sourceTitle) &&
    typeof o.sourceQuestion === 'string' &&
    typeof o.score === 'number' &&
    typeof o.salary === 'boolean'
  );
}

/**
 * Hand-written guard for an `answers.suggest` payload (extension stays
 * zod-free). Mirrors `ExtensionAnswersSuggestResultSchema`'s discriminated
 * union: `ok:true` requires a `suggestions` array of well-formed entries;
 * `ok:false` requires a string `error`.
 */
function isExtensionAnswersSuggestResult(v: unknown): v is ExtensionAnswersSuggestResult {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  if (o.ok === true) {
    return Array.isArray(o.suggestions) && o.suggestions.every(isExtensionAnswerSuggestion);
  }
  if (o.ok === false) return typeof o.error === 'string';
  return false;
}

/** Rebuild an {@link ExtensionAnswersSuggestResult} from only the known,
 *  defined keys — mirrors `normalizeAnswersSaveResult`. This verb's errors
 *  are surfaced to the user (like `answers.save`), so the fallback text is
 *  written the same way: `ok:false` + a plain `error`. */
function normalizeAnswersSuggestResult(payload: unknown): ExtensionAnswersSuggestResult {
  if (!isExtensionAnswersSuggestResult(payload)) {
    return { ok: false, error: 'The desktop app sent a malformed suggestions result.' };
  }
  if (!payload.ok) return { ok: false, error: payload.error };
  const suggestions: ExtensionAnswerSuggestion[] = payload.suggestions.map((s) => {
    const out: ExtensionAnswerSuggestion = {
      question: s.question,
      answer: s.answer,
      sourceQuestion: s.sourceQuestion,
      score: s.score,
      salary: s.salary,
    };
    if (s.sourceCompany !== undefined) out.sourceCompany = s.sourceCompany;
    if (s.sourceTitle !== undefined) out.sourceTitle = s.sourceTitle;
    return out;
  });
  return { ok: true, suggestions };
}

// ── transport seam ──────────────────────────────────────────────────────────

/**
 * One open connection to the desktop bridge. `onMessage` delivers a PARSED
 * object — ws JSON.parses the string frame, native messaging already auto-parses
 * the JSON for us.
 */
interface BridgeTransport {
  send(envelope: ExtensionEnvelope): void;
  onMessage(cb: (env: unknown) => void): void;
  onClose(cb: () => void): void;
  close(): void;
}

class WebSocketTransport implements BridgeTransport {
  constructor(private readonly socket: WebSocket) {}

  send(envelope: ExtensionEnvelope): void {
    this.socket.send(JSON.stringify(envelope));
  }

  onMessage(cb: (env: unknown) => void): void {
    this.socket.addEventListener('message', (ev: MessageEvent) => {
      if (typeof ev.data !== 'string') return;
      let parsed: unknown;
      try {
        parsed = JSON.parse(ev.data);
      } catch {
        return;
      }
      cb(parsed);
    });
  }

  onClose(cb: () => void): void {
    this.socket.addEventListener('close', cb);
    // `close` fires after `error`; cleanup happens there. No error handler needed.
  }

  close(): void {
    this.socket.close();
  }
}

/** Distinguishable native-connect failures (see {@link connectNative}). */
const NATIVE_UNAVAILABLE = 'native_unavailable'; // host not registered → fall back to ws
const NATIVE_APP_DOWN = 'native_app_down'; // host ran, app is down → app_not_running, no ws

class NativeMessagingTransport implements BridgeTransport {
  constructor(private readonly port: Browser.runtime.Port) {}

  send(envelope: ExtensionEnvelope): void {
    this.port.postMessage(envelope);
  }

  onMessage(cb: (env: unknown) => void): void {
    this.port.onMessage.addListener((msg: unknown) => {
      // `bridge.ready` is transport-local; never forward it to result correlation.
      if (isBridgeReady(msg)) {
        // ok:false after connect = the app's bridge went away → behave like close.
        if (!msg.ok) this.close();
        return;
      }
      cb(msg);
    });
  }

  onClose(cb: () => void): void {
    this.port.onDisconnect.addListener(cb);
  }

  close(): void {
    this.port.disconnect();
  }
}

interface BridgeReady {
  type: 'bridge.ready';
  ok: boolean;
}

function isBridgeReady(msg: unknown): msg is BridgeReady {
  return (
    typeof msg === 'object' &&
    msg !== null &&
    (msg as { type?: unknown }).type === 'bridge.ready' &&
    typeof (msg as { ok?: unknown }).ok === 'boolean'
  );
}

/**
 * Connect via native messaging, resolving only after `bridge.ready{ok:true}`.
 * Rejects with {@link NATIVE_APP_DOWN} on `ok:false` (host reachable, app down)
 * or {@link NATIVE_UNAVAILABLE} on pre-ready disconnect / ready-timeout (host not
 * registered → caller falls back to ws). THROWS SYNCHRONOUSLY if `connectNative`
 * itself fails so the caller can start the ws probe in the same tick (the ws
 * reconnect test asserts the probe fires synchronously after the timer).
 */
function connectNative(): Promise<NativeMessagingTransport> {
  const port: Browser.runtime.Port = browser.runtime.connectNative(HOST_NAME);

  return new Promise<NativeMessagingTransport>((resolve, reject) => {
    let settled = false;
    const finish = (fn: () => void): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      fn();
    };
    // Pre-attach failures must close the native Port; otherwise the browser keeps
    // the spawned host process alive across reconnect-backoff attempts.
    const rejectWith = (reason: string, disconnect: boolean): void => {
      finish(() => {
        if (disconnect) {
          try {
            port.disconnect();
          } catch {
            /* already closed */
          }
        }
        reject(new Error(reason));
      });
    };

    const timer = setTimeout(() => rejectWith(NATIVE_UNAVAILABLE, true), READY_TIMEOUT_MS);

    port.onMessage.addListener((msg: unknown) => {
      if (!isBridgeReady(msg)) return; // ignore stray frames before ready
      if (msg.ok) finish(() => resolve(new NativeMessagingTransport(port)));
      else rejectWith(NATIVE_APP_DOWN, true);
    });
    port.onDisconnect.addListener(() => {
      // Disconnect before ready = host not registered (lastError set); the port
      // already fired disconnect, so do NOT call disconnect() again here.
      rejectWith(NATIVE_UNAVAILABLE, false);
    });
  });
}

export class BridgeClient {
  private transport: BridgeTransport | null = null;
  private port: number | null = null;
  private phase: BridgePhase = 'searching';
  private readonly pending = new Map<string, PendingResolver>();
  /** In-flight `profile.get` resolvers, correlated by `reqId` (autofill). Kept
   *  separate from {@link pending} so the battle-tested import/auth path is
   *  untouched; both share the {@link timers} map (reqIds are unique UUIDs). */
  private readonly pendingProfile = new Map<string, ProfileResolver>();
  /** In-flight `applied.check` resolvers, correlated by `reqId`. Kept separate
   *  from {@link pending} for the same reason as {@link pendingProfile}. */
  private readonly pendingApplied = new Map<string, AppliedResolver>();
  /** In-flight `status.update` resolvers, correlated by `reqId`. Kept separate
   *  from {@link pending} for the same reason as {@link pendingProfile}. */
  private readonly pendingStatus = new Map<string, StatusResolver>();
  /** In-flight `answers.save` resolvers, correlated by `reqId`. Kept separate
   *  from {@link pending} for the same reason as {@link pendingProfile}. */
  private readonly pendingAnswers = new Map<string, AnswersResolver>();
  /** In-flight `answers.suggest` resolvers, correlated by `reqId`. Kept
   *  separate from {@link pending} for the same reason as {@link pendingProfile}. */
  private readonly pendingSuggest = new Map<string, SuggestResolver>();
  private readonly timers = new Map<string, ReturnType<typeof setTimeout>>();
  private backoffIndex = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private disposed = false;
  /**
   * The in-flight `doConnect()` promise (transport probe THROUGH the full v2
   * handshake), or `null` when no connection attempt is running. A concurrent
   * `ensureConnected()` call AWAITS this SAME promise instead of returning early
   * — this is the fix for the "app frame reaches an unverified peer" race: the
   * transport can be non-null while `performHandshake` is still verifying the
   * peer's `serverProof`, so `isOpen()` alone is never a safe "ready" signal for
   * a concurrent caller. See `importJob`/`getProfile`, which additionally gate on
   * `this.phase === 'connected'` (the authenticated session), never on transport
   * liveness.
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
   * Set while a v2 handshake is in flight. `handshakeFrame` receives every
   * incoming frame (so `performHandshake` can advance step by step);
   * `handshakeClosed` is invoked on socket close so a step resolves as `closed`.
   * Both are cleared when a step settles.
   */
  private handshakeFrame: ((env: Partial<ExtensionEnvelope>) => void) | null = null;
  private handshakeClosed: (() => void) | null = null;

  /** Notified on every phase change so the background can broadcast status. */
  constructor(
    private readonly onPhaseChange: (status: BridgeStatus) => void,
    /** Optional: called on connect to retrieve the stored pairing token for the auth handshake. */
    private readonly getStoredToken?: () => Promise<string | null>
  ) {}

  status(): BridgeStatus {
    return { phase: this.phase, port: this.port };
  }

  /** Whether a transport is currently live. */
  isOpen(): boolean {
    return this.transport !== null;
  }

  /**
   * Ensure a connection: if already open (authenticated), no-op; otherwise try
   * native then ws — INCLUDING the full v2 handshake. Safe to call repeatedly
   * (popup-open wake, reconnect button, a concurrent `importJob`/`getProfile`).
   *
   * A concurrent call while a connection attempt is already running AWAITS the
   * SAME promise rather than short-circuiting — critical because `attach()` sets
   * `this.transport` before `performHandshake` has verified the peer's
   * `serverProof`. Without this, a second caller would see `isOpen()` (transport
   * non-null) and treat an unverified transport as ready, sending an app frame
   * (e.g. the active-tab DOM) to a peer that has not yet proven it knows the
   * pairing token.
   */
  async ensureConnected(): Promise<void> {
    if (this.disposed || this.phase === 'bad_token') return;
    if (this.connectPromise) return this.connectPromise;
    if (this.isOpen()) return; // already open + past a settled handshake
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
    // Native first. ponytail: serial single-in-flight is fine — the popup
    // imports one job at a time, matching the Rust host's serial relay.
    // `connectNative()` THROWS SYNCHRONOUSLY when the host isn't registered, so
    // building the readiness promise is separated from awaiting it — that keeps
    // the ws fallback probe firing in the SAME tick (no microtask hop) when
    // native is unavailable, which the ws reconnect test relies on.
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

    const socket = await this.probeRange();
    if (socket) {
      await this.attach(new WebSocketTransport(socket));
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
  }

  /** Tear down the transport and cancel timers (worker shutdown / manual reset). */
  dispose(): void {
    this.disposed = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    // Settle any in-flight handshake step so its timeout timer is cleared.
    this.handshakeClosed?.();
    this.handshakeFrame = null;
    this.handshakeClosed = null;
    for (const t of this.timers.values()) clearTimeout(t);
    this.timers.clear();
    this.pending.clear();
    this.pendingProfile.clear();
    this.pendingApplied.clear();
    this.pendingStatus.clear();
    this.pendingAnswers.clear();
    this.pendingSuggest.clear();
    this.transport?.close();
    this.transport = null;
  }

  /**
   * Send an `import.request` and resolve with the validated `import.result`
   * payload. Rejects on no connection, timeout, or a malformed reply. The frame
   * carries NO token — the socket is already authenticated by the v2 handshake.
   *
   * Gated on `this.phase === 'connected'` — the AUTHENTICATED SESSION — never on
   * transport liveness. A non-null `this.transport` does NOT mean the peer is
   * verified: `attach()` sets it before `performHandshake` checks the peer's
   * `serverProof`. Only `'connected'` means the mutual handshake completed, so
   * this is the seam that stops the active-tab DOM (or the profile) from ever
   * reaching an unverified/rogue peer.
   */
  async importJob(payload: ExtensionImportRequest): Promise<ExtensionImportResult> {
    await this.ensureConnected();
    if (this.phase !== 'connected' || !this.transport) {
      throw new Error('Desktop app not reachable. Is AI Job Hunter running?');
    }
    const transport = this.transport;

    const reqId = newReqId();
    const envelope: ExtensionEnvelope = {
      type: EXTENSION_MESSAGE_TYPES.importRequest,
      reqId,
      payload,
    };

    return new Promise<ExtensionImportResult>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(reqId);
        this.timers.delete(reqId);
        reject(new Error('Timed out waiting for the desktop app to respond.'));
      }, REQUEST_TIMEOUT_MS);
      this.timers.set(reqId, timer);
      this.pending.set(reqId, resolve);

      try {
        transport.send(envelope);
      } catch (err) {
        clearTimeout(timer);
        this.timers.delete(reqId);
        this.pending.delete(reqId);
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    });
  }

  /**
   * Send a `profile.get` and resolve with the validated `profile.result`
   * payload — the contact profile for assisted autofill, or `{ error }` when the
   * desktop refuses (autofill opt-in off) or the reply is malformed. Rejects only
   * on no connection / timeout / send failure. The profile is returned to the
   * caller transiently and is never stored by this client.
   */
  async getProfile(): Promise<ExtensionProfileResult> {
    await this.ensureConnected();
    // Gate on the authenticated session — see the note on `importJob`. The
    // Contact Profile is the highest-sensitivity payload this client sends;
    // never hand it to a peer that has not proven it knows the pairing token.
    if (this.phase !== 'connected' || !this.transport) {
      throw new Error('Desktop app not reachable. Is AI Job Hunter running?');
    }
    const transport = this.transport;

    const reqId = newReqId();
    const envelope: ExtensionEnvelope = {
      type: EXTENSION_MESSAGE_TYPES.profileGet,
      reqId,
      payload: null,
    };

    return new Promise<ExtensionProfileResult>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pendingProfile.delete(reqId);
        this.timers.delete(reqId);
        reject(new Error('Timed out waiting for the desktop app to respond.'));
      }, REQUEST_TIMEOUT_MS);
      this.timers.set(reqId, timer);
      this.pendingProfile.set(reqId, resolve);

      try {
        transport.send(envelope);
      } catch (err) {
        clearTimeout(timer);
        this.timers.delete(reqId);
        this.pendingProfile.delete(reqId);
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    });
  }

  /**
   * Send an `applied.check` for `url` and resolve with the validated
   * `applied.result` payload — whether an Application already exists for it,
   * or `{ found: false, error }` when the reply is malformed. Rejects only on
   * no connection / timeout / send failure; the popup's fire-and-forget auto
   * check swallows every rejection into a silent no-render (read-only, never
   * blocks the import controls — see `popup.ts`).
   */
  async checkApplied(url: string): Promise<ExtensionAppliedCheckResult> {
    await this.ensureConnected();
    if (this.phase !== 'connected' || !this.transport) {
      throw new Error('Desktop app not reachable. Is AI Job Hunter running?');
    }
    const transport = this.transport;

    const reqId = newReqId();
    const envelope: ExtensionEnvelope = {
      type: EXTENSION_MESSAGE_TYPES.appliedCheck,
      reqId,
      payload: { url },
    };

    return new Promise<ExtensionAppliedCheckResult>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pendingApplied.delete(reqId);
        this.timers.delete(reqId);
        reject(new Error('Timed out waiting for the desktop app to respond.'));
      }, REQUEST_TIMEOUT_MS);
      this.timers.set(reqId, timer);
      this.pendingApplied.set(reqId, resolve);

      try {
        transport.send(envelope);
      } catch (err) {
        clearTimeout(timer);
        this.timers.delete(reqId);
        this.pendingApplied.delete(reqId);
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    });
  }

  /**
   * Send a `status.update { url, to: 'applied' }` and resolve with the
   * validated `status.update` payload. Rejects only on no connection /
   * timeout / send failure — UNLIKE `checkApplied`, a well-formed `ok:false`
   * reply is NOT folded away here: this is a deliberate click action, so the
   * caller (background.ts) must pass the `error` straight through to the
   * popup instead of swallowing it.
   */
  async updateStatus(url: string): Promise<ExtensionStatusUpdateResult> {
    await this.ensureConnected();
    if (this.phase !== 'connected' || !this.transport) {
      throw new Error('Desktop app not reachable. Is AI Job Hunter running?');
    }
    const transport = this.transport;

    const reqId = newReqId();
    const envelope: ExtensionEnvelope = {
      type: EXTENSION_MESSAGE_TYPES.statusUpdate,
      reqId,
      payload: { url, to: 'applied' },
    };

    return new Promise<ExtensionStatusUpdateResult>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pendingStatus.delete(reqId);
        this.timers.delete(reqId);
        reject(new Error('Timed out waiting for the desktop app to respond.'));
      }, REQUEST_TIMEOUT_MS);
      this.timers.set(reqId, timer);
      this.pendingStatus.set(reqId, resolve);

      try {
        transport.send(envelope);
      } catch (err) {
        clearTimeout(timer);
        this.timers.delete(reqId);
        this.pendingStatus.delete(reqId);
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    });
  }

  /**
   * Send an `answers.save { url, answers }` and resolve with the validated
   * `answers.result` payload. Rejects only on no connection / timeout / send
   * failure — like `updateStatus`, a well-formed `ok:false` reply is NOT
   * folded away here: this is a deliberate click action, so the caller
   * (background.ts) must pass the `error` straight through to the popup
   * instead of swallowing it.
   */
  async saveAnswers(
    url: string,
    answers: ExtensionAnswerPair[]
  ): Promise<ExtensionAnswersSaveResult> {
    await this.ensureConnected();
    if (this.phase !== 'connected' || !this.transport) {
      throw new Error('Desktop app not reachable. Is AI Job Hunter running?');
    }
    const transport = this.transport;

    const reqId = newReqId();
    const envelope: ExtensionEnvelope = {
      type: EXTENSION_MESSAGE_TYPES.answersSave,
      reqId,
      payload: { url, answers },
    };

    return new Promise<ExtensionAnswersSaveResult>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pendingAnswers.delete(reqId);
        this.timers.delete(reqId);
        reject(new Error('Timed out waiting for the desktop app to respond.'));
      }, REQUEST_TIMEOUT_MS);
      this.timers.set(reqId, timer);
      this.pendingAnswers.set(reqId, resolve);

      try {
        transport.send(envelope);
      } catch (err) {
        clearTimeout(timer);
        this.timers.delete(reqId);
        this.pendingAnswers.delete(reqId);
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    });
  }

  /**
   * Send an `answers.suggest { questions }` and resolve with the validated
   * `answers.suggest.result` payload — the desktop's fuzzy-matched
   * suggestions (or a refusal). Rejects only on no connection / timeout /
   * send failure — like `saveAnswers`, a well-formed `ok:false` reply is NOT
   * folded away here: this answers a deliberate click, so the caller
   * (background.ts) must pass the `error` straight through to the popup.
   */
  async suggestAnswers(questions: string[]): Promise<ExtensionAnswersSuggestResult> {
    await this.ensureConnected();
    if (this.phase !== 'connected' || !this.transport) {
      throw new Error('Desktop app not reachable. Is AI Job Hunter running?');
    }
    const transport = this.transport;

    const reqId = newReqId();
    const envelope: ExtensionEnvelope = {
      type: EXTENSION_MESSAGE_TYPES.answersSuggest,
      reqId,
      payload: { questions },
    };

    return new Promise<ExtensionAnswersSuggestResult>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pendingSuggest.delete(reqId);
        this.timers.delete(reqId);
        reject(new Error('Timed out waiting for the desktop app to respond.'));
      }, REQUEST_TIMEOUT_MS);
      this.timers.set(reqId, timer);
      this.pendingSuggest.set(reqId, resolve);

      try {
        transport.send(envelope);
      } catch (err) {
        clearTimeout(timer);
        this.timers.delete(reqId);
        this.pendingSuggest.delete(reqId);
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    });
  }

  // ── internals ─────────────────────────────────────────────────────────────

  private setPhase(phase: BridgePhase): void {
    if (this.phase === phase) return;
    this.phase = phase;
    this.onPhaseChange(this.status());
  }

  /** Try each port in order; resolve the first socket that reaches OPEN. */
  private async probeRange(): Promise<WebSocket | null> {
    // v2 mutual handshake: even though we still connect to the FIRST loopback
    // port that answers, the pairing token is NEVER sent — the extension proves
    // knowledge of it via HMAC and, crucially, verifies the desktop's own
    // `serverProof` before sending ANY import/profile frame. A port-squatter that
    // cannot produce a valid serverProof lands us in `bad_token` with zero PII
    // sent (see performHandshake). See the "Threat model" section in
    // apps/extension/README.md.
    this.setPhase('searching');
    for (let port = PORT_START; port <= PORT_END; port += 1) {
      const socket = await this.tryOpen(port);
      if (socket) {
        this.port = port;
        return socket;
      }
    }
    this.port = null;
    return null;
  }

  /** Open one port with a timeout; resolve the socket or null on failure. */
  private tryOpen(port: number): Promise<WebSocket | null> {
    return new Promise((resolve) => {
      let settled = false;
      let socket: WebSocket;
      try {
        socket = new WebSocket(`ws://127.0.0.1:${port}`);
      } catch {
        resolve(null);
        return;
      }
      const timer = setTimeout(() => {
        if (settled) return;
        settled = true;
        socket.close();
        resolve(null);
      }, OPEN_TIMEOUT_MS);

      socket.addEventListener('open', () => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resolve(socket);
      });
      const fail = (): void => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resolve(null);
      };
      socket.addEventListener('error', fail);
      socket.addEventListener('close', fail);
    });
  }

  /** Wire an opened transport, then run the v2 handshake if a token is stored. */
  private async attach(transport: BridgeTransport): Promise<void> {
    this.transport = transport;
    this.backoffIndex = 0;
    this.authRejected = false;
    this.outdated = false;

    transport.onMessage((env) => this.onMessage(env));
    transport.onClose(() => {
      this.transport = null;
      this.failAllPending('Connection to the desktop app closed.');
      // If a handshake is in flight, let it settle (it decides outdated /
      // bad_token / reconnect) — don't set a phase or reconnect from here.
      if (this.handshakeClosed) {
        const notify = this.handshakeClosed;
        this.handshakeClosed = null;
        this.handshakeFrame = null;
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
      } else {
        this.setPhase('app_not_running');
        this.scheduleReconnect();
      }
    });

    // v2 mutual handshake: run it only if a token is stored. Without a token the
    // socket is open but unpaired — computeStatus() surfaces that as 'not_paired'.
    const token = this.getStoredToken ? await this.getStoredToken() : null;
    if (!token) {
      this.setPhase('connected');
      return;
    }
    await this.performHandshake(token);
  }

  /**
   * Drive the v2 mutual HMAC handshake to completion. The token is used ONLY as
   * an HMAC key — it never goes on the wire. The extension MUST verify the
   * desktop's `serverProof` before this resolves `connected`; a peer that can't
   * prove it knows the token (rogue/port-squatter) never receives any PII.
   */
  private async performHandshake(token: string): Promise<void> {
    const transport = this.transport;
    if (!transport) return;

    const clientNonce = randomNonceHex();

    // Step 1: hello (no token) → await the desktop's challenge.
    transport.send({
      type: EXTENSION_MESSAGE_TYPES.hello,
      reqId: newReqId(),
      payload: { protocol: EXTENSION_PROTOCOL_VERSION, clientNonce },
    });
    const step1 = await this.awaitHandshakeFrame();
    if (step1.kind === 'timeout') {
      // Total silence — treat as a transient transport failure (recoverable).
      return this.finishHandshake('app_not_running');
    }
    if (step1.kind === 'closed' || step1.env.type !== EXTENSION_MESSAGE_TYPES.challenge) {
      // The desktop closed without a challenge, or replied a non-challenge frame
      // (e.g. an old desktop's import.result / update.required). Either way it
      // does not speak v2 → the user must update the desktop app.
      return this.finishHandshake('outdated');
    }
    const serverNonce = readHexField(step1.env.payload, 'serverNonce');
    // Defense-in-depth (mirrors the Rust `is_valid_nonce` shape check on the
    // client nonce): reject a malformed/oversized serverNonce as a clean
    // handshake failure BEFORE it feeds the HMAC — never silently proceed with
    // attacker-shaped input. Grouped with "missing" as `outdated`: a peer whose
    // very first reply doesn't carry a well-formed nonce does not properly speak
    // v2.
    if (!serverNonce || !isValidNonceHex(serverNonce)) {
      return this.finishHandshake('outdated');
    }

    // Step 3: prove we know the token (HMAC over the client role) → await auth.ok.
    const clientProof = await computeProof(token, 'client', serverNonce, clientNonce);
    if (!this.transport) {
      // Socket closed while we were computing the proof. AMBIGUOUS: the Rust
      // `Unauthorized` path closes WITHOUT a reply BY DESIGN (see
      // extension_bridge/mod.rs), so this is indistinguishable from a genuine
      // app crash/restart. Never assert a hard wrong-token verdict from silence
      // alone — recoverable, consistent with the step-1 timeout above.
      return this.finishHandshake('app_not_running');
    }
    this.transport.send({
      type: EXTENSION_MESSAGE_TYPES.auth,
      reqId: newReqId(),
      payload: { proof: clientProof },
    });
    const step2 = await this.awaitHandshakeFrame();
    if (step2.kind === 'closed' || step2.kind === 'timeout') {
      // No auth.ok arrived — silence (closed or timed out) after we already sent
      // our proof. Same ambiguity as above: the Rust `Unauthorized` path closes
      // without a reply, indistinguishable from a crash. Recoverable.
      return this.finishHandshake('app_not_running');
    }
    if (step2.env.type !== EXTENSION_MESSAGE_TYPES.authOk) {
      // The peer DID reply — with something other than auth.ok. Unlike silence,
      // this is an actual, non-ambiguous response from a peer that spoke v2 far
      // enough to send a challenge; treat it as untrusted → bad_token (re-pair).
      return this.finishHandshake('bad_token');
    }

    // Step 5: verify the desktop's serverProof CONSTANT-TIME before trusting it.
    const serverProof = readHexField(step2.env.payload, 'serverProof');
    const expected = await computeProof(token, 'server', serverNonce, clientNonce);
    if (!serverProof || !constantTimeHexEqual(serverProof, expected)) {
      // The peer cannot prove it knows the token (rogue/port-squatter). We have
      // sent NO PII (import/profile only happen after 'connected'); drop the
      // socket and surface bad_token.
      return this.finishHandshake('bad_token');
    }

    // Mutual auth complete — the socket is authenticated.
    this.handshakeFrame = null;
    this.handshakeClosed = null;
    this.setPhase('connected');
  }

  /**
   * Await the next handshake frame, a socket close, or a per-step timeout. While
   * pending, `onMessage` routes EVERY incoming frame here (no import/profile
   * frames are expected before the socket is authenticated).
   */
  private awaitHandshakeFrame(): Promise<HandshakeStep> {
    return new Promise<HandshakeStep>((resolve) => {
      let done = false;
      const settle = (step: HandshakeStep): void => {
        if (done) return;
        done = true;
        clearTimeout(timer);
        this.handshakeFrame = null;
        this.handshakeClosed = null;
        resolve(step);
      };
      const timer = setTimeout(() => settle({ kind: 'timeout' }), HANDSHAKE_TIMEOUT_MS);
      this.handshakeFrame = (env) => settle({ kind: 'frame', env });
      this.handshakeClosed = () => settle({ kind: 'closed' });
    });
  }

  /**
   * Terminal handshake outcome: clear the handshake hooks, set the phase, and
   * drop the transport. `bad_token` / `outdated` suppress the reconnect loop;
   * `app_not_running` schedules a reconnect (a transient transport blip).
   */
  private finishHandshake(phase: 'app_not_running' | 'outdated' | 'bad_token'): void {
    this.handshakeFrame = null;
    this.handshakeClosed = null;
    if (phase === 'bad_token') this.authRejected = true;
    if (phase === 'outdated') this.outdated = true;
    const transport = this.transport;
    this.transport = null;
    transport?.close();
    if (this.disposed) return;
    this.setPhase(phase);
    if (phase === 'app_not_running') this.scheduleReconnect();
  }

  /** Clear + return the correlation timer for a settled `reqId`. */
  private clearTimer(reqId: string): void {
    const timer = this.timers.get(reqId);
    if (timer) {
      clearTimeout(timer);
      this.timers.delete(reqId);
    }
  }

  /** Match a parsed reply envelope by `reqId`, validate payload, resolve caller. */
  private onMessage(parsed: unknown): void {
    if (typeof parsed !== 'object' || parsed === null) return;
    const env = parsed as Partial<ExtensionEnvelope>;

    // While the v2 handshake is in flight, EVERY frame goes to the handshake
    // driver (challenge / auth.ok / an unexpected non-v2 reply). No import or
    // profile frames are expected before the socket is authenticated.
    if (this.handshakeFrame) {
      this.handshakeFrame(env);
      return;
    }

    const reqId = typeof env.reqId === 'string' ? env.reqId : '';

    // profile.result → the assisted-autofill contact profile (separate map).
    if (env.type === EXTENSION_MESSAGE_TYPES.profileResult) {
      const resolveProfile = this.pendingProfile.get(reqId);
      if (typeof resolveProfile !== 'function') return;
      this.pendingProfile.delete(reqId);
      this.clearTimer(reqId);
      resolveProfile(normalizeProfileResult(env.payload));
      return;
    }

    // applied.result → the "have I already applied?" outcome (separate map).
    if (env.type === EXTENSION_MESSAGE_TYPES.appliedResult) {
      const resolveApplied = this.pendingApplied.get(reqId);
      if (typeof resolveApplied !== 'function') return;
      this.pendingApplied.delete(reqId);
      this.clearTimer(reqId);
      resolveApplied(normalizeAppliedCheckResult(env.payload));
      return;
    }

    // status.result → the "mark as applied" outcome (separate map).
    if (env.type === EXTENSION_MESSAGE_TYPES.statusResult) {
      const resolveStatus = this.pendingStatus.get(reqId);
      if (typeof resolveStatus !== 'function') return;
      this.pendingStatus.delete(reqId);
      this.clearTimer(reqId);
      resolveStatus(normalizeStatusUpdateResult(env.payload));
      return;
    }

    // answers.result → the "save my answers" outcome (separate map).
    if (env.type === EXTENSION_MESSAGE_TYPES.answersResult) {
      const resolveAnswers = this.pendingAnswers.get(reqId);
      if (typeof resolveAnswers !== 'function') return;
      this.pendingAnswers.delete(reqId);
      this.clearTimer(reqId);
      resolveAnswers(normalizeAnswersSaveResult(env.payload));
      return;
    }

    // answers.suggest.result → the "suggest answers" outcome (separate map).
    if (env.type === EXTENSION_MESSAGE_TYPES.answersSuggestResult) {
      const resolveSuggest = this.pendingSuggest.get(reqId);
      if (typeof resolveSuggest !== 'function') return;
      this.pendingSuggest.delete(reqId);
      this.clearTimer(reqId);
      resolveSuggest(normalizeAnswersSuggestResult(env.payload));
      return;
    }

    if (env.type !== EXTENSION_MESSAGE_TYPES.importResult) return;
    const resolve = this.pending.get(reqId);
    if (typeof resolve !== 'function') return;

    this.pending.delete(reqId);
    this.clearTimer(reqId);

    if (!isExtensionImportResult(env.payload)) {
      resolve({ error: 'The desktop app sent a malformed import result.' });
      return;
    }
    // Rebuild from only the known, defined keys — mirrors the old zod
    // `.safeParse(...).data` which STRIPPED unknown keys (the guard alone would
    // pass extra keys through). Copying only defined keys also keeps `toEqual`
    // equality with the minimal payloads tests send (vitest treats an explicit
    // `undefined` key as present).
    const src = env.payload;
    const normalized: ExtensionImportResult = {};
    if (src.applicationId !== undefined) normalized.applicationId = src.applicationId;
    if (src.status !== undefined) normalized.status = src.status;
    if (src.title !== undefined) normalized.title = src.title;
    if (src.company !== undefined) normalized.company = src.company;
    if (src.matchScore !== undefined) normalized.matchScore = src.matchScore;
    if (src.error !== undefined) normalized.error = src.error;
    if (src.partial !== undefined) normalized.partial = src.partial;
    resolve(normalized);
  }

  private failAllPending(reason: string): void {
    for (const [reqId, resolve] of this.pending.entries()) {
      const timer = this.timers.get(reqId);
      if (timer) clearTimeout(timer);
      resolve({ error: reason });
    }
    for (const [reqId, resolve] of this.pendingProfile.entries()) {
      const timer = this.timers.get(reqId);
      if (timer) clearTimeout(timer);
      resolve({ error: reason });
    }
    for (const [reqId, resolve] of this.pendingApplied.entries()) {
      const timer = this.timers.get(reqId);
      if (timer) clearTimeout(timer);
      resolve({ found: false, error: reason });
    }
    for (const [reqId, resolve] of this.pendingStatus.entries()) {
      const timer = this.timers.get(reqId);
      if (timer) clearTimeout(timer);
      resolve({ ok: false, error: reason });
    }
    for (const [reqId, resolve] of this.pendingAnswers.entries()) {
      const timer = this.timers.get(reqId);
      if (timer) clearTimeout(timer);
      resolve({ ok: false, error: reason });
    }
    for (const [reqId, resolve] of this.pendingSuggest.entries()) {
      const timer = this.timers.get(reqId);
      if (timer) clearTimeout(timer);
      resolve({ ok: false, error: reason });
    }
    this.pending.clear();
    this.pendingProfile.clear();
    this.pendingApplied.clear();
    this.pendingStatus.clear();
    this.pendingAnswers.clear();
    this.pendingSuggest.clear();
    this.timers.clear();
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
