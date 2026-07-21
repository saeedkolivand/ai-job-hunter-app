import { invoke } from '@tauri-apps/api/core';

import type { DedupMarkNotDuplicateRequest } from '@ajh/shared/schemas';

export const dedup = {
  // Honest union: the command resolves `{ error }` on failure (Tauri never
  // rejects a returned Value). `useMarkNotDuplicate` narrows + throws.
  markNotDuplicate: (req: DedupMarkNotDuplicateRequest) =>
    invoke<{ success: true } | { error: string }>('dedup_mark_not_duplicate', { req }),
};
