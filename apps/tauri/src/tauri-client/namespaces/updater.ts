import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { asyncUnsub } from '../utils.js';

export const updater = {
  check: () => invoke('updater_check'),
  download: () => invoke('updater_download'),
  install: () => invoke('updater_install'),
  onStatus: (handler: (status: unknown) => void) =>
    asyncUnsub(() => listen<unknown>('updater:status', (e) => handler(e.payload))),
};
