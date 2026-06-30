import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import { createMockClient, withProviders } from '@/test-support';

import { useAutopilotFocusNavigation } from './use-autopilot-focus-navigation';

// ── Mocks ─────────────────────────────────────────────────────────────────────

const navigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigate,
}));

const setAutopilot = vi.fn();
vi.mock('@/store/session-store', () => ({
  useSessionStore: (selector: (s: { setAutopilot: typeof setAutopilot }) => unknown) =>
    selector({ setAutopilot }),
}));

// Capture the live-event handler registered by useAutopilotFocusEvents so tests
// can fire it directly (simulates the shell's autopilot:focus emit).
let capturedFocusHandler: ((event: { autopilotId: string }) => void) | undefined;
vi.mock('@/services', () => ({
  keys: { autopilot: { all: ['autopilot'] } },
  useAutopilotFocusEvents: (handler: (event: { autopilotId: string }) => void) => {
    capturedFocusHandler = handler;
  },
}));

// useQueryClient — return a stub with invalidateQueries as a spy.
const invalidateQueries = vi.fn();
vi.mock('@tanstack/react-query', async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>();
  return {
    ...actual,
    useQueryClient: () => ({ invalidateQueries }),
  };
});

// ── Helpers ───────────────────────────────────────────────────────────────────

/**
 * Render the hook with a mock client whose `autopilot.takePendingFocus` resolves
 * to `pending`. On mount the hook drains once; tests can also override for later
 * drain triggers.
 *
 * Override shape: `createMockClient` from `@/test-support` accepts dotted-key
 * overrides (`'namespace.method': fn`) via a Proxy — NOT nested objects. The
 * Proxy `get` trap checks `overrides['autopilot.takePendingFocus']` directly so
 * the override IS applied; call assertions below prove it each time.
 */
function renderWithPending(
  pending: string | null,
  takePendingFocus = vi.fn().mockResolvedValue(pending)
) {
  const client = createMockClient({ 'autopilot.takePendingFocus': takePendingFocus });
  const utils = renderHook(() => useAutopilotFocusNavigation(), {
    wrapper: withProviders(client),
  });
  return { ...utils, takePendingFocus };
}

beforeEach(() => {
  navigate.mockClear();
  setAutopilot.mockClear();
  invalidateQueries.mockClear();
  capturedFocusHandler = undefined;
});

afterEach(() => {
  vi.clearAllMocks();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('useAutopilotFocusNavigation — pull drain (buffered deep-link path)', () => {
  it('navigates to /autopilot and sets focusedId when a buffered id is pulled on mount', async () => {
    const { takePendingFocus } = renderWithPending('ap-123');

    // Prove the override was actually called (not a default-null path passing by luck).
    await waitFor(() => expect(takePendingFocus).toHaveBeenCalledOnce());
    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/autopilot' }));
    expect(setAutopilot).toHaveBeenCalledExactlyOnceWith({ focusedId: 'ap-123' });
    expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: ['autopilot'] });
  });

  it('does nothing when the pull returns null (common case — nothing buffered)', async () => {
    const { takePendingFocus } = renderWithPending(null);

    await waitFor(() => expect(takePendingFocus).toHaveBeenCalled());
    expect(navigate).not.toHaveBeenCalled();
    expect(setAutopilot).not.toHaveBeenCalled();
  });

  it('drains exactly once even if a later window-focus trigger sees a cleared buffer', async () => {
    // First pull (mount) returns the id; subsequent pulls see null (cleared).
    const takePendingFocus = vi.fn().mockResolvedValueOnce('ap-once').mockResolvedValue(null);
    renderWithPending(null, takePendingFocus);

    // Prove the override fn fired (not a default-null path) and produced navigation.
    await waitFor(() => expect(takePendingFocus).toHaveBeenCalledOnce());
    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/autopilot' }));
    expect(setAutopilot).toHaveBeenCalledWith({ focusedId: 'ap-once' });

    await act(async () => {
      window.dispatchEvent(new Event('focus'));
      document.dispatchEvent(new Event('visibilitychange'));
      await Promise.resolve();
    });

    // Still exactly one navigation — the buffer was cleared after the first pull.
    expect(navigate).toHaveBeenCalledTimes(1);
  });

  it('drains on window focus (tray/close-to-tray restore path)', async () => {
    // Mount drain returns null; a later buffer arrives.
    const takePendingFocus = vi.fn().mockResolvedValue(null);
    renderWithPending(null, takePendingFocus);

    // Mount drain called the override (not the default) and correctly did nothing.
    await waitFor(() => expect(takePendingFocus).toHaveBeenCalledOnce());
    expect(navigate).not.toHaveBeenCalled();

    takePendingFocus.mockResolvedValueOnce('ap-focus');
    await act(async () => {
      window.dispatchEvent(new Event('focus'));
      await Promise.resolve();
    });

    // Override called a second time (window focus trigger) and produced navigation.
    await waitFor(() => expect(takePendingFocus).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/autopilot' }));
    expect(setAutopilot).toHaveBeenCalledWith({ focusedId: 'ap-focus' });
  });

  it('drains on visibilitychange to visible (hidden→shown restore path)', async () => {
    const takePendingFocus = vi.fn().mockResolvedValue(null);
    renderWithPending(null, takePendingFocus);

    // Mount drain: override called, nothing buffered.
    await waitFor(() => expect(takePendingFocus).toHaveBeenCalledOnce());
    expect(navigate).not.toHaveBeenCalled();

    takePendingFocus.mockResolvedValueOnce('ap-vis');
    // jsdom defaults visibilityState to 'visible', so we simulate the transition.
    Object.defineProperty(document, 'visibilityState', {
      value: 'visible',
      configurable: true,
    });
    await act(async () => {
      document.dispatchEvent(new Event('visibilitychange'));
      await Promise.resolve();
    });

    // Override called a second time (visibility trigger) and drove navigation.
    await waitFor(() => expect(takePendingFocus).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/autopilot' }));
    expect(setAutopilot).toHaveBeenCalledWith({ focusedId: 'ap-vis' });
  });

  it('silently swallows IPC errors (no unhandled rejection)', async () => {
    const takePendingFocus = vi.fn().mockRejectedValue(new Error('IPC failure'));
    // Should not throw — the hook catches transient failures.
    expect(() => renderWithPending(null, takePendingFocus)).not.toThrow();
    // Let the rejected promise settle.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(navigate).not.toHaveBeenCalled();
  });
});

describe('useAutopilotFocusNavigation — live event path', () => {
  it('handles a live autopilot:focus event with an id', async () => {
    renderWithPending(null);

    await waitFor(() => expect(capturedFocusHandler).toBeDefined());

    act(() => {
      capturedFocusHandler?.({ autopilotId: 'ap-live' });
    });

    expect(navigate).toHaveBeenCalledWith({ to: '/autopilot' });
    expect(setAutopilot).toHaveBeenCalledWith({ focusedId: 'ap-live' });
    expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: ['autopilot'] });
  });

  it('only invalidates (no navigation) for a live event with an empty autopilotId', async () => {
    renderWithPending(null);

    await waitFor(() => expect(capturedFocusHandler).toBeDefined());

    act(() => {
      capturedFocusHandler?.({ autopilotId: '' });
    });

    expect(invalidateQueries).toHaveBeenCalled();
    expect(navigate).not.toHaveBeenCalled();
    expect(setAutopilot).not.toHaveBeenCalled();
  });
});
