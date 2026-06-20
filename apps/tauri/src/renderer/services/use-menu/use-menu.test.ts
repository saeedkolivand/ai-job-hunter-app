import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { MenuActionEvent, MenuNavigateEvent, PendingMenuIntent } from '@ajh/shared';

import { createMockClient, withProviders } from '@/test-support';

import { useMenuIntents } from './use-menu';

// ── Helpers ───────────────────────────────────────────────────────────────────

const NAV_INTENT: PendingMenuIntent = {
  event: 'menu:navigate',
  payload: { route: '/jobs', section: null } satisfies MenuNavigateEvent,
};

const ACTION_INTENT: PendingMenuIntent = {
  event: 'menu:action',
  payload: { action: 'check-updates' } satisfies MenuActionEvent,
};

/**
 * Render the hook with a mock client whose `menu.takePending` is controlled by
 * the caller. `onNavigate` and `onAction` are passed through so assertions can
 * be made on them.
 *
 * The `onNavigate` / `onAction` event-listener registrars (`menu.onNavigate`,
 * `menu.onAction`) default to no-op unsub stubs matching the `createMockClient`
 * default for `on*` methods — they return `() => {}`.
 *
 * Pass `enablePoll: true` to activate the 250 ms backstop (macOS behaviour).
 * Defaults to `true` so existing poll-backstop tests remain unchanged.
 */
function setup(takePending: () => Promise<PendingMenuIntent | null>, enablePoll = true) {
  const onNavigate = vi.fn();
  const onAction = vi.fn();
  const client = createMockClient({ 'menu.takePending': takePending });
  const utils = renderHook(() => useMenuIntents(onNavigate, onAction, enablePoll), {
    wrapper: withProviders(client),
  });
  return { ...utils, onNavigate, onAction, takePending };
}

// ── visibilityState descriptor management ────────────────────────────────────

let originalVisibilityDescriptor: PropertyDescriptor | undefined;

function setVisibilityState(value: DocumentVisibilityState) {
  Object.defineProperty(document, 'visibilityState', {
    configurable: true,
    get: () => value,
  });
}

