import { useEffect, useRef } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { AppNotification } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';
import { useUiStore } from '@/store/ui-store';

import { keys } from '../query-client';

export const useNotifications = () => {
  const api = useAppClient();
  return useQuery<AppNotification[]>({
    queryKey: keys.notifications.all,
    queryFn: () => api.notifications.list(),
  });
};

export const useMarkNotificationRead = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.notifications.markRead(id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.notifications.all });
    },
  });
};

export const useMarkAllNotificationsRead = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.notifications.markAllRead(),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.notifications.all });
    },
  });
};

export const useRemoveNotification = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.notifications.remove(id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.notifications.all });
    },
  });
};

export const useClearAllNotifications = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.notifications.clearAll(),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.notifications.all });
    },
  });
};

/**
 * App-global subscription to notification list changes + the "open inbox"
 * signal (OS-banner / tray click). Mounted ONCE in the root layout (like
 * `useApplicationEvents`); never call from a feature component, or the
 * listeners would attach/detach per route. The subscribe-once `useRef`
 * discipline keeps the async Tauri `listen` from racing re-subscription.
 */
export const useNotificationEvents = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  const setNotificationsOpen = useUiStore((s) => s.setNotificationsOpen);
  // Keep the latest setter in a ref so the effect subscribes ONCE.
  const setOpenRef = useRef(setNotificationsOpen);
  setOpenRef.current = setNotificationsOpen;
  useEffect(() => {
    const offChanged = api.notifications.onChanged(() => {
      void qc.invalidateQueries({ queryKey: keys.notifications.all });
    });
    const offOpen = api.notifications.onOpenInbox(() => {
      setOpenRef.current(true);
    });
    return () => {
      offChanged();
      offOpen();
    };
  }, [api, qc]);
};
