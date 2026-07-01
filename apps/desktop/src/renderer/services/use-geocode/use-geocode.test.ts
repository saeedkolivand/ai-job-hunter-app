import { describe, expect, it, vi } from 'vitest';

import { createMockClient, renderHookWithClient } from '@/test-support';

import { useGeocodeSuggest } from './use-geocode';

describe('use-geocode services', () => {
  it('useGeocodeSuggest returns a callable function', () => {
    const { result } = renderHookWithClient(() => useGeocodeSuggest());
    expect(typeof result.current).toBe('function');
  });

  it('delegates to api.geocode.suggest with the given query', async () => {
    const suggest = vi.fn().mockResolvedValue([{ label: 'Berlin, Germany', value: 'Berlin' }]);
    const client = createMockClient({ 'geocode.suggest': suggest });

    const { result } = renderHookWithClient(() => useGeocodeSuggest(), { client });
    await result.current('Berlin');

    expect(suggest).toHaveBeenCalledWith('Berlin');
  });
});
