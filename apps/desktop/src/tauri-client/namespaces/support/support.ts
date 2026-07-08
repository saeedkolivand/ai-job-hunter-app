import { invoke } from '@tauri-apps/api/core';

export const support = {
  exportDiagnostics: (dest: string) =>
    invoke<{ success: true; path: string } | { success: false; error: string }>(
      'support_export_diagnostics',
      { dest }
    ),
};
