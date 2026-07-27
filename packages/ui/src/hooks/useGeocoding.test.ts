import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useGeocoding } from './useGeocoding';

describe('useGeocoding', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('does not fetch for queries shorter than 2 characters', () => {
    const fetcher = vi.fn().mockResolvedValue([{ display: 'X' }]);
    const { result } = renderHook(() => useGeocoding('a', fetcher));
    expect(result.current.suggestions).toEqual([]);
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('debounces and stores suggestions from the provided fetcher', async () => {
    const fetcher = vi.fn().mockResolvedValue([{ display: 'Berlin, Germany' }]);
    const { result, rerender } = renderHook(({ q }) => useGeocoding(q, fetcher), {
      initialProps: { q: '' },
    });
    rerender({ q: 'Berlin' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(350);
    });
    expect(fetcher).toHaveBeenCalledWith('Berlin');
    expect(result.current.suggestions).toEqual([{ display: 'Berlin, Germany' }]);
    expect(result.current.activeIndex).toBe(-1);
  });

  it('ignores a slow superseded response that resolves after a newer one', async () => {
    // The 300ms debounce only bounds when a request STARTS. Resume typing after
    // a pause and two are in flight; if the older (slower) one wins the race it
    // used to overwrite the list with suggestions for a stale query.
    const fetcher = vi.fn((q: string) =>
      q === 'Be'
        ? new Promise((resolve) =>
            setTimeout(() => resolve([{ display: 'Belgrade, Serbia' }]), 2000)
          )
        : Promise.resolve([{ display: 'Berlin, Germany' }])
    ) as unknown as (q: string) => Promise<{ display: string }[]>;

    const { result, rerender } = renderHook(({ q }) => useGeocoding(q, fetcher), {
      initialProps: { q: '' },
    });

    rerender({ q: 'Be' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(350); // fires the slow 'Be' fetch
    });

    rerender({ q: 'Berlin' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(350); // fires + resolves the fast 'Berlin' fetch
    });
    expect(result.current.suggestions).toEqual([{ display: 'Berlin, Germany' }]);

    // Now let the stale 'Be' request land.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(result.current.suggestions).toEqual([{ display: 'Berlin, Germany' }]);
  });

  it('ignores an in-flight response after the query drops below 2 characters', async () => {
    // The short-query branch returns early, but React still runs the previous
    // cleanup first — so the superseded request must be flagged there too,
    // otherwise it repopulates a list the hook just cleared.
    const fetcher = vi.fn(
      () =>
        new Promise((resolve) => setTimeout(() => resolve([{ display: 'Berlin, Germany' }]), 2000))
    ) as unknown as (q: string) => Promise<{ display: string }[]>;

    const { result, rerender } = renderHook(({ q }) => useGeocoding(q, fetcher), {
      initialProps: { q: '' },
    });

    rerender({ q: 'Berlin' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(350);
    });

    rerender({ q: 'B' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(result.current.suggestions).toEqual([]);
  });

  it('never fetches on its own — the hook has no built-in geocoder', async () => {
    // Regression pin for the retired `defaultFetch`: the hook used to call
    // Nominatim directly when no fetcher was passed, which shipped live in the
    // published Storybook. `onFetchSuggestions` is now required, so the only
    // traffic is the caller's — asserted here by proving `fetch` is untouched.
    const globalFetch = vi.fn();
    vi.stubGlobal('fetch', globalFetch);
    const fetcher = vi.fn().mockResolvedValue([{ display: 'Berlin, Germany' }]);

    const { rerender } = renderHook(({ q }) => useGeocoding(q, fetcher), {
      initialProps: { q: '' },
    });
    rerender({ q: 'Berlin' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(350);
    });

    expect(fetcher).toHaveBeenCalledWith('Berlin');
    expect(globalFetch).not.toHaveBeenCalled();
  });
});
