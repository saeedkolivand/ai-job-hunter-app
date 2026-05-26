import { useEffect } from 'react';
import { createRootRoute, Outlet, useRouter } from '@tanstack/react-router';

import { NotificationProvider } from '@ajh/ui';

import { CinematicBackground } from '@/components/background/CinematicBackground';
import { Sidebar } from '@/components/layout/Sidebar';
import { StatusBar } from '@/components/layout/StatusBar';
import { Titlebar } from '@/components/layout/Titlebar';
import { UpdateBanner } from '@/components/ui/UpdateBanner';
import { OnboardingWizard } from '@/features/onboarding/OnboardingWizard';
import { CapabilityProvider } from '@/providers/CapabilityProvider';

function RootLayout() {
  const router = useRouter();
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

  // Ensure Ctrl/Cmd+A selects all text in focused inputs and textareas.
  // WebView2 on Windows does not wire this natively; adding it here fixes
  // the pattern where Ctrl+A then Delete/Backspace fails to clear text.
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== 'a') return;
      const el = document.activeElement;
      if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
        e.preventDefault();
        el.select();
      }
    };
    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, []);

  // Redirect unknown paths to home instead of showing a blank screen.
  useEffect(() => {
    return router.subscribe('onResolved', ({ toLocation }) => {
      const known = (router.routesByPath as unknown as Record<string, unknown>)[
        toLocation.pathname
      ];
      if (toLocation.pathname !== '/' && !known) {
        void router.navigate({ to: '/', replace: true });
      }
    });
  }, [router]);

  return (
    <NotificationProvider>
      <CapabilityProvider>
        <div className="app-content relative flex h-screen flex-col overflow-hidden pt-3">
          <CinematicBackground />
          <Titlebar />
          <div className="flex flex-1 overflow-hidden">
            <Sidebar />
            <main className="glass-surface m-3 flex-1 overflow-hidden rounded-2xl">
              <Outlet />
            </main>
          </div>
          <StatusBar />
          <OnboardingWizard />
          <UpdateBanner />
        </div>
      </CapabilityProvider>
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
