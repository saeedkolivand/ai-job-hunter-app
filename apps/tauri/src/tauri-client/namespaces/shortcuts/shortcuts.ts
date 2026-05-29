import { listen } from '@tauri-apps/api/event';

import { asyncUnsub } from '../../utils.js';

export const shortcuts = {
  onCommandPalette: (handler: () => void) =>
    asyncUnsub(() => listen('shortcut:command-palette', () => handler())),
};
