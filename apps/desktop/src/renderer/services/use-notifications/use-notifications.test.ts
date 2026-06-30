/**
 * use-notifications service hooks — Priority 1
 *
 * Strategy:
 *  - createMockClient from test-support (proxy-based spy factory).
 *  - renderHookWithClient wraps QueryClient + AppClientProvider.
 *  - Assertions: each hook calls the right client method; mutations invalidate
 *    keys.notifications.all on success; useNotificationEvents subscribes once,
 *    wires all three channels, and unsubscribes on unmount.
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
  });

  it('subscribes to onChanged, onOpenInbox, and onOsBannerClick exactly once on mount', () => {
    const offChanged = vi.fn();
    const offOpen = vi.fn();
    const offBanner = vi.fn();
    const onChanged = vi.fn(() => offChanged);
    const onOpenInbox = vi.fn(() => offOpen);
    const onOsBannerClick = vi.fn(() => offBanner);
    const client = createMockClient({
      'notifications.onChanged': onChanged,
      'notifications.onOpenInbox': onOpenInbox,
      'notifications.onOsBannerClick': onOsBannerClick,
    });

    renderHookWithClient(() => useNotificationEvents(), { client });

    expect(onChanged).toHaveBeenCalledTimes(1);
    expect(onOpenInbox).toHaveBeenCalledTimes(1);
    expect(onOsBannerClick).toHaveBeenCalledTimes(1);
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

  it('calling the onOsBannerClick handler calls api.notifications.clicked()', async () => {
    let bannerHandler: (() => void) | null = null;
    const onOsBannerClick = vi.fn((cb: () => void) => {
      bannerHandler = cb;
      return () => {};
    });
    const clicked = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({
      'notifications.onOsBannerClick': onOsBannerClick,
      'notifications.clicked': clicked,
    });

    renderHookWithClient(() => useNotificationEvents(), { client });

    await act(async () => {
      bannerHandler?.();
      await Promise.resolve();
    });

    expect(clicked).toHaveBeenCalledTimes(1);
  });

  it('unsubscribes all three listeners on unmount', () => {
    const offChanged = vi.fn();
    const offOpen = vi.fn();
    const offBanner = vi.fn();
    const client = createMockClient({
      'notifications.onChanged': vi.fn(() => offChanged),
      'notifications.onOpenInbox': vi.fn(() => offOpen),
      'notifications.onOsBannerClick': vi.fn(() => offBanner),
    });

    const { unmount } = renderHookWithClient(() => useNotificationEvents(), { client });
    unmount();

    expect(offChanged).toHaveBeenCalledTimes(1);
    expect(offOpen).toHaveBeenCalledTimes(1);
    expect(offBanner).toHaveBeenCalledTimes(1);
  });

  it('does NOT re-subscribe on re-render (subscribe-once discipline)', () => {
    const onChanged = vi.fn(() => () => {});
    const onOpenInbox = vi.fn(() => () => {});
    const onOsBannerClick = vi.fn(() => () => {});
    const client = createMockClient({
      'notifications.onChanged': onChanged,
      'notifications.onOpenInbox': onOpenInbox,
      'notifications.onOsBannerClick': onOsBannerClick,
    });

    const { rerender } = renderHookWithClient(() => useNotificationEvents(), { client });

    rerender();
    rerender();

    // Effect deps are [api, qc] — both stable; listeners must register exactly once.
    expect(onChanged).toHaveBeenCalledTimes(1);
    expect(onOpenInbox).toHaveBeenCalledTimes(1);
    expect(onOsBannerClick).toHaveBeenCalledTimes(1);
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
    const unsubBanner = vi.fn();
    const onChanged = vi.fn(() => unsubChanged);
    const onOpenInbox = vi.fn(() => unsubOpen);
    const onOsBannerClick = vi.fn(() => unsubBanner);

    const client = createMockClient({
      'notifications.onChanged': onChanged,
      'notifications.onOpenInbox': onOpenInbox,
      'notifications.onOsBannerClick': onOsBannerClick,
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
    const netBanner = onOsBannerClick.mock.calls.length - unsubBanner.mock.calls.length;
    expect(netChanged).toBe(1);
    expect(netOpen).toBe(1);
    expect(netBanner).toBe(1);

    unmount();

    // After unmount: every listener is unsubscribed — subscribe count == unsubscribe count.
    expect(onChanged.mock.calls.length).toBe(unsubChanged.mock.calls.length);
    expect(onOpenInbox.mock.calls.length).toBe(unsubOpen.mock.calls.length);
    expect(onOsBannerClick.mock.calls.length).toBe(unsubBanner.mock.calls.length);
  });
});
