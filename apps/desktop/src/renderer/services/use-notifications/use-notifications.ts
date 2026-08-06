import { useEffect, useRef } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useRouter } from '@tanstack/react-router';

import type { AppNotification } from '@ajh/shared';

import { resolveNotificationRoute } from '@/lib/notification-route';
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
  // Same subscribe-once discipline as the setter above: the router identity is
  // stable in practice, but keeping it behind a ref means the effect can never
  // re-subscribe because of it.
  const router = useRouter();
  const routerRef = useRef(router);
  routerRef.current = router;
  useEffect(() => {
    const offChanged = api.notifications.onChanged(() => {
      void qc.invalidateQueries({ queryKey: keys.notifications.all });
    });
    // Covers BOTH the tray click and an OS-banner body click: the backend now
    // handles the banner natively (`show_clickable_banner`) and emits
    // `notifications:open` itself, so there is no renderer-side banner
    // subscription any more. The old one went through the notification plugin's
    // `onAction`, which is mobile-only — it failed with "Command not found" on
    // every desktop startup and no banner click ever reached the app.
    //
    // With a `route` (an OS banner for one specific notification) go straight
    // there — clicking "Autopilot X found 3 jobs" should land on THAT autopilot,
    // not on a list the user then has to search. Without one (tray click) fall
    // back to opening the inbox. Same validate-then-navigate rule the bell's own
    // rows use, so an unknown backend route can't break navigation.
    const offOpen = api.notifications.onOpenInbox((payload) => {
      const route = payload?.route;
      if (!route) {
        setOpenRef.current(true);
        return;
      }
      const validatedTo = resolveNotificationRoute(route.to);
      void routerRef.current.navigate({
        to: validatedTo,
        search: validatedTo === route.to ? route.search : undefined,
      });
    });
    return () => {
      offChanged();
      offOpen();
    };
  }, [api, qc]);
};
