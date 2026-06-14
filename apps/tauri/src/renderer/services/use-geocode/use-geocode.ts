import { useCallback } from 'react';

import type { GeocodeSuggestion } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

/**
 * Location autocomplete suggestions.
 *
 * Ports & adapters: the location input wants an on-demand `(query) => suggestions`
 * callback (debounced + cancelled by the input itself), not a cached query — so
 * this hook returns a stable function rather than a React Query result. It keeps
 * the renderer off `api.geocode.*` directly (ESLint ports rule).
 */
export const useGeocodeSuggest = (): ((query: string) => Promise<GeocodeSuggestion[]>) => {
  const api = useAppClient();
  return useCallback((query: string) => api.geocode.suggest(query), [api]);
};
