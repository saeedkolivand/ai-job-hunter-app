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

  it('uses the default Nominatim fetch and parses the address shape', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => [
          { address: { city: 'Paris', state: 'Île-de-France', country: 'France' } },
          { address: { town: 'Lyon', country: 'France' } },
          { address: {} }, // skipped — no city/town/village
        ],
      })
    );
    const { result, rerender } = renderHook(({ q }) => useGeocoding(q), {
      initialProps: { q: '' },
    });
    rerender({ q: 'France' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(350);
    });
    expect(result.current.suggestions).toEqual([
      { display: 'Paris, Île-de-France, France' },
      { display: 'Lyon, France' },
    ]);
  });

  it('returns no suggestions when the default fetch fails', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false }));
    const { result, rerender } = renderHook(({ q }) => useGeocoding(q), {
      initialProps: { q: '' },
    });
    rerender({ q: 'Nowhere' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(350);
    });
    expect(result.current.suggestions).toEqual([]);
  });
});
