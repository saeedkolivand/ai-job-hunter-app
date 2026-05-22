/**
 * Backward-compatibility shim — all logic lives in Notification.tsx.
 * Existing callers of useToast / ToastProvider / ToastItem / ToastVariant
 * continue to work unchanged.
 */
import {
  type NotificationItem,
  NotificationProvider,
  type NotificationVariant,
  useNotification,
} from './Notification';

export type ToastVariant = NotificationVariant;
export type ToastItem = NotificationItem;

export const ToastProvider = NotificationProvider;

export function useToast() {
  return useNotification();
}
