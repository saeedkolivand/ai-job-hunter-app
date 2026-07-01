import { listen } from '@tauri-apps/api/event';

import { EVENT_CHANNELS } from '@ajh/shared';

import { asyncUnsub } from '../../utils.js';

export const shortcuts = {
  onCommandPalette: (handler: () => void) =>
    asyncUnsub(() => listen(EVENT_CHANNELS.shortcuts.commandPalette, () => handler())),
};
