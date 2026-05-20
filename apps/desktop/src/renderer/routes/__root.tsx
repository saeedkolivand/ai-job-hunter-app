import { createRootRoute, Outlet } from '@tanstack/react-router';

import { ToastProvider } from '@ajh/ui';

import { CinematicBackground } from '@/components/background/CinematicBackground';
import { CommandPalette } from '@/components/command-palette/CommandPalette';
import { Sidebar } from '@/components/layout/Sidebar';
import { StatusBar } from '@/components/layout/StatusBar';
import { Titlebar } from '@/components/layout/Titlebar';
import { OnboardingWizard } from '@/features/onboarding/OnboardingWizard';
import { CapabilityProvider } from '@/providers/CapabilityProvider';

export const Route = createRootRoute({
  component: () => (
    <ToastProvider>
      <CapabilityProvider>
        <div className="relative flex h-screen flex-col overflow-hidden pt-3">
          <CinematicBackground />
          <Titlebar />
          <div className="flex flex-1 overflow-hidden">
            <Sidebar />
            <main className="glass-surface m-3 flex-1 overflow-hidden rounded-2xl">
              <Outlet />
            </main>
          </div>
          <StatusBar />
          <CommandPalette />
          <OnboardingWizard />
        </div>
      </CapabilityProvider>
    </ToastProvider>
  ),
});
