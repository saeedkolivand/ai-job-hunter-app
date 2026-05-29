import { invoke } from '@tauri-apps/api/core';

import type { ApplyStartRequest } from '@ajh/shared/schemas';

export const apply = {
  start: (req: ApplyStartRequest) => invoke('apply_start', { req }),
  catalog: () => invoke('apply_catalog'),
};
