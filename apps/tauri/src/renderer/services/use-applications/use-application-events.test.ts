import { describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { createMockClient, makeQueryClient, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import { useApplicationEvents } from './use-applications';

describe('useApplicationEvents', () => {
  it('invalidates applications.all when the onChanged listener fires', async () => {
    // Capture the listener registered via api.applications.onChanged so we can
    // fire it manually — simulates the Tauri event arriving from the bridge.
    let capturedListener: ((event: unknown) => void) | null = null;
    const onChanged = vi.fn((cb: (event: unknown) => void) => {
      capturedListener = cb;
      return () => {};
    });

    const client = createMockClient({
      'applications.onChanged': onChanged,
    });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    renderHookWithClient(() => useApplicationEvents(), { client, queryClient });

    expect(onChanged).toHaveBeenCalledTimes(1);

    // Fire the event.
    await act(async () => {
      capturedListener?.({ id: 'app-1', action: 'created' });
      await Promise.resolve();
    });

    expect(invalidate).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: keys.applications.all })
    );
  });

  it('invalidates postings.all when the onChanged listener fires', async () => {
    let capturedListener: ((event: unknown) => void) | null = null;
    const onChanged = vi.fn((cb: (event: unknown) => void) => {
      capturedListener = cb;
      return () => {};
    });

    const client = createMockClient({
      'applications.onChanged': onChanged,
    });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    renderHookWithClient(() => useApplicationEvents(), { client, queryClient });

    await act(async () => {
      capturedListener?.({ id: 'app-2', action: 'created' });
      await Promise.resolve();
    });

    expect(invalidate).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: keys.postings.all })
    );
  });

  it('both applications.all and postings.all are invalidated in a single event', async () => {
    let capturedListener: ((event: unknown) => void) | null = null;
    const onChanged = vi.fn((cb: (event: unknown) => void) => {
      capturedListener = cb;
      return () => {};
    });

    const client = createMockClient({
      'applications.onChanged': onChanged,
    });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    renderHookWithClient(() => useApplicationEvents(), { client, queryClient });

    await act(async () => {
      capturedListener?.({ id: 'app-3', action: 'imported' });
      await Promise.resolve();
    });

    const calledKeys = invalidate.mock.calls.map((c) => (c[0] as { queryKey: unknown }).queryKey);
    expect(calledKeys).toContainEqual(keys.applications.all);
    expect(calledKeys).toContainEqual(keys.postings.all);
    // Exactly two invalidations per event — no extra side-effects.
    expect(invalidate).toHaveBeenCalledTimes(2);
  });

  it('forwards the event to the optional onChanged callback', async () => {
    let capturedListener: ((event: unknown) => void) | null = null;
    const onChanged = vi.fn((cb: (event: unknown) => void) => {
      capturedListener = cb;
      return () => {};
    });

    const client = createMockClient({
      'applications.onChanged': onChanged,
    });
    const externalHandler = vi.fn();
    const queryClient = makeQueryClient();

    renderHookWithClient(() => useApplicationEvents(externalHandler), { client, queryClient });

    const event = { id: 'app-4', action: 'updated' };
    await act(async () => {
      capturedListener?.(event);
      await Promise.resolve();
    });

    expect(externalHandler).toHaveBeenCalledWith(event);
  });

  it('unsubscribes on unmount so no stale listener leaks', () => {
    const off = vi.fn();
    const onChanged = vi.fn(() => off);

    const client = createMockClient({
      'applications.onChanged': onChanged,
    });
    const { unmount } = renderHookWithClient(() => useApplicationEvents(), { client });

    unmount();

    expect(off).toHaveBeenCalledTimes(1);
  });
});
