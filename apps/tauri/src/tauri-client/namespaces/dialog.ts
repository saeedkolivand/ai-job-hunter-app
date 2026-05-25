import { invoke } from '@tauri-apps/api/core';

export const dialog = {
  openFiles: (opts?: Record<string, unknown>) => invoke('dialog_open_files', opts ?? undefined),
};
