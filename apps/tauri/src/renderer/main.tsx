import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClientProvider } from '@tanstack/react-query';
import { createRouter, RouterProvider } from '@tanstack/react-router';

import { restoreTheme } from '@ajh/ui';

import { AppSplash } from '@/components/layout/AppSplash';
import { AppClientProvider } from '@/providers/AppClientProvider';
import { PerformanceModeProvider } from '@/providers/PerformanceModeProvider';

import { routeTree } from './routeTree.gen';
import { queryClient } from './services/query-client';

import './i18n';
import './styles/globals.css';

restoreTheme();

const router = createRouter({
  routeTree,
  defaultPreload: 'intent',
  defaultPreloadStaleTime: 0,
});

// Ensure the app starts at the dashboard
void router.navigate({ to: '/', replace: true });

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <AppClientProvider>
      <PerformanceModeProvider>
        <QueryClientProvider client={queryClient}>
          <AppSplash />
          <RouterProvider router={router} />
        </QueryClientProvider>
      </PerformanceModeProvider>
    </AppClientProvider>
  </React.StrictMode>
);
