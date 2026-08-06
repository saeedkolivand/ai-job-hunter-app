/**
 * use-notifications service hooks — Priority 1
 *
 * Strategy:
 *  - createMockClient from test-support (proxy-based spy factory).
 *  - renderHookWithClient wraps QueryClient + AppClientProvider.
 *  - Assertions: each hook calls the right client method; mutations invalidate
 *    keys.notifications.all on success; useNotificationEvents subscribes once,
 *    wires both channels, and unsubscribes on unmount.
 *  - @tanstack/react-router's useRouter is mocked (same as NotificationBell's
 *    test) so the hook can navigate without a RouterProvider.
 */
import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import {
  createMockClient,
  exerciseServiceHooks,
  makeQueryClient,
  renderHookWithClient,
  withProviders,
} from '@/test-support';

import { keys } from '../query-client';
import * as mod from './use-notifications';
import {
  useClearAllNotifications,
  useMarkAllNotificationsRead,
  useMarkNotificationRead,
  useNotificationEvents,
  useNotifications,
  useRemoveNotification,
} from './use-notifications';

const mockNavigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useRouter: () => ({ navigate: mockNavigate }),
}));

afterEach(() => vi.restoreAllMocks());

// ── Smoke ─────────────────────────────────────────────────────────────────────

describe('use-notifications service hooks smoke', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

// ── useNotifications ──────────────────────────────────────────────────────────

describe('useNotifications', () => {
  it('calls api.notifications.list() and returns the data', async () => {
    const fixture = [
      { id: 'n1', kind: 'test', title: 'T1', body: 'B1', createdAt: 1000, read: false },
    ];
    const list = vi.fn().mockResolvedValue(fixture);
    const client = createMockClient({ 'notifications.list': list });

    const { result } = renderHookWithClient(() => useNotifications(), { client });

    await waitFor(() => expect(result.current.data).toEqual(fixture));
    expect(list).toHaveBeenCalledTimes(1);
  });

  it('returns an empty array when list resolves empty', async () => {
    const list = vi.fn().mockResolvedValue([]);
    const client = createMockClient({ 'notifications.list': list });

    const { result } = renderHookWithClient(() => useNotifications(), { client });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual([]);
  });
});

// ── useMarkNotificationRead ───────────────────────────────────────────────────

describe('useMarkNotificationRead', () => {
  it('calls api.notifications.markRead with the given id', async () => {
    const markRead = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'notifications.markRead': markRead });

    const { result } = renderHookWithClient(() => useMarkNotificationRead(), { client });

    await act(async () => {
      result.current.mutate('n1');
      await Promise.resolve();
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(markRead).toHaveBeenCalledWith('n1');
  });

  it('invalidates keys.notifications.all on success', async () => {
    const markRead = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'notifications.markRead': markRead });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useMarkNotificationRead(), {
      client,
      queryClient,
    });

    await act(async () => {
      result.current.mutate('n1');
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(invalidate).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: keys.notifications.all })
    );
  });
});

// ── useMarkAllNotificationsRead ───────────────────────────────────────────────

describe('useMarkAllNotificationsRead', () => {
  it('calls api.notifications.markAllRead', async () => {
    const markAllRead = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'notifications.markAllRead': markAllRead });

    const { result } = renderHookWithClient(() => useMarkAllNotificationsRead(), { client });

    await act(async () => {
      result.current.mutate();
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(markAllRead).toHaveBeenCalledTimes(1);
  });

  it('invalidates keys.notifications.all on success', async () => {
    const markAllRead = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'notifications.markAllRead': markAllRead });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useMarkAllNotificationsRead(), {
      client,
      queryClient,
    });

    await act(async () => {
      result.current.mutate();
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(invalidate).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: keys.notifications.all })
    );
  });
});

// ── useRemoveNotification ─────────────────────────────────────────────────────

describe('useRemoveNotification', () => {
  it('calls api.notifications.remove with the given id', async () => {
    const remove = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'notifications.remove': remove });

    const { result } = renderHookWithClient(() => useRemoveNotification(), { client });

    await act(async () => {
      result.current.mutate('n2');
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(remove).toHaveBeenCalledWith('n2');
  });

  it('invalidates keys.notifications.all on success', async () => {
    const remove = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'notifications.remove': remove });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useRemoveNotification(), { client, queryClient });

    await act(async () => {
      result.current.mutate('n2');
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(invalidate).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: keys.notifications.all })
    );
  });
});

// ── useClearAllNotifications ──────────────────────────────────────────────────

describe('useClearAllNotifications', () => {
  it('calls api.notifications.clearAll', async () => {
    const clearAll = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'notifications.clearAll': clearAll });

    const { result } = renderHookWithClient(() => useClearAllNotifications(), { client });

    await act(async () => {
      result.current.mutate();
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(clearAll).toHaveBeenCalledTimes(1);
  });

  it('invalidates keys.notifications.all on success', async () => {
    const clearAll = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'notifications.clearAll': clearAll });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useClearAllNotifications(), {
      client,
      queryClient,
    });

    await act(async () => {
      result.current.mutate();
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(invalidate).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: keys.notifications.all })
    );
  });
});

