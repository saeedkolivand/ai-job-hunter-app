import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import { BridgeClient } from '../bridge';
import { setupFakeWebSocket, T } from './test-support';

vi.mock('@wxt-dev/browser', () => import('./browser-mock'));

type PortListener = (msg: unknown) => void;

function buildFakePort() {
  const msgListeners: PortListener[] = [];
  const discListeners: Array<() => void> = [];
  return {
    postMessage: vi.fn(),
    disconnect: vi.fn(),
    onMessage: { addListener: (cb: PortListener) => void msgListeners.push(cb) },
    onDisconnect: { addListener: (cb: () => void) => void discListeners.push(cb) },
    simulateMessage: (obj: unknown) => msgListeners.forEach((cb) => cb(obj)),
    simulateDisconnect(lastError?: { message: string }) {
      (browser.runtime as { lastError?: unknown }).lastError = lastError;
      discListeners.forEach((cb) => cb());
    },
  };
}

describe('BridgeClient – native messaging transport', () => {
  const connectNativeMock = vi.mocked(browser.runtime.connectNative);
  const fake = setupFakeWebSocket();

  /** A native port wired in as the host, with `ensureConnected()` already in flight. */
  function startNative() {
    const port = buildFakePort();
    connectNativeMock.mockReturnValue(port as never);
    const client = new BridgeClient(vi.fn());
    return { port, client, connecting: client.ensureConnected() };
  }

  beforeEach(() => {
    (browser.runtime as { lastError?: unknown }).lastError = undefined;
  });

  afterEach(() => {
    connectNativeMock.mockReset();
    // Restore the suite default (throw) for any later file.
    connectNativeMock.mockImplementation(() => {
      throw new Error('connectNative not available');
    });
  });

  /** The ws probe took over: open the first socket and expect a connected client on 47615. */
  async function expectWsFallback(client: BridgeClient, connecting: Promise<void>): Promise<void> {
    (await fake.next()).simulateOpen();
    await connecting;
    expect(client.status()).toMatchObject({ phase: 'connected', port: 47615 });
    client.dispose();
  }

  it('connects native-first on bridge.ready{ok:true} and round-trips an import via the port', async () => {
    const { port, client, connecting } = startNative();
    port.simulateMessage({ type: 'bridge.ready', ok: true });
    await connecting;

    expect(client.status().phase).toBe('connected');
    expect(fake.sockets).toHaveLength(0); // never touched ws

    const importPromise = client.importJob({ url: 'https://example.com/job/123', applied: false });
    await vi.waitFor(() => {
      expect(port.postMessage).toHaveBeenCalled();
    });
    const sent = port.postMessage.mock.calls[0]?.[0] as { type: string; reqId: string };
    expect(sent.type).toBe(T.importRequest);

    // Reply arrives as a PARSED OBJECT (native auto-parses JSON), not a string.
    port.simulateMessage({
      type: T.importResult,
      reqId: sent.reqId,
      payload: { applicationId: 'native-1', status: 'saved' },
    });
    expect(await importPromise).toEqual({ applicationId: 'native-1', status: 'saved' });
    client.dispose();
  });

  it('enters app_not_running on bridge.ready{ok:false} with NO ws fallback', async () => {
    vi.useFakeTimers();
    const { port, client, connecting } = startNative();
    port.simulateMessage({ type: 'bridge.ready', ok: false });
    await connecting;

    expect(port.disconnect).toHaveBeenCalled(); // ok:false closes the native port
    expect(client.status().phase).toBe('app_not_running');
    expect(fake.sockets).toHaveLength(0); // app down ≠ fall back to ws

    // Reconnect scheduled — advance past backoff[0]=500ms; it re-tries native.
    const callsBefore = connectNativeMock.mock.calls.length;
    vi.advanceTimersByTime(600);
    expect(connectNativeMock.mock.calls.length).toBeGreaterThan(callsBefore);
    client.dispose();
  });

  it('falls back to the ws probe when connectNative throws', async () => {
    connectNativeMock.mockImplementation(() => {
      throw new Error('host not registered');
    });
    const client = new BridgeClient(vi.fn());
    const connecting = client.ensureConnected();
    const first = await fake.next();
    expect(first.url).toBe('ws://127.0.0.1:47615');
    first.simulateOpen();
    await connecting;
    expect(client.status()).toMatchObject({ phase: 'connected', port: 47615 });
    client.dispose();
  });

  it('falls back to ws when the port disconnects (lastError) before any bridge.ready', async () => {
    const { port, client, connecting } = startNative();
    // Host not registered: onDisconnect fires with lastError, before any ready.
    port.simulateDisconnect({ message: 'Native host has exited.' });
    await expectWsFallback(client, connecting);
  });

  it('falls back to ws when no bridge.ready arrives within the ready timeout', async () => {
    vi.useFakeTimers();
    const { port, client, connecting } = startNative();
    // No ready frame — advance past READY_TIMEOUT_MS (1500ms) → fall back.
    await vi.advanceTimersByTimeAsync(1_600);
    await expectWsFallback(client, connecting);
    expect(port.disconnect).toHaveBeenCalled(); // timeout closes the native port
  });
});
