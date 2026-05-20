import { Outlet, createRootRoute } from '@tanstack/react-router';
import { CinematicBackground } from '@/components/background/CinematicBackground';
import { Titlebar } from '@/components/layout/Titlebar';
import { Sidebar } from '@/components/layout/Sidebar';
import { StatusBar } from '@/components/layout/StatusBar';
import { CommandPalette } from '@/components/command-palette/CommandPalette';
import { CapabilityProvider } from '@/providers/CapabilityProvider';
import { OnboardingWizard } from '@/features/onboarding/OnboardingWizard';
import { ToastProvider } from '@/components/ui/Toast';
import { UpdateBanner } from '@/components/ui/UpdateBanner';

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
          <UpdateBanner />
        </div>
      </CapabilityProvider>
    </ToastProvider>
  ),
});
