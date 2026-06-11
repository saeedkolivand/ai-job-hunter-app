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
          { addresstype: 'country', address: { country: 'France' } },
          { addresstype: 'road', address: { road: 'Rue de Rivoli', country: 'France' } }, // skipped — no city/town/village, not a country
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
      { display: 'Paris, France' },
      { display: 'Lyon, France' },
      { display: 'France' },
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

  it('resolves city from municipality when city/town/village absent', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => [{ address: { municipality: 'Espoo', country: 'Finland' } }],
      })
    );
    const { result, rerender } = renderHook(({ q }) => useGeocoding(q), {
      initialProps: { q: '' },
    });
    rerender({ q: 'Espoo' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(350);
    });
    expect(result.current.suggestions).toEqual([{ display: 'Espoo, Finland' }]);
  });

  it('resolves city from hamlet when city/town/village/municipality absent', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => [{ address: { hamlet: 'Little Snoring', country: 'United Kingdom' } }],
      })
    );
    const { result, rerender } = renderHook(({ q }) => useGeocoding(q), {
      initialProps: { q: '' },
    });
    rerender({ q: 'Little Snoring' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(350);
    });
    expect(result.current.suggestions).toEqual([{ display: 'Little Snoring, United Kingdom' }]);
  });

  it('deduplicates country-level results — two identical country entries produce one suggestion', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => [
          { addresstype: 'country', address: { country: 'France' } },
          { addresstype: 'country', address: { country: 'France' } },
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
    expect(result.current.suggestions).toHaveLength(1);
    expect(result.current.suggestions).toEqual([{ display: 'France' }]);
  });

  it('skips road/POI results and does not include road names in any suggestion display', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => [
          { address: { city: 'Paris', country: 'France' } },
          { addresstype: 'road', address: { road: 'Rue de Rivoli', country: 'France' } },
          { address: {} },
        ],
      })
    );
    const { result, rerender } = renderHook(({ q }) => useGeocoding(q), {
      initialProps: { q: '' },
    });
    rerender({ q: 'Rivoli' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(350);
    });
    expect(result.current.suggestions).toHaveLength(1);
    expect(result.current.suggestions).toEqual([{ display: 'Paris, France' }]);
    for (const s of result.current.suggestions) {
      expect(s.display).not.toContain('Rue de Rivoli');
    }
  });
});
