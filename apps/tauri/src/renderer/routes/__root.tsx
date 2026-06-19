import { PanelLeft } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef } from 'react';
import {
  createRootRoute,
  type NavigateOptions,
  Outlet,
  useNavigate,
  useRouter,
} from '@tanstack/react-router';

import type { NotificationToast } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, NotificationProvider, transition, useNotification } from '@ajh/ui';

import { CinematicBackground } from '@/components/background/CinematicBackground';
import { ProtocolVersionGate } from '@/components/layout/ProtocolVersionGate';
import { ShortcutsOverlay } from '@/components/layout/ShortcutsOverlay';
import { Sidebar } from '@/components/layout/Sidebar';
import { StatusBar } from '@/components/layout/StatusBar';
import { Titlebar } from '@/components/layout/Titlebar';
import { UpdateBanner } from '@/components/ui/UpdateBanner';
import { OnboardingWizard } from '@/features/onboarding/OnboardingWizard';
import { useAutopilotFocusNavigation } from '@/hooks/use-autopilot-focus-navigation';
import { useMenuNavigation } from '@/hooks/use-menu-navigation';
import { installUnknownPathRedirect } from '@/lib/router-guard';
import { useAppClient } from '@/providers/AppClientProvider';
import { CapabilityProvider } from '@/providers/CapabilityProvider';
import {
  useAccentEvents,
  useApplicationEvents,
  useNotificationEvents,
  useSyncCloseToTray,
} from '@/services';
import { useSidebarCollapsed, useToggleSidebar } from '@/store/preferences-store';

/** Drives the native-menu navigation/actions. Rendered INSIDE
 *  `NotificationProvider` so its check-for-updates feedback can raise toasts. */
function MenuNavigationBridge() {
  useMenuNavigation();
  return null;
}

/** Live-refreshes the applications + postings lists on out-of-band application
 *  changes (e.g. a browser-extension import). The user-facing toast now comes
 *  from the store-driven `NotificationToastBridge`; this only keeps the lists
 *  fresh. Mounted once (the listener attaches a single time). */
function ApplicationEventsBridge() {
  useApplicationEvents();
  return null;
}

/** Mounts the app-global notification subscriptions (list-changed + open-inbox).
 *  Rendered once inside `NotificationProvider`; the listeners attach a single time. */
function NotificationEventsBridge() {
  useNotificationEvents();
  return null;
}

/** Keeps a 'system' accent live: re-pulls + re-applies the OS accent on the
 *  shell's `system:accentChanged` push (Windows) and on window-focus refetch
 *  (macOS/fallback). Mounted once; the listener attaches a single time. */
function AccentEventsBridge() {
  useAccentEvents();
  return null;
}

/** Raises a transient in-app toast for each pushed notification (window focused),
 *  with a "View" that follows the record's carried `route`. The title/body come
 *  from the Rust-generated record — the unified source for all toasts. Rendered
 *  once inside `NotificationProvider`; the listener attaches a single time via the
 *  subscribe-once `useRef` discipline. */
function NotificationToastBridge() {
  const { t } = useTranslation();
  const api = useAppClient();
  const notify = useNotification();
  const navigate = useNavigate();

  // Keep the latest toast-raising logic in a ref so the listener subscribes ONCE.
  const handlerRef = useRef<(toast: NotificationToast) => void>(() => {});
  handlerRef.current = (toast: NotificationToast) => {
    const route = toast.route;
    notify.success({
      message: toast.title,
      description: toast.body,
      btn: route ? (
        <Button
          variant="glass"
          onClick={() => {
            // `route.to`/`route.search` are open-typed (string / unknown map) on
            // the wire; TanStack's `navigate` is strictly typed over the route
            // tree, so cast to its option shape — the value is validated by the
            // route's `validateSearch` on arrival.
            void navigate({ to: route.to, search: route.search } as NavigateOptions);
          }}
        >
          {t('notifications.toast.view')}
        </Button>
      ) : undefined,
    });
  };

  useEffect(() => {
    const off = api.notifications.onToast((toast) => handlerRef.current(toast));
    return () => off();
  }, [api]);

  return null;
}

