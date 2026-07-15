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

import { restoreTheme } from '@ajh/ui';

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

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <AppClientProvider client={tauriClient}>
      <PerformanceModeProvider>
        <QueryClientProvider client={queryClient}>
          <AiConfigBoot />
          <RouterProvider router={router} />
        </QueryClientProvider>
      </PerformanceModeProvider>
    </AppClientProvider>
  </React.StrictMode>
);