// ── useNotificationEvents ─────────────────────────────────────────────────────

describe('useNotificationEvents', () => {
  // Zustand ui-store state isolation: reset between tests.
  beforeEach(async () => {
    const { useUiStore } = await import('@/store/ui-store');
    useUiStore.setState({ notificationsOpen: false });
    mockNavigate.mockClear();
  });

  it('subscribes to onChanged and onOpenInbox exactly once on mount', () => {
    // Deliberately no third (banner) subscription: the OS-banner click is
    // handled natively in Rust and arrives on `onOpenInbox` carrying a route.
    const offChanged = vi.fn();
    const offOpen = vi.fn();
    const onChanged = vi.fn(() => offChanged);
    const onOpenInbox = vi.fn(() => offOpen);
    const client = createMockClient({
      'notifications.onChanged': onChanged,
      'notifications.onOpenInbox': onOpenInbox,
    });

    renderHookWithClient(() => useNotificationEvents(), { client });

    expect(onChanged).toHaveBeenCalledTimes(1);
    expect(onOpenInbox).toHaveBeenCalledTimes(1);
  });

  it('calling the onChanged handler invalidates keys.notifications.all', async () => {
    let changedHandler: (() => void) | null = null;
    const onChanged = vi.fn((cb: () => void) => {
      changedHandler = cb;
      return () => {};
    });
    const client = createMockClient({ 'notifications.onChanged': onChanged });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    renderHookWithClient(() => useNotificationEvents(), { client, queryClient });

    await act(async () => {
      changedHandler?.();
    });

    // Use waitFor to be robust against async scheduling inside the void-invalidate call.
    await waitFor(() =>
      expect(invalidate).toHaveBeenCalledWith(
        expect.objectContaining({ queryKey: keys.notifications.all })
      )
    );
  });

  it('calling the onOpenInbox handler sets notificationsOpen to true', async () => {
    const { useUiStore } = await import('@/store/ui-store');
    let openHandler: (() => void) | null = null;
    const onOpenInbox = vi.fn((cb: () => void) => {
      openHandler = cb;
      return () => {};
    });
    const client = createMockClient({ 'notifications.onOpenInbox': onOpenInbox });

    renderHookWithClient(() => useNotificationEvents(), { client });

    expect(useUiStore.getState().notificationsOpen).toBe(false);

    await act(async () => {
      openHandler?.();
    });

    expect(useUiStore.getState().notificationsOpen).toBe(true);
  });

  it('an OS-banner payload carrying a route navigates instead of opening the inbox', async () => {
    // The reported ask: clicking "Autopilot X found 3 jobs" must land on THAT
    // autopilot. The backend puts the clicked notification's own route on the
    // payload; a routed open must NOT also pop the inbox drawer, which would be
    // a second, competing destination.
    const { useUiStore } = await import('@/store/ui-store');
    type Payload = { route?: { to: string; search?: Record<string, unknown> } };
    let openHandler: ((p: Payload) => void) | null = null;
    const onOpenInbox = vi.fn((cb: (p: Payload) => void) => {
      openHandler = cb;
      return () => {};
    });
    const client = createMockClient({ 'notifications.onOpenInbox': onOpenInbox });

    renderHookWithClient(() => useNotificationEvents(), { client });

    await act(async () => {
      openHandler?.({ route: { to: '/autopilot', search: { focus: 'ap-42' } } });
      await Promise.resolve();
    });

    // Straight to the exact autopilot, search params preserved.
    expect(mockNavigate).toHaveBeenCalledWith({
      to: '/autopilot',
      search: { focus: 'ap-42' },
    });
    // And NOT also popping the inbox drawer — that would be a second,
    // competing destination for one click.
    expect(useUiStore.getState().notificationsOpen).toBe(false);
  });

  it('an unknown backend route is survived, not thrown on', async () => {
    // `resolveNotificationRoute` maps it to the '/' fallback rather than handing
    // TanStack Router a path it does not know.
    type Payload = { route?: { to: string } };
    let openHandler: ((p: Payload) => void) | null = null;
    const onOpenInbox = vi.fn((cb: (p: Payload) => void) => {
      openHandler = cb;
      return () => {};
    });
    const client = createMockClient({ 'notifications.onOpenInbox': onOpenInbox });

    renderHookWithClient(() => useNotificationEvents(), { client });

    await act(async () => {
      openHandler?.({ route: { to: '/does-not-exist' } });
      await Promise.resolve();
    });

    // Falls back to '/', and drops the search: those params were meant for a
    // route we are no longer going to.
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/', search: undefined });
  });

  it('a payload with no route opens the inbox (the tray-click case)', async () => {
    const { useUiStore } = await import('@/store/ui-store');
    type Payload = { route?: { to: string } };
    let openHandler: ((p: Payload) => void) | null = null;
    const onOpenInbox = vi.fn((cb: (p: Payload) => void) => {
      openHandler = cb;
      return () => {};
    });
    const client = createMockClient({ 'notifications.onOpenInbox': onOpenInbox });

    renderHookWithClient(() => useNotificationEvents(), { client });

    await act(async () => {
      openHandler?.({});
      await Promise.resolve();
    });

    expect(useUiStore.getState().notificationsOpen).toBe(true);
    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it('unsubscribes both listeners on unmount', () => {
    const offChanged = vi.fn();
    const offOpen = vi.fn();
    const client = createMockClient({
      'notifications.onChanged': vi.fn(() => offChanged),
      'notifications.onOpenInbox': vi.fn(() => offOpen),
    });

    const { unmount } = renderHookWithClient(() => useNotificationEvents(), { client });
    unmount();

    expect(offChanged).toHaveBeenCalledTimes(1);
    expect(offOpen).toHaveBeenCalledTimes(1);
  });

  it('does NOT re-subscribe on re-render (subscribe-once discipline)', () => {
    const onChanged = vi.fn(() => () => {});
    const onOpenInbox = vi.fn(() => () => {});
    const client = createMockClient({
      'notifications.onChanged': onChanged,
      'notifications.onOpenInbox': onOpenInbox,
    });

    const { rerender } = renderHookWithClient(() => useNotificationEvents(), { client });

    rerender();
    rerender();

    // Effect deps are [api, qc] — both stable; listeners must register exactly
    // once. The router added for route navigation sits behind a ref for the same
    // reason, so it did not reintroduce a re-subscribe.
    expect(onChanged).toHaveBeenCalledTimes(1);
    expect(onOpenInbox).toHaveBeenCalledTimes(1);
  });

  /**
   * HIGH — StrictMode no-listener-leak test.
   *
   * React.StrictMode double-invokes effects (mount→cleanup→mount) in development.
   * The production app renders inside StrictMode (main.tsx). The `useRef`-guarded
   * subscribe-once discipline must survive that: after StrictMode's
   * mount→unmount→remount each channel must have exactly ONE net-active listener
   * (i.e. subscribe calls == unsubscribe calls + 1).  After the final unmount
   * every listener must be unsubscribed (subscribe count == unsubscribe count).
   *
   * Why we assert NET-active rather than "exactly 1 subscribe": StrictMode
   * legitimately fires the effect twice, calling subscribe twice and the returned
   * cleanup once between the two runs. The contract is no LEAK (net active = 1
   * while mounted; 0 after unmount), which is what the `useRef` guard
   * plus the `[api, qc]` dep array together guarantee.
   *
   * If the `useRef` guard or the cleanup return were removed, StrictMode would
   * produce subscribe×2 but unsubscribe×1, leaving a net-2 active listener after
   * mount and net-1 after unmount — both assertions below would then FAIL.
   */
  it('no listener leak under React.StrictMode (subscribe×N == unsubscribe×(N-1) while mounted; ==N after unmount)', () => {
    const unsubChanged = vi.fn();
    const unsubOpen = vi.fn();
    const onChanged = vi.fn(() => unsubChanged);
    const onOpenInbox = vi.fn(() => unsubOpen);

    const client = createMockClient({
      'notifications.onChanged': onChanged,
      'notifications.onOpenInbox': onOpenInbox,
    });
    const queryClient = makeQueryClient();

    // Wrap ONLY this test in StrictMode — do NOT add StrictMode to withProviders
    // or renderHookWithClient, which would skew other suites' call counts.
    const StrictWrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(
        React.StrictMode,
        null,
        React.createElement(
          ({ children: c }: { children: React.ReactNode }) =>
            withProviders(client, queryClient)({ children: c }),
          null,
          children
        )
      );

    const { unmount } = renderHook(() => useNotificationEvents(), { wrapper: StrictWrapper });

    // While mounted: each channel has exactly 1 net-active listener.
    // Net-active = subscribe calls - unsubscribe calls must equal 1.
    const netChanged = onChanged.mock.calls.length - unsubChanged.mock.calls.length;
    const netOpen = onOpenInbox.mock.calls.length - unsubOpen.mock.calls.length;
    expect(netChanged).toBe(1);
    expect(netOpen).toBe(1);

    unmount();

    // After unmount: every listener is unsubscribed — subscribe count == unsubscribe count.
    expect(onChanged.mock.calls.length).toBe(unsubChanged.mock.calls.length);
    expect(onOpenInbox.mock.calls.length).toBe(unsubOpen.mock.calls.length);
  });
});
