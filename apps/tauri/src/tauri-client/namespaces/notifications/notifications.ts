import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { NotificationToast } from '@ajh/shared';

import { asyncUnsub } from '../../utils.js';

export const notifications = {
  list: () => invoke('notifications_list'),
  markRead: (id: string) => invoke('notifications_mark_read', { id }),
  markAllRead: () => invoke('notifications_mark_all_read'),
  remove: (id: string) => invoke('notifications_remove', { id }),
  clearAll: () => invoke('notifications_clear_all'),
  clicked: () => invoke('notifications_clicked'),
  // Emitted by every mutator command — see `commands::notifications::CHANGED_EVENT`.
  onChanged: (handler: () => void) =>
    asyncUnsub(() => listen('notifications:changed', () => handler())),
  // OS-banner / tray click "open the inbox" signal — see `notifications_clicked`.
  onOpenInbox: (handler: () => void) =>
    asyncUnsub(() => listen('notifications:open', () => handler())),
  // In-app toast for a just-pushed notification (window focused) — see the Rust
  // `push_and_notify` `notifications:toast` emit.
  onToast: (handler: (toast: NotificationToast) => void) =>
    asyncUnsub(() => listen<NotificationToast>('notifications:toast', (e) => handler(e.payload))),
};
