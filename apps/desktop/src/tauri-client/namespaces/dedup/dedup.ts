import { invoke } from '@tauri-apps/api/core';

import type { DedupMarkNotDuplicateRequest } from '@ajh/shared/schemas';

export const dedup = {
  markNotDuplicate: (req: DedupMarkNotDuplicateRequest) =>
    invoke<{ success: boolean }>('dedup_mark_not_duplicate', { req }),
};
