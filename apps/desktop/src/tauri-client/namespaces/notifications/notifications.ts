import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { onAction } from '@tauri-apps/plugin-notification';

import { EVENT_CHANNELS, type NotificationToast } from '@ajh/shared';

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
  // OS-banner / tray click "open the inbox" signal — see `notifications_clicked`.
  onOpenInbox: (handler: () => void) =>
    asyncUnsub(() => listen(EVENT_CHANNELS.notifications.open, () => handler())),
  // OS-banner body click (`@tauri-apps/plugin-notification` `onAction`). The
  // payload is unused — any click opens the inbox. Wire the handler to
  // `clicked()` (focuses the window + emits `notifications:open`).
  onOsBannerClick: (handler: () => void) =>
    asyncUnsub(() =>
      onAction(() => handler()).then((listener) => () => void listener.unregister())
    ),
  // In-app toast for a just-pushed notification (window focused) — see the Rust
  // `push_and_notify` `notifications:toast` emit.
  onToast: (handler: (toast: NotificationToast) => void) =>
    asyncUnsub(() =>
      listen<NotificationToast>(EVENT_CHANNELS.notifications.toast, (e) => handler(e.payload))
    ),
};
