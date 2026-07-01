import { invoke } from '@tauri-apps/api/core';

import type { GeocodeSuggestion } from '@ajh/shared';

export const geocode = {
  suggest: (query: string) => invoke('geocode_suggest', { query }) as Promise<GeocodeSuggestion[]>,
};