function RootLayout() {
  const router = useRouter();
  const { t } = useTranslation();
  const isCollapsed = useSidebarCollapsed();
  const toggleSidebar = useToggleSidebar();

  // Route to an autopilot's found-jobs when the tray/deep-link asks (app-global).
  useAutopilotFocusNavigation();
  // Push the persisted close-to-tray preference to the shell once on boot.
  useSyncCloseToTray();
  useEffect(() => {
    // Prevent mouse side-buttons (back/forward, buttons 3 & 4) from triggering
    // browser history navigation which leads to unhandled routes in the SPA.
    const block = (e: MouseEvent) => {
      if (e.button === 3 || e.button === 4) {
        e.preventDefault();
        e.stopPropagation();
      }
    };
    window.addEventListener('mousedown', block, true);
    window.addEventListener('mouseup', block, true);
    window.addEventListener('click', block, true);
    return () => {
      window.removeEventListener('mousedown', block, true);
      window.removeEventListener('mouseup', block, true);
      window.removeEventListener('click', block, true);
    };
  }, []);

  // WebView2 on Windows does not natively wire Ctrl+A (select-all) in text
  // fields, and after a programmatic select() it also fails to delete the
  // full selection on Backspace/Delete. Both are patched here.
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const el = document.activeElement;
      if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement)) return;

      // Ctrl/Cmd+A → select all
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
        e.preventDefault();
        el.select();
        return;
      }

      // Backspace/Delete when everything is selected → clear via native setter
      // so React's onChange fires correctly.
      if (e.key === 'Backspace' || e.key === 'Delete') {
        const { selectionStart, selectionEnd, value } = el;
        if (selectionStart === 0 && selectionEnd === value.length && value.length > 0) {
          e.preventDefault();
          const proto =
            el instanceof HTMLInputElement
              ? HTMLInputElement.prototype
              : HTMLTextAreaElement.prototype;
          const setter = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
          setter?.call(el, '');
          el.dispatchEvent(new Event('input', { bubbles: true }));
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, []);

  // Redirect genuinely-unknown paths to home (matched dynamic/param routes are kept).
  useEffect(() => installUnknownPathRedirect(router), [router]);

  return (
    <NotificationProvider>
      <MenuNavigationBridge />
      <ApplicationEventsBridge />
      <NotificationEventsBridge />
      <AccentEventsBridge />
      <NotificationToastBridge />
      <ProtocolVersionGate>
        <CapabilityProvider>
          <div className="app-content relative flex h-screen flex-col overflow-hidden pt-3">
            <CinematicBackground />
            <Titlebar />
            <div className="flex flex-1 overflow-hidden">
              {/* Unmount (not just shrink) the sidebar when collapsed so its links
                  leave the tab order and stay out of reach of keyboard/SR users. */}
              <AnimatePresence initial={false}>
                {!isCollapsed && (
                  <motion.div
                    key="sidebar"
                    initial={{ width: 0, opacity: 0 }}
                    animate={{ width: 'auto', opacity: 1 }}
                    exit={{ width: 0, opacity: 0 }}
                    transition={transition.normal}
                    className="flex overflow-hidden"
                    style={{ flexShrink: 0 }}
                  >
                    <Sidebar />
                  </motion.div>
                )}
              </AnimatePresence>
              <div className="relative flex flex-1 overflow-hidden">
                {isCollapsed && (
                  <div className="absolute left-6 top-6 z-10">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={toggleSidebar}
                      aria-label={t('nav.expandSidebar')}
                    >
                      <PanelLeft size={16} />
                    </Button>
                  </div>
                )}
                <main className="app-main glass-surface m-3 flex-1 overflow-hidden rounded-2xl">
                  <Outlet />
                </main>
              </div>
            </div>
            <StatusBar />
            <OnboardingWizard />
            <UpdateBanner />
            <ShortcutsOverlay />
          </div>
        </CapabilityProvider>
      </ProtocolVersionGate>
    </NotificationProvider>
  );
}

export const Route = createRootRoute({
  component: RootLayout,
  notFoundComponent: () => (
    <div className="flex h-full flex-col items-center justify-center gap-4 text-center">
      <p className="text-base font-semibold text-foreground/50">Page not found</p>
      <p className="text-sm text-foreground/30">Redirecting…</p>
    </div>
  ),
});
