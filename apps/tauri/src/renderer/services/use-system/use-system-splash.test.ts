/**
 * Tests for the native splash-screen renderer hooks:
 *   - useSyncThemeMirror: calls setThemeMirror on boot, re-calls on
 *     data-color-scheme attribute change, and cleans up on unmount.
 *   - appReady (AppReadyBridge logic): fires once on mount, swallows rejection.
 */
import { useEffect, useRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { useAppClient } from '@/providers/AppClientProvider';
import { createMockClient, renderHookWithClient } from '@/test-support';

import { useSyncThemeMirror } from './use-system';

// ---------------------------------------------------------------------------
// MutationObserver stub
//
// The module-level `observerCallback` approach breaks because React itself
// creates MutationObservers — its constructor calls overwrite the captured var.
// Instead we collect ALL instances keyed by their callback, then locate ours
// by checking which instance's callback wraps api.system.setThemeMirror.
// ---------------------------------------------------------------------------
type ObserverEntry = {
  cb: MutationCallback;
  observeArgs: [Node, MutationObserverInit?][];
  disconnected: boolean;
  fire: () => void;
};

let instances: ObserverEntry[] = [];
const globalDisconnect = vi.fn();
const globalObserve = vi.fn();

beforeEach(() => {
  instances = [];
  globalDisconnect.mockClear();
  globalObserve.mockClear();

  vi.stubGlobal(
    'MutationObserver',
    class {
      private entry: ObserverEntry;
      constructor(cb: MutationCallback) {
        const entry: ObserverEntry = {
          cb,
          observeArgs: [],
          disconnected: false,
          fire: () => cb([] as MutationRecord[], null as unknown as MutationObserver),
        };
        this.entry = entry;
        instances.push(entry);
      }
      observe(target: Node, opts?: MutationObserverInit) {
        this.entry.observeArgs.push([target, opts]);
        globalObserve(target, opts);
      }
      disconnect() {
        this.entry.disconnected = true;
        globalDisconnect();
      }
    }
  );

  document.documentElement.dataset.colorScheme = 'dark';
});

afterEach(() => {
  vi.unstubAllGlobals();
  delete document.documentElement.dataset.colorScheme;
});

/**
 * Find the ObserverEntry whose observe() was called with the
 * `data-color-scheme` attributeFilter — that's the one useSyncThemeMirror owns.
 */
function findOurObserver(): ObserverEntry | undefined {
  return instances.find((inst) =>
    inst.observeArgs.some(
      ([, opts]) =>
        opts?.attributes === true &&
        Array.isArray(opts.attributeFilter) &&
        opts.attributeFilter.includes('data-color-scheme')
    )
  );
}

/**
 * Trigger useSyncThemeMirror's observer callback (simulating applyTheme
 * writing a new value to data-color-scheme on <html>).
 */
function simulateSchemeChange(scheme: 'light' | 'dark') {
  document.documentElement.dataset.colorScheme = scheme;
  const entry = findOurObserver();
  if (entry) {
    act(() => {
      entry.fire();
    });
  }
}

// ---------------------------------------------------------------------------
// useSyncThemeMirror
// ---------------------------------------------------------------------------
describe('useSyncThemeMirror', () => {
  it('calls setThemeMirror with data-color-scheme on mount (boot path)', async () => {
    document.documentElement.dataset.colorScheme = 'light';
    const setThemeMirror = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'system.setThemeMirror': setThemeMirror });

    renderHookWithClient(() => useSyncThemeMirror(), { client });

    await waitFor(() => expect(setThemeMirror).toHaveBeenCalledWith('light'));
    // Every call during mount must be 'light' — no wrong scheme pushed.
    for (const [arg] of setThemeMirror.mock.calls) {
      expect(arg).toBe('light');
    }
  });

  it('defaults to resolved system scheme when data-color-scheme is absent', async () => {
    delete document.documentElement.dataset.colorScheme;
    const setThemeMirror = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'system.setThemeMirror': setThemeMirror });

    renderHookWithClient(() => useSyncThemeMirror(), { client });

    await waitFor(() =>
      expect(setThemeMirror).toHaveBeenCalledWith(expect.stringMatching(/^light|dark$/))
    );
  });

  it('re-calls setThemeMirror when data-color-scheme changes', async () => {
    document.documentElement.dataset.colorScheme = 'dark';
    const setThemeMirror = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'system.setThemeMirror': setThemeMirror });

    renderHookWithClient(() => useSyncThemeMirror(), { client });
    await waitFor(() => expect(setThemeMirror).toHaveBeenCalledWith('dark'));

    const callsBefore = setThemeMirror.mock.calls.length;
    simulateSchemeChange('light');

    await waitFor(() => expect(setThemeMirror).toHaveBeenCalledWith('light'));
    expect(setThemeMirror.mock.calls.length).toBeGreaterThan(callsBefore);
  });

  it('wires a MutationObserver with the data-color-scheme attributeFilter', async () => {
    document.documentElement.dataset.colorScheme = 'dark';
    const setThemeMirror = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'system.setThemeMirror': setThemeMirror });

    renderHookWithClient(() => useSyncThemeMirror(), { client });
    await waitFor(() => expect(setThemeMirror).toHaveBeenCalled());

    const entry = findOurObserver();
    expect(entry).toBeDefined();
    const lastObserveOpts = entry?.observeArgs.at(-1)?.[1];
    expect(lastObserveOpts).toEqual({
      attributes: true,
      attributeFilter: ['data-color-scheme'],
    });
  });

  it('disconnects the observer on unmount', async () => {
    document.documentElement.dataset.colorScheme = 'dark';
    const setThemeMirror = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'system.setThemeMirror': setThemeMirror });

    const { unmount } = renderHookWithClient(() => useSyncThemeMirror(), { client });
    await waitFor(() => expect(setThemeMirror).toHaveBeenCalled());

    // After unmount the observer we own must be disconnected.
    unmount();

    // The entry is the last one that observed with our attributeFilter.
    const entry = findOurObserver();
    expect(entry).toBeDefined();
    expect(entry?.disconnected).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// appReady — inline hook mirrors the AppReadyBridge component logic
// ---------------------------------------------------------------------------
function useAppReadyBridge() {
  const api = useAppClient();
  const fired = useRef(false);
  useEffect(() => {
    if (fired.current) return;
    fired.current = true;
    void api.system.appReady().catch(() => {});
  }, [api]);
}

describe('appReady (AppReadyBridge logic)', () => {
  it('calls appReady on mount and does not re-fire on subsequent re-renders', async () => {
    const appReady = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'system.appReady': appReady });

    const { rerender } = renderHookWithClient(() => useAppReadyBridge(), { client });
    await waitFor(() => expect(appReady).toHaveBeenCalled());

    const callsAfterMount = appReady.mock.calls.length;
    rerender();
    rerender();

    // Re-renders must not fire appReady again — the ref guard is per-instance.
    expect(appReady).toHaveBeenCalledTimes(callsAfterMount);
  });

  it('swallows rejection — does not surface an error (Rust 10s backstop)', async () => {
    const appReady = vi.fn().mockRejectedValue(new Error('shell not ready'));
    const client = createMockClient({ 'system.appReady': appReady });

    expect(() => {
      renderHookWithClient(() => useAppReadyBridge(), { client });
    }).not.toThrow();

    await waitFor(() => expect(appReady).toHaveBeenCalled());
  });
});
