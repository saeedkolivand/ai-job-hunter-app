import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { EVENT_CHANNELS, type NotificationOpen, type NotificationToast } from '@ajh/shared';

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
    asyncUnsub(() => listen(EVENT_CHANNELS.notifications.changed, () => handler())),
  // OS-banner / tray click "open the inbox" signal — see `notifications_clicked`
  // and `show_clickable_banner`. The payload carries the clicked banner's own
  // route (absent for a tray click).
  onOpenInbox: (handler: (payload: NotificationOpen) => void) =>
    asyncUnsub(() =>
      listen<NotificationOpen>(EVENT_CHANNELS.notifications.open, (e) => handler(e.payload))
    ),
  // In-app toast for a just-pushed notification (window focused) — see the Rust
  // `push_and_notify` `notifications:toast` emit.
  onToast: (handler: (toast: NotificationToast) => void) =>
    asyncUnsub(() =>
      listen<NotificationToast>(EVENT_CHANNELS.notifications.toast, (e) => handler(e.payload))
    ),
};
