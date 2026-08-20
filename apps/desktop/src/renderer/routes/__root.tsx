import { AnimatePresence, motion, MotionConfig } from 'motion/react';
import { useEffect, useRef } from 'react';
import {
  createRootRoute,
  type ErrorComponentProps,
  type NavigateOptions,
  Outlet,
  useNavigate,
  useRouter,
} from '@tanstack/react-router';

import type { NotificationToast } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, ErrorState, NotificationProvider, transition, useNotification } from '@ajh/ui';

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
import { useWindowTaskbarSync } from '@/hooks/use-window-taskbar-sync';
import { readOnboardingComplete } from '@/lib/onboarding-mirror';
import { installUnknownPathRedirect } from '@/lib/router-guard';
import { useAppClient } from '@/providers/AppClientProvider';
import { CapabilityProvider } from '@/providers/CapabilityProvider';
import {
  useAccentEvents,
  useApplicationEvents,
  useAutoIndex,
  useExtensionBridgeEvents,
  useNotificationEvents,
  useSyncCloseToTray,
  useSyncSalaryExpectation,
  useSyncSemanticScoring,
} from '@/services';
import {
  useOnboardingCompleted,
  usePreferencesStore,
  useSidebarCollapsed,
} from '@/store/preferences-store';

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

/** Keeps the embedding index current when `autoIndexOnUpload` is on — covers a
 *  fresh import, an embedding provider/model change, and leftovers from a
 *  previous session. Mounted once here for the same reason the event bridges
 *  are: a per-route mount would re-run the check on every navigation. */
function AutoIndexBridge() {
  useAutoIndex();
  return null;
}

/** Keeps the extension-bridge connection pill live: invalidates the status
 *  query on the bridge's `extensionBridge:changed` push (a 0→1 / →0
 *  live-connection transition), so Settings flips instantly on pair/unpair
 *  instead of waiting on its 30s poll. Mounted once; the listener attaches a
 *  single time. */
function ExtensionBridgeEventsBridge() {
  useExtensionBridgeEvents();
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

/**
 * Root-level error boundary — TanStack Router walks up to this when a route
 * component throws and neither it nor any ancestor route defines its own
 * `errorComponent` (e.g. the Rules-of-Hooks crash logged 2026-08-18, which
 * white-screened the whole app because no boundary existed at all).
 *
 * This replaces the WHOLE shell, not just the failed route's content: the
 * router wraps a match's own component in the catch boundary, and the root
 * match's component IS `RootLayout` — Titlebar, Sidebar, StatusBar and every
 * always-mounted bridge go with it. So there is no sidebar left to send the
 * user to, and the copy must not promise one.
 *
 * That is also why `reset` alone is not an escape hatch. It re-attempts
 * rendering the route that threw, which recovers a transient failure but loops
 * straight back for a deterministic render bug — exactly the case this was
 * built for. The dashboard button is the real way out: `useNavigate` resolves
 * fine inside the boundary, and landing on `/` remounts the shell.
 *
 * Neither action fixes the root cause; the underlying crash is tracked
 * separately.
 *
 * Exported (not just used inline) so it can be unit-tested standalone,
 * without mounting the rest of the root layout's provider tree.
 */
export function RootErrorBoundary({ reset }: ErrorComponentProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  return (
    <ErrorState
      title={t('errorBoundary.title')}
      description={t('errorBoundary.description')}
      onRetry={reset}
      action={
        <Button
          variant="default"
          onClick={() => {
            void navigate({ to: '/' });
          }}
        >
          {t('errorBoundary.goHome')}
        </Button>
      }
      className="h-full"
    />
  );
}

function RootLayout() {
  const router = useRouter();
  const isCollapsed = useSidebarCollapsed();
  const onboardingCompleted = useOnboardingCompleted();
  const setOnboardingComplete = usePreferencesStore((s) => s.setOnboardingComplete);

  // Route to an autopilot's found-jobs when the tray/deep-link asks (app-global).
  useAutopilotFocusNavigation();
  // Push the persisted close-to-tray preference to the shell once on boot.
  useSyncCloseToTray();
  // Push the persisted salary expectation onto the backend job_preferences
  // store once on boot (Task #30) — existing users' saved value lands there.
  useSyncSalaryExpectation();
  // Push the persisted semantic-scoring preference onto its backend-readable
  // mirror once on boot (ADR-020 addendum) — the headless Autopilot scheduler
  // has no other way to read a webview-localStorage setting.
  useSyncSemanticScoring();
  // Sync taskbar progress + flash attention on job completion/failure.
  useWindowTaskbarSync();
  // One-shot: if Zustand says not completed, check the disk mirror (survives
  // webview-data clear) and hydrate the store so the wizard is skipped.
  const checkedMirrorRef = useRef(false);
  useEffect(() => {
    if (onboardingCompleted || checkedMirrorRef.current) return;
    checkedMirrorRef.current = true;
    void readOnboardingComplete().then((done) => {
      if (done) setOnboardingComplete();
    });
  }, [onboardingCompleted, setOnboardingComplete]);
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

  // `reducedMotion="user"` makes EVERY motion component in the app honour the OS
  // "reduce motion" setting (transforms are dropped, opacity still cross-fades)
  // without each call site gating itself. CSS `@media (prefers-reduced-motion)`
  // rules never reached motion's JS-driven animations.
  return (
    <MotionConfig reducedMotion="user">
      <NotificationProvider>
        <MenuNavigationBridge />
        <ApplicationEventsBridge />
        <NotificationEventsBridge />
        <AutoIndexBridge />
        <ExtensionBridgeEventsBridge />
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
    </MotionConfig>
  );
}

export const Route = createRootRoute({
  component: RootLayout,
  errorComponent: RootErrorBoundary,
  notFoundComponent: () => (
    <div className="flex h-full flex-col items-center justify-center gap-4 text-center">
      <p className="text-base font-semibold text-foreground/50">Page not found</p>
      <p className="text-sm text-foreground/30">Redirecting…</p>
    </div>
  ),
});