function restoreVisibilityState() {
  if (originalVisibilityDescriptor) {
    Object.defineProperty(document, 'visibilityState', originalVisibilityDescriptor);
  }
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

beforeEach(() => {
  // Capture the original visibilityState descriptor before any test overrides it.
  originalVisibilityDescriptor = Object.getOwnPropertyDescriptor(document, 'visibilityState');
  // Fake timers control setInterval / clearInterval used by the poll backstop.
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  // Restore visibilityState — Object.defineProperty overrides are NOT undone by
  // vi.restoreAllMocks(), so we must restore manually.
  restoreVisibilityState();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('useMenuIntents — poll backstop', () => {
  it('drains a buffered navigate intent via poll when no focus/visibility events fire', async () => {
    // mount → null so mount-drain is a no-op, first poll tick → intent, rest → null.
    const takePending = vi
      .fn<() => Promise<PendingMenuIntent | null>>()
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(NAV_INTENT)
      .mockResolvedValue(null);

    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    setVisibilityState('visible');

    const { onNavigate } = setup(takePending);

    // Let mount-drain microtasks settle (async timer advance drains microtask
    // queue between callbacks — zero advance just flushes pending microtasks).
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(onNavigate).not.toHaveBeenCalled();

    // Advance past the 250 ms poll interval — the poll fires, drains the intent.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(260);
    });

    expect(onNavigate).toHaveBeenCalledExactlyOnceWith(NAV_INTENT.payload);
  });

  it('does not re-fire onNavigate on subsequent poll ticks after buffer is empty', async () => {
    // mount → null, first poll tick → intent, all later ticks → null.
    const takePending = vi
      .fn<() => Promise<PendingMenuIntent | null>>()
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(NAV_INTENT)
      .mockResolvedValue(null);

    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    setVisibilityState('visible');

    const { onNavigate } = setup(takePending);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    // First tick delivers the intent.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(260);
    });

    expect(onNavigate).toHaveBeenCalledTimes(1);

    // Several more ticks — buffer is empty, onNavigate must not be called again.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250 * 5);
    });

    expect(onNavigate).toHaveBeenCalledTimes(1);
  });

  it('does not invoke takePending via poll when document is not focused', async () => {
    // Poll gate: `visibilityState === 'visible' && hasFocus()`.
    // hasFocus=false → poll body skipped entirely.
    vi.spyOn(document, 'hasFocus').mockReturnValue(false);
    setVisibilityState('visible');

    const takePending = vi
      .fn<() => Promise<PendingMenuIntent | null>>()
      .mockResolvedValueOnce(null) // mount drain → empty
      .mockResolvedValue(NAV_INTENT); // all poll ticks → intent (must never reach)

    const { onNavigate } = setup(takePending);

    // Allow mount-drain to settle.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    const callsAfterMount = takePending.mock.calls.length;

    // Advance several poll intervals — poll gate (hasFocus=false) must suppress all.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250 * 4);
    });

    expect(takePending).toHaveBeenCalledTimes(callsAfterMount);
    expect(onNavigate).not.toHaveBeenCalled();
  });

  it('does not invoke takePending via poll when document is not visible', async () => {
    // Same as above but gate fails on visibilityState rather than hasFocus.
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    setVisibilityState('hidden');

    const takePending = vi
      .fn<() => Promise<PendingMenuIntent | null>>()
      .mockResolvedValueOnce(null) // mount drain → empty
      .mockResolvedValue(NAV_INTENT); // all poll ticks → intent (must never reach)

    const { onNavigate } = setup(takePending);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    const callsAfterMount = takePending.mock.calls.length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(250 * 4);
    });

    expect(takePending).toHaveBeenCalledTimes(callsAfterMount);
    expect(onNavigate).not.toHaveBeenCalled();
  });

  it('drains on the next poll tick after focus is restored', async () => {
    let focused = false;
    vi.spyOn(document, 'hasFocus').mockImplementation(() => focused);
    setVisibilityState('visible');

    const takePending = vi
      .fn<() => Promise<PendingMenuIntent | null>>()
      .mockResolvedValueOnce(null) // mount drain
      .mockResolvedValueOnce(NAV_INTENT) // first focused poll tick
      .mockResolvedValue(null);

    const { onNavigate } = setup(takePending);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    // Poll while unfocused — must stay silent.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });

    expect(onNavigate).not.toHaveBeenCalled();

    // Restore focus — next poll tick must drain.
    focused = true;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(260);
    });

    expect(onNavigate).toHaveBeenCalledExactlyOnceWith(NAV_INTENT.payload);
  });

  it('stops polling after unmount and does not call onNavigate or takePending again', async () => {
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    setVisibilityState('visible');

    // Spy on setInterval to capture the handle returned for the 250 ms poll.
    // We wrap the real (fake-timer) implementation so advanceTimersByTimeAsync
    // still drives ticks normally in this and every other test.
    const realSetInterval = window.setInterval.bind(window);
    let pollHandle: number | undefined;
    const setIntervalSpy = vi
      .spyOn(window, 'setInterval')
      .mockImplementation((handler: TimerHandler, delay?: number, ...args: unknown[]) => {
        const id = realSetInterval(handler, delay, ...args);
        // Capture only the 250 ms poll registered by the hook.
        if (delay === 250) pollHandle = id;
        return id;
      });

    const clearIntervalSpy = vi.spyOn(window, 'clearInterval');

    const takePending = vi.fn<() => Promise<PendingMenuIntent | null>>().mockResolvedValue(null);
    const { onNavigate, unmount } = setup(takePending);

    // Let mount-drain settle.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    // Confirm the interval is running — advance one tick.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(260);
    });

    const callsBeforeUnmount = takePending.mock.calls.length;

    // Unmount triggers cleanup: clearInterval(poll) + cancelled = true.
    unmount();

    // Assert clearInterval was called with the exact handle the hook registered,
    // not just any clearInterval call React/jsdom may emit during teardown.
    expect(pollHandle).toBeDefined();
    expect(clearIntervalSpy).toHaveBeenCalledWith(pollHandle);

    setIntervalSpy.mockRestore();

    // Advance several more ticks — interval must be gone.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250 * 5);
    });

    expect(takePending).toHaveBeenCalledTimes(callsBeforeUnmount);
    expect(onNavigate).not.toHaveBeenCalled();
  });

  it('routes a buffered menu:action intent to onAction, not onNavigate', async () => {
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    setVisibilityState('visible');

    const takePending = vi
      .fn<() => Promise<PendingMenuIntent | null>>()
      .mockResolvedValueOnce(null) // mount drain
      .mockResolvedValueOnce(ACTION_INTENT) // first poll tick
      .mockResolvedValue(null);

    const { onNavigate, onAction } = setup(takePending);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(260);
    });

    expect(onAction).toHaveBeenCalledExactlyOnceWith(ACTION_INTENT.payload);
    expect(onNavigate).not.toHaveBeenCalled();
  });

  it('swallows IPC rejection, keeps polling, and delivers on recovery', async () => {
    // HIGH 2: takePending rejects on the first poll tick (simulating a transient
    // IPC failure). The try/catch in drain() must swallow it — no unhandled
    // rejection, no throw. The interval must survive and deliver on the next tick.
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    setVisibilityState('visible');

    const takePending = vi
      .fn<() => Promise<PendingMenuIntent | null>>()
      .mockResolvedValueOnce(null) // mount drain → empty
      .mockRejectedValueOnce(new Error('ipc failed')) // first poll tick → rejects
      .mockRejectedValueOnce(new Error('ipc failed again')) // second poll tick → rejects
      .mockResolvedValueOnce(NAV_INTENT) // third poll tick → recovers
      .mockResolvedValue(null);

    const { onNavigate, onAction } = setup(takePending);

    // Mount-drain settles.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(onNavigate).not.toHaveBeenCalled();
    expect(onAction).not.toHaveBeenCalled();

    // First poll tick — rejects; try/catch swallows; no callbacks fired.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(260);
    });

    expect(onNavigate).not.toHaveBeenCalled();
    expect(onAction).not.toHaveBeenCalled();

    // Second poll tick — rejects again; still no callbacks.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(260);
    });

    expect(onNavigate).not.toHaveBeenCalled();

    // Third poll tick — resolves with NAV_INTENT; intent delivered.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(260);
    });

    expect(onNavigate).toHaveBeenCalledExactlyOnceWith(NAV_INTENT.payload);
    expect(onAction).not.toHaveBeenCalled();

    // Confirm takePending was called on all ticks (poll survived the rejections).
    // mount drain (1) + tick1 reject (1) + tick2 reject (1) + tick3 resolve (1) = 4.
    expect(takePending).toHaveBeenCalledTimes(4);
  });

  it('does not create the interval when enablePoll is false (Windows/Linux path)', async () => {
    // With enablePoll=false the setInterval must never be called for the 250ms
    // backstop. The only takePending call should be the mount-drain.
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    setVisibilityState('visible');

    const setIntervalSpy = vi.spyOn(window, 'setInterval');

    const takePending = vi
      .fn<() => Promise<PendingMenuIntent | null>>()
      .mockResolvedValueOnce(null) // mount drain → empty
      .mockResolvedValue(NAV_INTENT); // would be delivered by poll — must not reach

    const { onNavigate, onAction } = setup(takePending, false);

    // Settle mount-drain.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    const mountDrainCalls = takePending.mock.calls.length;

    // Advance several poll intervals — no interval was created, so only the
    // mount-drain call should have fired.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250 * 5);
    });

    expect(takePending).toHaveBeenCalledTimes(mountDrainCalls);
    expect(onNavigate).not.toHaveBeenCalled();
    expect(onAction).not.toHaveBeenCalled();
    // Confirm the hook never registered a 250 ms interval.
    expect(setIntervalSpy).not.toHaveBeenCalledWith(expect.any(Function), 250);

    setIntervalSpy.mockRestore();
  });
});
