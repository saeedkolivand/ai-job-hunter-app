import type { ReactNode } from 'react';

export type NotificationVariant = 'success' | 'error' | 'info' | 'warning';

/** All six corner/edge placements. */
export type NotificationPlacement =
  'top' | 'topLeft' | 'topRight' | 'bottom' | 'bottomLeft' | 'bottomRight';

/** Open a notification. `duration` is in SECONDS; `0` keeps it open until dismissed. */
export interface NotificationConfig {
  /** Title line (bold). */
  message: ReactNode;
  /** Optional secondary line. */
  description?: ReactNode;
  variant?: NotificationVariant;
  /** Seconds before auto-dismiss. `0` = sticky. Default `4.5`. */
  duration?: number;
  /** Corner/edge to render in. Default `topRight`. */
  placement?: NotificationPlacement;
  /** Custom action area rendered under the text (e.g. a button). */
  btn?: ReactNode;
  /** Override the variant icon. Pass `null` to hide it. */
  icon?: ReactNode;
  /** Stable id — opening with an existing key UPDATES that notification (and
   *  resets its timer) instead of stacking a duplicate. Auto-generated if absent. */
  key?: string;
  /** Show the close button. Default `true`. */
  closable?: boolean;
  /** Pause the auto-dismiss timer while hovered. Default `true`. */
  pauseOnHover?: boolean;
  onClose?: () => void;
}

/** Imperative API returned by {@link useNotification}. */
export interface NotificationApi {
  open: (config: NotificationConfig) => string;
  success: (config: Omit<NotificationConfig, 'variant'>) => string;
  error: (config: Omit<NotificationConfig, 'variant'>) => string;
  info: (config: Omit<NotificationConfig, 'variant'>) => string;
  warning: (config: Omit<NotificationConfig, 'variant'>) => string;
  /** Dismiss one notification by key, or all when called with no key. */
  destroy: (key?: string) => void;
}

export interface NotificationItem {
  key: string;
  message: ReactNode;
  description?: ReactNode;
  variant: NotificationVariant;
  duration: number;
  placement: NotificationPlacement;
  btn?: ReactNode;
  icon?: ReactNode;
  closable: boolean;
  pauseOnHover: boolean;
  onClose?: () => void;
}
