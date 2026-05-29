import { invoke } from '@tauri-apps/api/core';

import type { HybridSearchRequest } from '@ajh/shared/schemas';

export const search = {
  hybrid: (req: HybridSearchRequest) => invoke('search_hybrid', { req }),
};
