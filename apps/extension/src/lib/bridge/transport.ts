/**
 * The two transports behind one {@link BridgeTransport} seam — native messaging
 * (preferred) and the loopback WebSocket (fallback) — plus how each is opened.
 */

import { type Browser, browser } from '@wxt-dev/browser';

import type { ExtensionEnvelope } from '@ajh/shared/extension-protocol';

import { HOST_NAME, OPEN_TIMEOUT_MS, PORT_END, PORT_START, READY_TIMEOUT_MS } from './constants';

/**
 * One open connection to the desktop bridge. `onMessage` delivers a PARSED
 * object — ws JSON.parses the string frame, native messaging already auto-parses
 * the JSON for us.
 */
export interface BridgeTransport {
  send(envelope: ExtensionEnvelope): void;
  onMessage(cb: (env: unknown) => void): void;
  onClose(cb: () => void): void;
  close(): void;
}

export class WebSocketTransport implements BridgeTransport {
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
export const NATIVE_UNAVAILABLE = 'native_unavailable'; // host not registered → fall back to ws
export const NATIVE_APP_DOWN = 'native_app_down'; // host ran, app is down → app_not_running, no ws

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

export class NativeMessagingTransport implements BridgeTransport {
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

/**
 * Connect via native messaging, resolving only after `bridge.ready{ok:true}`.
 * Rejects with {@link NATIVE_APP_DOWN} on `ok:false` (host reachable, app down)
 * or {@link NATIVE_UNAVAILABLE} on pre-ready disconnect / ready-timeout (host not
 * registered → caller falls back to ws). THROWS SYNCHRONOUSLY if `connectNative`
 * itself fails so the caller can start the ws probe in the same tick (the ws
 * reconnect test asserts the probe fires synchronously after the timer).
 */
export function connectNative(): Promise<NativeMessagingTransport> {
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

/** Open one port with a timeout; resolve the socket or null on failure. */
function tryOpen(port: number): Promise<WebSocket | null> {
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

/**
 * Try each loopback port in order; resolve the first socket that reaches OPEN.
 * Even though we connect to the FIRST port that answers, the pairing token is
 * NEVER sent — the extension proves knowledge of it via HMAC and verifies the
 * desktop's own `serverProof` before sending ANY import/profile frame (see the
 * auth handshake). See the "Threat model" section in apps/extension/README.md.
 */
export async function probePorts(): Promise<{ socket: WebSocket; port: number } | null> {
  for (let port = PORT_START; port <= PORT_END; port += 1) {
    const socket = await tryOpen(port);
    if (socket) return { socket, port };
  }
  return null;
}
