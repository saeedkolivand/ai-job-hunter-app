import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { ChangelogResult, UpdateCheckResult } from '@ajh/shared';

import { asyncUnsub } from '../../utils.js';

export const updater = {
  check: () => invoke<UpdateCheckResult>('updater_check'),
  download: () => invoke('updater_download'),
  install: () => invoke('updater_install'),
  changelog: () => invoke<ChangelogResult>('updater_changelog'),
  onStatus: (handler: (status: unknown) => void) =>
    asyncUnsub(() => listen<unknown>('updater:status', (e) => handler(e.payload))),
};
