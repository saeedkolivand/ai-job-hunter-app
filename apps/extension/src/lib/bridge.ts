/**
 * Loopback WebSocket client to the desktop bridge.
 *
 * The desktop binds `127.0.0.1` on the first free port in the range below
 * (see `apps/tauri/src-tauri/src/extension_bridge/mod.rs::PORT_RANGE`). We probe
 * that exact range, hold a SINGLE socket, send `import.request` envelopes
 * (token on every frame), and correlate replies by `reqId`.
 *
 * Lifecycle note (MV3): the background service worker can be evicted at any
 * time, tearing down this client. The background entry re-creates it on wake
 * and when the popup opens, so this class assumes it may be short-lived and
 * keeps no cross-eviction state beyond the in-flight `reqId` map (which dies
 * with the worker — callers re-issue on the fresh instance).
 */

import {
  EXTENSION_MESSAGE_TYPES,
  type ExtensionEnvelope,
  type ExtensionImportRequest,
  type ExtensionImportResult,
  ExtensionImportResultSchema,
} from '@ajh/shared';

/** Inclusive probe range — mirrors the desktop `PORT_RANGE` (47615..=47620). */
const PORT_START = 47615;
const PORT_END = 47620;

/** Per-request timeout (ms) — the desktop fetch+parse for URL mode can be slow. */
const REQUEST_TIMEOUT_MS = 30_000;

/** Backoff schedule (ms) for reconnect attempts; the last value repeats. */
const BACKOFF_MS = [500, 1_000, 2_000, 5_000, 10_000];

/** WS handshake/open timeout per port probe. */
const OPEN_TIMEOUT_MS = 1_500;

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

export class BridgeClient {
  private socket: WebSocket | null = null;
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

  /** Whether an authenticated socket is currently open. */
  isOpen(): boolean {
    return this.socket?.readyState === WebSocket.OPEN;
  }

  /**
   * Ensure a connection: if already open, no-op; otherwise probe the range.
   * Safe to call repeatedly (popup-open wake, reconnect button).
   */
  async ensureConnected(): Promise<void> {
    if (this.disposed || this.isOpen() || this.connecting) return;
    this.connecting = true;
    try {
      const socket = await this.probeRange();
      if (socket) {
        this.attach(socket);
      } else {
        this.setPhase('app_not_running');
        this.scheduleReconnect();
      }
    } finally {
      this.connecting = false;
    }
  }

  /** Tear down the socket and cancel timers (worker shutdown / manual reset). */
  dispose(): void {
    this.disposed = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    for (const t of this.timers.values()) clearTimeout(t);
    this.timers.clear();
    this.pending.clear();
    this.socket?.close();
    this.socket = null;
  }

  /**
   * Send an `import.request` and resolve with the validated `import.result`
   * payload. Rejects on no connection, timeout, or a malformed reply.
   */
  async importJob(token: string, payload: ExtensionImportRequest): Promise<ExtensionImportResult> {
    await this.ensureConnected();
    const socket = this.socket;
    if (!socket || socket.readyState !== WebSocket.OPEN) {
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
        socket.send(JSON.stringify(envelope));
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

  /** Wire an opened socket: register handlers and reset backoff. */
  private attach(socket: WebSocket): void {
    this.socket = socket;
    this.backoffIndex = 0;
    this.setPhase('connected');

    socket.addEventListener('message', (ev: MessageEvent) => {
      this.onMessage(typeof ev.data === 'string' ? ev.data : '');
    });
    socket.addEventListener('close', () => {
      this.socket = null;
      this.failAllPending('Connection to the desktop app closed.');
      if (!this.disposed) {
        this.setPhase('app_not_running');
        this.scheduleReconnect();
      }
    });
    socket.addEventListener('error', () => {
      // `close` fires after `error`; cleanup happens there.
    });
  }

  /** Parse a reply envelope, match `reqId`, validate payload, resolve caller. */
  private onMessage(raw: string): void {
    if (!raw) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      return;
    }
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

    const result = ExtensionImportResultSchema.safeParse(env.payload);
    if (!result.success) {
      resolve({ error: 'The desktop app sent a malformed import result.' });
      return;
    }
    resolve(result.data);
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
