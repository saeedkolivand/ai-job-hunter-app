import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import {
  EVENT_CHANNELS,
  type MenuActionEvent,
  type MenuNavigateEvent,
  type PendingMenuIntent,
} from '@ajh/shared';

import { asyncUnsub } from '../../utils.js';

export const menu = {
  onNavigate: (handler: (event: MenuNavigateEvent) => void) =>
    asyncUnsub(() =>
      listen<MenuNavigateEvent>(EVENT_CHANNELS.menu.navigate, (e) => handler(e.payload))
    ),
  onAction: (handler: (event: MenuActionEvent) => void) =>
    asyncUnsub(() =>
      listen<MenuActionEvent>(EVENT_CHANNELS.menu.action, (e) => handler(e.payload))
    ),
  takePending: () => invoke<PendingMenuIntent | null>('menu_take_pending'),
};
