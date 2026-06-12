import type { AppNotification, NotificationToast } from '../../types/index.js';

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
  /**
   * Subscribe to OS-banner body clicks (the `@tauri-apps/plugin-notification`
   * `onAction`). Any click opens the inbox — wire it to `clicked()` so it focuses
   * the window and emits `notifications:open`. Sync unsubscribe.
   */
  onOsBannerClick(handler: () => void): () => void;
  /**
   * Subscribe to in-app toasts: a notification was just pushed while the window
   * was focused, so the renderer shows a transient toast (with a "View" that
   * follows the carried `route`) instead of relying on the OS banner. Sync
   * unsubscribe. See the Rust `notifications:toast` emit in `push_and_notify`.
   */
  onToast(handler: (toast: NotificationToast) => void): () => void;
}

/**
 * Request/response channel registry for the notification commands. The push
 * event names (`notifications:changed` / `:open` / `:toast`) live in the
 * centralized `EVENT_CHANNELS` registry under `packages/shared/src/events/`,
 * not here. This namespace is currently request-only and has no channels yet
 * (the read/mutate commands invoke their own Tauri command names directly).
 */
export const NOTIFICATIONS_CHANNELS = {} as const;
