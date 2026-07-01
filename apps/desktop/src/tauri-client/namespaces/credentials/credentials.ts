import { invoke } from '@tauri-apps/api/core';

export const credentials = {
  available: () => invoke('credentials_available'),
};
