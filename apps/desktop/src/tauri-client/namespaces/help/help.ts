import { invoke } from '@tauri-apps/api/core';

import type { HelpSearchRequest, HelpSearchResult } from '@ajh/shared/schemas';

export const help = {
  search: (req: HelpSearchRequest) => invoke<HelpSearchResult>('help_search', { req }),
};
