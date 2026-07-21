import { invoke } from '@tauri-apps/api/core';

import type { DiscoveredCompany } from '@ajh/shared/ipc';
import type { DiscoverySearchRequest, DiscoveryStarRequest } from '@ajh/shared/schemas';

export const discovery = {
  searchCompanies: (req: DiscoverySearchRequest) =>
    invoke<DiscoveredCompany[]>('discovery_search_companies', { req }),
  // Honest union: the command resolves `{ error }` on failure (Tauri never
  // rejects a returned Value). `useSetStarred` narrows + throws.
  setStarred: (req: DiscoveryStarRequest) =>
    invoke<{ success: true } | { error: string }>('discovery_set_starred', { req }),
  watched: () => invoke<DiscoveredCompany[]>('discovery_watched'),
};
