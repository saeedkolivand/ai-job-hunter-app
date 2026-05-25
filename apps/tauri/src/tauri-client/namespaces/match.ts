import { invoke } from '@tauri-apps/api/core';

export const match = {
  resume: (req: unknown) => invoke('match_resume', { req }),
};
