import { listen } from '@tauri-apps/api/event';

import { asyncUnsub } from '../../utils.js';

export interface MenuNavigateEvent {
  route: string;
  section: string | null;
}

export interface MenuActionEvent {
  action: 'check-updates' | 'shortcuts';
}

export const menu = {
  onNavigate: (handler: (event: MenuNavigateEvent) => void) =>
    asyncUnsub(() => listen<MenuNavigateEvent>('menu.navigate', (e) => handler(e.payload))),
  onAction: (handler: (event: MenuActionEvent) => void) =>
    asyncUnsub(() => listen<MenuActionEvent>('menu.action', (e) => handler(e.payload))),
};
