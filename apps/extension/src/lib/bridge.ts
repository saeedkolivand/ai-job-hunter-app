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
 *    `apps/tauri/src-tauri/src/extension_bridge/mod.rs::PORT_RANGE`). Used when
 *    the native host isn't registered (old/never-installed app).
 *
 * Either way we hold a SINGLE transport, send `import.request` envelopes (token
 * on every frame), and correlate replies by `reqId`.
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
  type ExtensionEnvelope,
  type ExtensionImportRequest,
  type ExtensionImportResult,
} from '@ajh/shared/extension-protocol';

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

/** How long to wait for the native host's `bridge.ready` before falling back to ws. */
const READY_TIMEOUT_MS = 1_500;

type PendingResolver = (result: ExtensionImportResult) => void;

export type BridgePhase = 'searching' | 'connected' | 'app_not_running';

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
    (o.matchScore === undefined || typeof o.matchScore === 'number')
  );
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

    const timer = setTimeout(
      () => finish(() => reject(new Error(NATIVE_UNAVAILABLE))),
      READY_TIMEOUT_MS
    );

    port.onMessage.addListener((msg: unknown) => {
      if (!isBridgeReady(msg)) return; // ignore stray frames before ready
      finish(() =>
        msg.ok ? resolve(new NativeMessagingTransport(port)) : reject(new Error(NATIVE_APP_DOWN))
      );
    });
    port.onDisconnect.addListener(() => {
      // Disconnect before ready = host not registered (lastError set).
      finish(() => reject(new Error(NATIVE_UNAVAILABLE)));
    });
  });
}

export class BridgeClient {
  private transport: BridgeTransport | null = null;
  private port: number | null = null;
  private phase: BridgePhase = 'searching';
  private readonly pending = new Map<string, PendingResolver>();
  private readonly timers = new Map<string, ReturnType<typeof setTimeout>>();
  private backoffIndex = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private disposed = false;
  private connecting = false;

  /** Notified on every phase change so the background can broadcast status. */
  constructor(private readonly onPhaseChange: (status: BridgeStatus) => void) {}

  status(): BridgeStatus {
    return { phase: this.phase, port: this.port };
  }

  /** Whether a transport is currently live. */
  isOpen(): boolean {
    return this.transport !== null;
  }

  /**
   * Ensure a connection: if already open, no-op; otherwise try native then ws.
   * Safe to call repeatedly (popup-open wake, reconnect button).
   */
  async ensureConnected(): Promise<void> {
    if (this.disposed || this.isOpen() || this.connecting) return;
    this.connecting = true;
    try {
      this.setPhase('searching');
      // Native first. ponytail: serial single-in-flight is fine — the popup
      // imports one job at a time, matching the Rust host's serial relay.
      // `connectNative()` THROWS SYNCHRONOUSLY when the host isn't registered, so
      // building the readiness promise is separated from awaiting it — that keeps
      // the ws fallback probe firing in the SAME tick (no microtask hop) when
      // native is unavailable, which the ws reconnect test relies on.
      let readyPromise: Promise<NativeMessagingTransport> | null = null;
      try {
        readyPromise = connectNative();
      } catch {
        readyPromise = null; // host not registered → straight to ws, same tick.
      }
      if (readyPromise) {
        try {
          const native = await readyPromise;
          this.port = null; // native has no port number; diagnostics only.
          this.attach(native);
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
        this.attach(new WebSocketTransport(socket));
      } else {
        this.setPhase('app_not_running');
        this.scheduleReconnect();
      }
    } finally {
      this.connecting = false;
    }
  }

  /** Tear down the transport and cancel timers (worker shutdown / manual reset). */
  dispose(): void {
    this.disposed = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    for (const t of this.timers.values()) clearTimeout(t);
    this.timers.clear();
    this.pending.clear();
    this.transport?.close();
    this.transport = null;
  }

  /**
   * Send an `import.request` and resolve with the validated `import.result`
   * payload. Rejects on no connection, timeout, or a malformed reply.
   */
  async importJob(token: string, payload: ExtensionImportRequest): Promise<ExtensionImportResult> {
    await this.ensureConnected();
    const transport = this.transport;
    if (!transport) {
      throw new Error('Desktop app not reachable. Is AI Job Hunter running?');
    }

    const reqId = newReqId();
    const envelope: ExtensionEnvelope = {
      type: EXTENSION_MESSAGE_TYPES.importRequest,
      token,
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

  // ── internals ─────────────────────────────────────────────────────────────

  private setPhase(phase: BridgePhase): void {
    if (this.phase === phase) return;
    this.phase = phase;
    this.onPhaseChange(this.status());
  }

  /** Try each port in order; resolve the first socket that reaches OPEN. */
  private async probeRange(): Promise<WebSocket | null> {
    // TODO(bridge): the token is sent to the FIRST loopback port that answers in
    // this range, so a same-account local process squatting a port before the app
    // binds could harvest it on first import (accepted v1 risk). See the
    // "Threat model" section in apps/extension/README.md for the bounded impact
    // and the planned fix (server→client challenge HMAC over an in-app nonce, or
    // the native-messaging fallback, which cannot be port-squatted).
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

  /** Wire an opened transport: register handlers and reset backoff. */
  private attach(transport: BridgeTransport): void {
    this.transport = transport;
    this.backoffIndex = 0;
    this.setPhase('connected');

    transport.onMessage((env) => this.onMessage(env));
    transport.onClose(() => {
      this.transport = null;
      this.failAllPending('Connection to the desktop app closed.');
      if (!this.disposed) {
        this.setPhase('app_not_running');
        this.scheduleReconnect();
      }
    });
  }

  /** Match a parsed reply envelope by `reqId`, validate payload, resolve caller. */
  private onMessage(parsed: unknown): void {
    if (typeof parsed !== 'object' || parsed === null) return;
    const env = parsed as Partial<ExtensionEnvelope>;
    if (env.type !== EXTENSION_MESSAGE_TYPES.importResult) return;
    const reqId = typeof env.reqId === 'string' ? env.reqId : '';
    const resolve = this.pending.get(reqId);
    if (typeof resolve !== 'function') return;

    this.pending.delete(reqId);
    const timer = this.timers.get(reqId);
    if (timer) {
      clearTimeout(timer);
      this.timers.delete(reqId);
    }

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
    resolve(normalized);
  }

  private failAllPending(reason: string): void {
    for (const [reqId, resolve] of this.pending.entries()) {
      const timer = this.timers.get(reqId);
      if (timer) clearTimeout(timer);
      resolve({ error: reason });
    }
    this.pending.clear();
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
