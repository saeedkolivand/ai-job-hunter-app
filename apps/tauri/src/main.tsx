/**
 * Tauri spike entry point.
 *
 * This file mirrors apps/desktop/src/renderer/main.tsx but supplies a
 * TauriInvokeClient to AppClientProvider instead of the Electron IPC client.
 * Everything else — routes, components, service hooks — is imported from the
 * desktop renderer source via the `@` alias in vite.config.ts.
 */
import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClientProvider } from '@tanstack/react-query';
import { createRouter, RouterProvider } from '@tanstack/react-router';

import { restoreTheme } from '@ajh/ui';

import { AppClientProvider } from '@/providers/AppClientProvider';
import { PerformanceModeProvider } from '@/providers/PerformanceModeProvider';
import { routeTree } from '@/routeTree.gen';
import { queryClient } from '@/services/query-client';

import { createTauriInvokeClient } from './tauri-client';
import { TauriWindowControls } from './TauriWindowControls';
import { registerWindowControls } from '@/lib/window-controls-registry';

import '@/i18n';
import './styles.css';

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
          <RouterProvider router={router} />
        </QueryClientProvider>
      </PerformanceModeProvider>
    </AppClientProvider>
  </React.StrictMode>
);
