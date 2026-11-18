import { invoke } from '@tauri-apps/api/core';

import type { AtsView } from '@ajh/shared/ipc';

export const exportNs = {
  atsView: (path: string) => invoke<AtsView>('export_ats_view', { path }),
};
