import { invoke } from '@tauri-apps/api/core';

export const apply = {
  start: (req: unknown) => invoke('apply_start', { req }),
  catalog: () => invoke('apply_catalog'),
};
