import { Bell, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef } from 'react';
import { useRouter } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { Button, EmptyState, transition } from '@ajh/ui';

import { resolveNotificationRoute } from '@/lib/notification-route';
import { timeAgo } from '@/lib/time';
import {
  useClearAllNotifications,
  useMarkAllNotificationsRead,
  useMarkNotificationRead,
  useNotifications,
  useRemoveNotification,
} from '@/services';
import { useUiStore } from '@/store/ui-store';

export function NotificationBell() {
  const { t, i18n } = useTranslation();
  const router = useRouter();

  const open = useUiStore((s) => s.notificationsOpen);
  const setOpen = useUiStore((s) => s.setNotificationsOpen);

  const { data } = useNotifications();
  const items = data ?? [];
  const unreadCount = items.filter((n) => !n.read).length;
  // Copy before sorting so the React Query cache array is never mutated.
  const sorted = [...items].sort((a, b) => b.createdAt - a.createdAt);

  const markRead = useMarkNotificationRead();
  const markAllRead = useMarkAllNotificationsRead();
  const removeNotification = useRemoveNotification();
  const clearAll = useClearAllNotifications();

  const containerRef = useRef<HTMLDivElement>(null);

  // Close on Escape / outside-click. DOM-listener effect only (not remote data);
  // attaches while the dropdown is open and tears down on close/unmount.
  useEffect(() => {
    if (!open) return;
    const onMouseDown = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onMouseDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('mousedown', onMouseDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [open, setOpen]);

  return (
    <div ref={containerRef} className="relative">
      <Button
        variant="ghost"
        size="sm"
        className={`app-no-drag relative border-transparent bg-transparent hover:border-transparent hover:bg-white/[0.06] ${
          unreadCount > 0
            ? 'text-brand hover:text-brand'
            : 'text-foreground/70 hover:text-foreground'
        }`}
        aria-label={t('notifications.bell.aria')}
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <Bell size={16} />
        {unreadCount > 0 && (
          <span
            className="absolute -right-0.5 -top-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-brand px-1 text-[10px] font-semibold leading-none text-action-foreground"
            aria-label={t('notifications.unread.aria', { count: unreadCount })}
          >
            {unreadCount > 9 ? '9+' : unreadCount}
          </span>
        )}
      </Button>

      <AnimatePresence>
        {open && (
          <motion.div
            className="app-no-drag glass-dropdown absolute right-0 top-full mt-2 w-80 rounded-xl"
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={transition.fast}
          >
            <div className="flex items-center justify-between gap-2 border-b border-white/10 px-3 py-2">
              <span className="text-sm font-medium text-foreground">
                {t('notifications.title')}
              </span>
              <div className="flex items-center gap-1">
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={unreadCount === 0}
                  onClick={() => markAllRead.mutate()}
                >
                  {t('notifications.markAllRead')}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={items.length === 0}
                  onClick={() => clearAll.mutate()}
                >
                  {t('notifications.clearAll')}
                </Button>
              </div>
            </div>

            {sorted.length === 0 ? (
              <EmptyState icon={Bell} title={t('notifications.empty')} />
            ) : (
              <div className="max-h-96 overflow-y-auto">
                {sorted.map((n) => {
                  const openNotification = () => {
                    markRead.mutate(n.id);
                    setOpen(false);
                    if (n.route) {
                      const validatedTo = resolveNotificationRoute(n.route.to);
                      void router.navigate({
                        to: validatedTo,
                        search: validatedTo === n.route.to ? n.route.search : undefined,
                      });
                    }
                  };
                  return (
                    // A clickable row carries a nested remove `Button`; using a
                    // `<button>` row would nest interactive elements (invalid DOM),
                    // so it follows the repo's `role="button"` row pattern instead.
                    <div
                      key={n.id}
                      role="button"
                      tabIndex={0}
                      onClick={openNotification}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault();
                          openNotification();
                        }
                      }}
                      className="flex w-full items-start gap-2.5 px-3 py-2.5 text-left transition-colors hover:bg-white/5 focus-visible:bg-white/5 focus-visible:outline-none"
                    >
                      {n.read ? (
                        <span className="mt-1.5 h-2 w-2 shrink-0" />
                      ) : (
                        <span
                          className="mt-1.5 h-2 w-2 shrink-0 rounded-full bg-brand"
                          aria-label={t('notifications.unread.dotAria')}
                        />
                      )}
                      <div className="min-w-0 flex-1">
                        <p className="text-sm font-medium text-foreground">{n.title}</p>
                        <p className="line-clamp-2 text-xs text-foreground/60">{n.body}</p>
                        <p className="mt-0.5 text-[11px] text-foreground/40">
                          {timeAgo(n.createdAt, Date.now(), i18n.language)}
                        </p>
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        aria-label={t('notifications.remove.aria')}
                        onClick={(e) => {
                          e.stopPropagation();
                          removeNotification.mutate(n.id);
                        }}
                      >
                        <X size={14} />
                      </Button>
                    </div>
                  );
                })}
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
