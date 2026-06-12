import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { EVENT_CHANNELS } from '@ajh/shared';

import { asyncUnsub } from '../../utils.js';

export interface MenuNavigateEvent {
  route: string;
  section: string | null;
}

export interface MenuActionEvent {
  action: 'check-updates' | 'shortcuts';
}

/** A menu intent buffered shell-side while the window was hidden, pulled by the
 *  renderer once its JS loop is live (see `takePending`). */
export type PendingMenuIntent =
  | { event: 'menu:navigate'; payload: MenuNavigateEvent }
  | { event: 'menu:action'; payload: MenuActionEvent };

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
