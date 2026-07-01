/**
 * Browser E2E entry point.
 *
 * Mirrors main.tsx but swaps the Tauri transport for the in-memory
 * {@link createMockClient}, so the full renderer (routes, sidebar, features)
 * boots in a plain browser with no backend. Used only by Playwright via
 * `e2e.html`; never part of the production Tauri build.
 */
import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClientProvider } from '@tanstack/react-query';
import { createRouter, RouterProvider } from '@tanstack/react-router';

import { restoreTheme } from '@ajh/ui';

import { createMockClient } from '@/lib/mock-client';
import { registerWindowControls } from '@/lib/window-controls-registry';
import { AppClientProvider } from '@/providers/AppClientProvider';
import { PerformanceModeProvider } from '@/providers/PerformanceModeProvider';
import { routeTree } from '@/routeTree.gen';
import { queryClient } from '@/services/query-client';

import '@/i18n';
import './styles.css';

// No native window chrome in the browser harness.
registerWindowControls(() => null);

restoreTheme();

const router = createRouter({
  routeTree,
  defaultPreload: 'intent',
  defaultPreloadStaleTime: 0,
});

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}

// The harness loads at /e2e.html, which matches no route — send the router to
// the dashboard like the Tauri entry does.
void router.navigate({ to: '/', replace: true });

const client = createMockClient();

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <AppClientProvider client={client}>
      <PerformanceModeProvider>
        <QueryClientProvider client={queryClient}>
          <RouterProvider router={router} />
        </QueryClientProvider>
      </PerformanceModeProvider>
    </AppClientProvider>
  </React.StrictMode>
);
