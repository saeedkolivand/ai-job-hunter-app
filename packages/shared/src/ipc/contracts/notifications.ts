import type { AppNotification } from '../../types/index.js';

/**
 * Notification Center capability (Phase 2). The read/mutate seam over the
 * persisted Rust `NotificationStore`. Every mutator resolves once the store has
 * persisted; the renderer keeps its inbox live via `onChanged`. `clicked` is the
 * unified OS-banner / tray click target (focuses the window + opens the inbox via
 * `onOpenInbox`).
 */
export interface NotificationsContract {
  list(): Promise<AppNotification[]>;
  markRead(id: string): Promise<void>;
  markAllRead(): Promise<void>;
  remove(id: string): Promise<void>;
  clearAll(): Promise<void>;
  /** Invokes `notifications_clicked` — focuses the window and opens the inbox. */
  clicked(): Promise<void>;
  /** Subscribe to list changes (push / read / remove / clear). Sync unsubscribe. */
  onChanged(handler: () => void): () => void;
  /** Subscribe to the "open inbox" signal (OS-banner / tray click). Sync unsubscribe. */
  onOpenInbox(handler: () => void): () => void;
}

/**
 * Event channel names emitted by the notification commands. `changed` fires on
 * every mutation; `open` is the OS-banner / tray click "open the inbox" signal.
 */
export const NOTIFICATIONS_CHANNELS = {
  changed: 'notifications:changed',
  open: 'notifications:open',
} as const;
