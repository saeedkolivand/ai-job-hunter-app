import { useEffect } from 'react';
import { createRootRoute, Outlet, useRouter } from '@tanstack/react-router';

import { ToastProvider } from '@ajh/ui';

import { CinematicBackground } from '@/components/background/CinematicBackground';
import { CommandPalette } from '@/components/command-palette/CommandPalette';
import { Sidebar } from '@/components/layout/Sidebar';
import { StatusBar } from '@/components/layout/StatusBar';
import { Titlebar } from '@/components/layout/Titlebar';
import { UpdateBanner } from '@/components/ui/UpdateBanner';
import { OnboardingWizard } from '@/features/onboarding/OnboardingWizard';
import { CapabilityProvider } from '@/providers/CapabilityProvider';
import { useOnboardingCompleted } from '@/store/preferences-store';

function RootLayout() {
  const router = useRouter();
  const onboardingCompleted = useOnboardingCompleted();

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
    <ToastProvider>
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
          {onboardingCompleted && <CommandPalette />}
          <OnboardingWizard />
          <UpdateBanner />
        </div>
      </CapabilityProvider>
    </ToastProvider>
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
