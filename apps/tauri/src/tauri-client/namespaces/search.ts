import { invoke } from '@tauri-apps/api/core';

export const search = {
  hybrid: (req: unknown) => invoke('search_hybrid', { req }),
};
