/**
 * WHY THIS TEST EXISTS — closing the gap vs. route-outlet-nesting.test.tsx
 *
 * The prior route-outlet-nesting tests proved the unknown-path guard does not
 * eat dynamic routes, but they dodged three production details that were the
 * suspected cause of the live "Back from a tailored application lands on the
 * dashboard" bug:
 *
 *   (a) They asserted on the OVERVIEW tab. This test mounts the REAL documents
 *       tab (DocumentsTab → useDocuments / useDocumentText) under a real
 *       AppClientProvider + QueryClient (via withProviders(createMockClient())),
 *       which is the heaviest sub-tree the detail page can render.
 *   (b) They built the router with ad-hoc options. This test uses the SAME
 *       options as apps/tauri/src/main.tsx — defaultPreload: 'intent',
 *       defaultPreloadStaleTime: 0 — so preload-on-intent timing matches prod.
 *   (c) Their root routes had NO notFoundComponent. The real
 *       apps/tauri/src/renderer/routes/__root.tsx DOES define one, so this root
 *       route defines a notFoundComponent too — the key structural gap, since a
 *       root-level not-found match is exactly what could trip the guard.
 *
 * NOTE: this faithfully-reproduced flow does NOT reproduce the live bug. It is a
 * REGRESSION GUARD proving the back-nav correctly returns to /applications (leaf
 * match /applications/) and the unknown-path guard does NOT redirect to the
 * dashboard. The full @/routeTree.gen pulls in too many unmockable deps
 * (Sidebar, Titlebar, every top-level route), so a faithful hand-built subtree
 * is the correct fallback.
 */
import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from '@tanstack/react-router';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';

import type { Application } from '@ajh/shared';

import { installUnknownPathRedirect } from '@/lib/router-guard';
import { createMockClient, withProviders } from '@/test-support';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/components/layout/PageShell', () => ({
  PageShell: ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
}));

vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => () => '',
}));

vi.mock('@/features/jobs/hooks/useDefaultResumeId', () => ({
  useDefaultResumeId: () => null,
}));

vi.mock('@/features/documents/components/TailorFlow', () => ({
  TailorFlow: () => <div data-testid="tailor-flow-stub" />,
}));

const APPLICATION_FIXTURE: Application = {
  id: 'abc',
  status: 'applied',
  createdAt: 1000,
  updatedAt: 1000,
  jobUrl: 'https://acme.com/job/1',
  board: 'linkedin',
  company: 'Acme Corp',
  title: 'Senior Engineer',
  candidate: 'Jane',
  answers: [],
  brief: '',
  notes: '',
  comp: '',
  jobDescription: '',
  jobSummary: '',
  contactName: '',
  contactEmail: '',
};

vi.mock('@/services', async (importOriginal) => {
  const orig = await importOriginal();
  return {
    ...(orig as object),
    useApplication: () => ({
      data: { application: APPLICATION_FIXTURE, events: [] },
      isLoading: false,
      isError: false,
    }),
    useSetApplicationStatus: () => ({ mutateAsync: vi.fn(), isPending: false }),
    useUpdateApplication: () => ({ mutate: vi.fn(), isPending: false }),
    useOpenExternal: () => ({ mutate: vi.fn() }),
    useRemoveApplication: () => ({ mutateAsync: vi.fn(), isPending: false }),
    useDocuments: () => ({ data: [], isLoading: false }),
    useDocumentText: () => ({ data: undefined, isLoading: false }),
  };
});

vi.mock('@/services/use-ai-generations', () => ({
  useAiGenerations: () => ({ data: [] }),
}));

import { ApplicationDetailPage } from '@/features/applications/components/ApplicationDetailPage';
import { DETAIL_TABS } from '@/routes/applications.$id';

afterEach(() => cleanup());

const FROMS = ['jobs', 'autopilot', 'applications'] as const;

function buildRouter(initial: string) {
  const validateSearch = (
    s: Record<string, unknown>
  ): {
    tab?: (typeof DETAIL_TABS)[number];
    from?: (typeof FROMS)[number];
  } => ({
    tab: (DETAIL_TABS as readonly string[]).includes(s.tab as string)
      ? (s.tab as (typeof DETAIL_TABS)[number])
      : undefined,
    from: FROMS.includes(s.from as (typeof FROMS)[number])
      ? (s.from as (typeof FROMS)[number])
      : undefined,
  });

  const rootRoute = createRootRoute({
    component: () => <Outlet />,
    notFoundComponent: () => <div data-testid="notfound">Page not found</div>,
  });
  const indexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/',
    component: () => <div data-testid="dashboard">dashboard</div>,
  });
  const jobsRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/jobs',
    component: () => <div data-testid="jobs-list">jobs</div>,
  });
  const applicationsLayout = createRoute({
    getParentRoute: () => rootRoute,
    path: '/applications',
    component: () => <Outlet />,
  });
  const applicationsIndex = createRoute({
    getParentRoute: () => applicationsLayout,
    path: '/',
    component: () => <div data-testid="applications-list">list</div>,
  });
  const applicationsDetail = createRoute({
    getParentRoute: () => applicationsLayout,
    path: '$id',
    validateSearch,
    component: ApplicationDetailPage,
  });

  const routeTree = rootRoute.addChildren([
    indexRoute,
    jobsRoute,
    applicationsLayout.addChildren([applicationsIndex, applicationsDetail]),
  ]);

  return createRouter({
    routeTree,
    defaultPreload: 'intent',
    defaultPreloadStaleTime: 0,
    history: createMemoryHistory({ initialEntries: [initial] }),
  });
}

describe('Tailor → documents-tab → Back regression', () => {
  it('Back from the Tailor flow (from=jobs) returns to /jobs, not the dashboard, with the guard active', async () => {
    const router = buildRouter('/applications/abc?tab=documents&from=jobs');
    installUnknownPathRedirect(router);
    render(<RouterProvider router={router} />, { wrapper: withProviders(createMockClient()) });

    const backButton = await screen.findByText('applications.detail.backJobs');
    fireEvent.click(backButton);

    await waitFor(() => {
      expect(router.state.isLoading).toBe(false);
    });
    expect(router.state.location.pathname).toBe('/jobs');
    expect(screen.queryByTestId('dashboard')).not.toBeInTheDocument();
  });

  it('Back from a deep-link (no `from`) defaults to /applications, not the dashboard, with the guard active', async () => {
    const router = buildRouter('/applications/abc?tab=documents');
    installUnknownPathRedirect(router);
    render(<RouterProvider router={router} />, { wrapper: withProviders(createMockClient()) });

    const backButton = await screen.findByText('applications.detail.back');
    fireEvent.click(backButton);

    await waitFor(() => {
      expect(router.state.isLoading).toBe(false);
    });
    expect(router.state.location.pathname).toBe('/applications');
    expect(screen.queryByTestId('dashboard')).not.toBeInTheDocument();
  });
});
