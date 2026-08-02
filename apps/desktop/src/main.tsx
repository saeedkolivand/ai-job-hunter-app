/**
 * Tauri entry point.
 *
 * Supplies a TauriInvokeClient to AppClientProvider. All routes, components,
 * and service hooks live in src/renderer/ and are shared via the `@` alias.
 */
import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClientProvider } from '@tanstack/react-query';
import { createRouter, RouterProvider } from '@tanstack/react-router';

import { ErrorBoundary, restoreTheme } from '@ajh/ui';

import { registerWindowControls } from '@/lib/window-controls-registry';
import { AiConfigBoot } from '@/providers/AiConfigBoot';
import { AppClientProvider } from '@/providers/AppClientProvider';
import { PerformanceModeProvider } from '@/providers/PerformanceModeProvider';
import { routeTree } from '@/routeTree.gen';
import { queryClient } from '@/services/query-client';

import { installDesktopNativeBehaviors } from './desktop-native';
import { createTauriInvokeClient } from './tauri-client/index.js';
import { TauriWindowControls } from './TauriWindowControls';

import '@/i18n';
import './styles.css';

installDesktopNativeBehaviors();

registerWindowControls(TauriWindowControls);

restoreTheme();

const router = createRouter({
  routeTree,
  defaultPreload: 'intent',
  defaultPreloadStaleTime: 0,
});

void router.navigate({ to: '/', replace: true });

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}

const tauriClient = createTauriInvokeClient();

ReactDOM.createRoot(document.getElementById('root') as HTMLElement, {
  // React 19 routes UNCAUGHT render errors to `window.reportError` by default,
  // which raises a window `error` event — already picked up by the global
  // handlers that the injected browser SDK installs. Errors that an
  // ErrorBoundary CATCHES take a different path: React hands them to
  // `onCaughtError` and never lets them reach the window, so without this they
  // would be invisible to crash reporting. Re-reporting keeps the recovery UI
  // (the boundary still renders its fallback) while making the failure visible.
  //
  // Deliberately no hand-rolled `window.onerror` / `unhandledrejection`
  // listeners: the injected SDK already owns those, and a second listener would
  // report every error twice.
  onCaughtError: (error) => reportError(error),
}).render(
  <React.StrictMode>
    <ErrorBoundary>
      <AppClientProvider client={tauriClient}>
        <PerformanceModeProvider>
          <QueryClientProvider client={queryClient}>
            <AiConfigBoot />
            <RouterProvider router={router} />
          </QueryClientProvider>
        </PerformanceModeProvider>
      </AppClientProvider>
    </ErrorBoundary>
  </React.StrictMode>
);
