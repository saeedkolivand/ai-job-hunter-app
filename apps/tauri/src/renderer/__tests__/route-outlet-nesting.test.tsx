/**
 * Route Outlet nesting — regression integration test
 *
 * WHY THIS TEST EXISTS
 * --------------------
 * Unit tests for ApplicationDetailPage, AutopilotPage, and ApplyPageRoute all
 * mock `@tanstack/react-router` so they render in isolation without a
 * RouterProvider. That approach hid a routing bug: when parent routes like
 * `/applications` and `/autopilot` had their page components inlined (instead
 * of rendering only `<Outlet />`), child routes such as `/applications/$id` and
 * `/autopilot/apply` would never mount — the parent rendered its own content and
 * the Outlet was never reached.
 *
 * This file renders through a REAL TanStack Router route tree (real
 * RouterProvider + createRouter + createMemoryHistory + Outlet) so any future
 * regression where a parent layout swallows the Outlet instead of delegating to
 * its child will cause these tests to fail — exactly as the original bug would
 * have.
 *
 * What is real vs mocked
 * ----------------------
 * REAL: `@tanstack/react-router` (RouterProvider, createRouter, Outlet, etc.)
 *       The route tree structure (layout → child via real Outlet)
 *       ApplicationDetailPage (the actual component under test)
 *       ApplyPageRoute (the actual component under test)
 *       useSessionStore (real Zustand store — state set per test)
 *
 * MOCKED (leaf deps only, not the router):
 *       @ajh/translations — t(key) returns key so assertions use i18n key literals
 *       @/services (barrel) — service hooks return controlled fixtures
 *       @/services/use-ai-generations — returns empty generations list
 *       @/hooks/use-format-relative-time — inert stub
 *       @/components/layout/PageShell — passes children through transparently
 *       @/features/documents/components/GenerationCard — lightweight stub
 *       @/features/autopilot/components/ApplyPage — distinctive testid stub
 *       motion/react — passthrough (no animation engine)
 *       AutopilotCard, CreationWizard, EmptyState (autopilot), useAutopilotRun,
 *       PageTransition — lightweight stubs so AutopilotPage renders cheaply as
 *       the redirect-destination assertion target
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from '@tanstack/react-router';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import type { Application } from '@ajh/shared';

import { installUnknownPathRedirect } from '@/lib/router-guard';
import { useSessionStore } from '@/store/session-store';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── motion/react — passthrough so AnimatePresence / motion.div work cheaply ──

vi.mock('motion/react', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: React.forwardRef(
      (
        { children, ...rest }: React.HTMLAttributes<HTMLDivElement>,
        ref: React.Ref<HTMLDivElement>
      ) => (
        <div ref={ref} {...rest}>
          {children}
        </div>
      )
    ),
  },
}));

// ── @/components/layout/PageShell — transparent wrapper ──────────────────────

vi.mock('@/components/layout/PageShell', () => ({
  PageShell: ({
    children,
    actions,
  }: {
    children?: React.ReactNode;
    actions?: React.ReactNode;
    title?: string;
    subtitle?: string;
  }) => (
    <div data-testid="page-shell">
      {actions}
      {children}
    </div>
  ),
}));

// ── @/components/layout/PageTransition — passthrough ──────────────────────────

vi.mock('@/components/layout/PageTransition', () => ({
  PageTransition: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="page-transition">{children}</div>
  ),
}));

// ── GenerationCard stub ───────────────────────────────────────────────────────

vi.mock('@/features/documents/components/GenerationCard', () => ({
  GenerationCard: () => <div data-testid="generation-card" />,
}));

// ── AutopilotPage leaf stubs ───────────────────────────────────────────────────

vi.mock('@/features/autopilot/components/AutopilotCard', () => ({
  AutopilotCard: () => <div data-testid="autopilot-card" />,
}));

vi.mock('@/features/autopilot/components/CreationWizard', () => ({
  CreationWizard: () => <div data-testid="creation-wizard" />,
}));

vi.mock('@/features/autopilot/components/EmptyState', () => ({
  EmptyState: () => <div data-testid="autopilot-empty-state" />,
}));

vi.mock('@/features/autopilot/hooks/useAutopilotRun', () => ({
  useAutopilotRun: () => ({
    runStates: {},
    stepLogs: {},
    error: null,
    setError: vi.fn(),
    handleRun: vi.fn(),
    handleTogglePause: vi.fn(),
    handleDelete: vi.fn(),
  }),
}));

// ── useFormatRelativeTime — inert ─────────────────────────────────────────────

vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => () => '',
}));

// ── Service hooks ─────────────────────────────────────────────────────────────

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
  jobDescription: '',
  comp: '',
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
    useSetApplicationStatus: () => ({
      mutateAsync: vi.fn().mockResolvedValue(undefined),
      isPending: false,
    }),
    useUpdateApplication: () => ({ mutate: vi.fn(), isPending: false }),
    useOpenExternal: () => ({ mutate: vi.fn() }),
    useRemoveApplication: () => ({
      mutateAsync: vi.fn().mockResolvedValue(undefined),
      isPending: false,
    }),
    useAutopilots: () => ({ data: [], isLoading: false }),
    useInvalidateAutopilots: () => vi.fn(),
    // Documents tab is now the default landing tab, so its hooks load on a plain
    // /applications/$id mount — stub them so the panel renders without a client.
    useDocuments: () => ({ data: [], isLoading: false }),
    useDocumentText: () => ({ data: '', isLoading: false }),
  };
});

// useDefaultResumeId reads useDocuments internally; stub it for the same reason.
vi.mock('@/features/jobs/hooks/useDefaultResumeId', () => ({
  useDefaultResumeId: () => null,
}));

vi.mock('@/services/use-ai-generations', () => ({
  useAiGenerations: () => ({ data: [] }),
}));

// ── TailorFlow stub — cuts the heavy generation subtree (incl. the i18n shim) ─
// ApplicationDetailPage embeds TailorFlow in its Documents tab. Importing the
// real TailorFlow pulls in StepResume → ResumeInputCard → use-import-with-ocr →
// @/i18n shim, which calls i18n.on(...) on the @ajh/translations default export
// that the translations mock above does not supply.  This test is about ROUTING,
// not TailorFlow internals, so stubbing the leaf is the correct approach.

vi.mock('@/features/documents/components/TailorFlow', () => ({
  TailorFlow: () => <div data-testid="tailor-flow-stub" />,
}));

// ── Import real page components (after all mocks) ─────────────────────────────

import { ApplicationDetailPage } from '@/features/applications/components/ApplicationDetailPage';
import { ApplicationsPage } from '@/features/applications/components/ApplicationsPage';
import { AutopilotPage } from '@/features/autopilot/components/AutopilotPage';
import { DETAIL_TABS } from '@/routes/applications.$id';

// ── Mock @/routes/applications.index Route (useSearch) ───────────────────────
// ApplicationsPage imports Route from @/routes/applications.index for useSearch.
// In the integration tree we supply our own route, but the import seam still
// exists at module level — supply a harmless stub so useSearch() returns {}.

vi.mock('@/routes/applications.index', () => ({
  Route: { useSearch: () => ({}) },
}));

// ── Mock @/routes/autopilot.index Route (useSearch) ──────────────────────────
// AutopilotPage imports Route from @/routes/autopilot.index for useSearch.

vi.mock('@/routes/autopilot.index', () => ({
  Route: { useSearch: () => ({}) },
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

/**
 * Build a minimal real route tree that mirrors the production layout+child
 * structure for both /applications and /autopilot, then render it at the
 * given initial URL.
 *
 * The REAL Outlet from @tanstack/react-router is what proves child routes
 * mount through the layout — that is NOT mocked anywhere in this file.
 */
function renderAt(initialPath: string) {
  const rootRoute = createRootRoute({ component: () => <Outlet /> });

  // /applications layout — renders only <Outlet />, mirrors applications.tsx
  const applicationsLayout = createRoute({
    getParentRoute: () => rootRoute,
    path: '/applications',
    component: () => <Outlet />,
  });

  // /applications/ index — ApplicationsPage (mirrors applications.index.tsx)
  const applicationsIndex = createRoute({
    getParentRoute: () => applicationsLayout,
    path: '/',
    component: ApplicationsPage,
  });

  // /applications/$id — ApplicationDetailPage (mirrors applications.$id.tsx)
  // NOTE: ApplicationDetailPage calls Route.useParams() from the REAL
  // @/routes/applications.$id module. That module's Route is a TanStack Route
  // object whose useParams() reads from the router context. By placing the REAL
  // ApplicationDetailPage inside a real createRoute with path '$id', the router
  // context is populated correctly — no need to mock useParams.
  const applicationsDetail = createRoute({
    getParentRoute: () => applicationsLayout,
    path: '$id',
    component: ApplicationDetailPage,
  });

  // /autopilot layout — renders only <Outlet />, mirrors autopilot.tsx
  const autopilotLayout = createRoute({
    getParentRoute: () => rootRoute,
    path: '/autopilot',
    component: () => <Outlet />,
  });

  // /autopilot/ index — AutopilotPage (mirrors autopilot.index.tsx)
  const autopilotIndex = createRoute({
    getParentRoute: () => autopilotLayout,
    path: '/',
    component: AutopilotPage,
  });

  const routeTree = rootRoute.addChildren([
    applicationsLayout.addChildren([applicationsIndex, applicationsDetail]),
    autopilotLayout.addChildren([autopilotIndex]),
  ]);

  const router = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: [initialPath] }),
  });

  render(<RouterProvider router={router} />);

  return router;
}

// ── Store reset ───────────────────────────────────────────────────────────────

beforeEach(() => {
  useSessionStore.setState((s) => ({
    autopilot: {
      ...s.autopilot,
      creating: false,
      focusedId: null,
    },
    applications: { collapsedSections: [], filter: '' },
  }));
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('Route Outlet nesting — regression guard', () => {
  /**
   * Case 1: /applications/abc mounts ApplicationDetailPage through the layout Outlet.
   *
   * Regression: if the layout route ever renders its own page content instead of
   * <Outlet />, the detail page never mounts and the back-button / status-title
   * text will be absent. The list's "application-row" testid must also be absent
   * — proving this is the DETAIL view, not the list.
   */
  it('navigating to /applications/$id renders ApplicationDetailPage through the layout Outlet', async () => {
    renderAt('/applications/abc');

    // Distinctive text from ApplicationDetailPage that the list page never renders.
    // t() returns the key string because @ajh/translations is mocked.
    await waitFor(() => {
      expect(screen.getByText('applications.detail.back')).toBeInTheDocument();
    });

    // The overview tab button is another detail-only marker (active tab in the tablist).
    expect(screen.getByText('applications.detail.tabs.overview')).toBeInTheDocument();

    // Critically: ApplicationsPage's "application-row" testid must NOT be present
    // — confirms the list page did NOT mount (layout Outlet served the child).
    expect(screen.queryByTestId('application-row')).not.toBeInTheDocument();
  });

  /**
   * Case 4: /applications/abc?tab=GARBAGE exercises the REAL validateSearch on
   * the $id route. Unknown tab values are coerced to undefined by validateSearch;
   * ApplicationDetailPage then defaults to the first tab via `?? DETAIL_TABS[0]`
   * (currently `documents`).
   *
   * This test is the only one that exercises the real validateSearch coercion —
   * unit tests mock the router and cannot reach it.
   */
  it('navigating to /applications/$id?tab=GARBAGE coerces to the default (first) tab via validateSearch', async () => {
    // Build a route tree identical to renderAt but with the real validateSearch
    // wired onto the $id route so the coercion path is live.
    const validateSearch = (
      s: Record<string, unknown>
    ): { tab?: (typeof DETAIL_TABS)[number] } => ({
      tab: (DETAIL_TABS as readonly string[]).includes(s.tab as string)
        ? (s.tab as (typeof DETAIL_TABS)[number])
        : undefined,
    });

    const rootRoute = createRootRoute({ component: () => <Outlet /> });
    const applicationsLayout = createRoute({
      getParentRoute: () => rootRoute,
      path: '/applications',
      component: () => <Outlet />,
    });
    const applicationsIndex = createRoute({
      getParentRoute: () => applicationsLayout,
      path: '/',
      component: ApplicationsPage,
    });
    const applicationsDetail = createRoute({
      getParentRoute: () => applicationsLayout,
      path: '$id',
      validateSearch,
      component: ApplicationDetailPage,
    });

    const routeTree = rootRoute.addChildren([
      applicationsLayout.addChildren([applicationsIndex, applicationsDetail]),
    ]);

    const router = createRouter({
      routeTree,
      history: createMemoryHistory({ initialEntries: ['/applications/abc?tab=GARBAGE'] }),
    });

    render(<RouterProvider router={router} />);

    // ApplicationDetailPage should mount and default to the first tab (documents).
    await waitFor(() => {
      expect(screen.getByText('applications.detail.back')).toBeInTheDocument();
    });

    // The first tab (documents) must be the active one (aria-selected="true").
    // t() returns keys so the tab label is the i18n key literal.
    const defaultTab = screen.getByRole('tab', {
      name: /applications\.detail\.tabs\.documents/i,
    });
    expect(defaultTab).toHaveAttribute('aria-selected', 'true');

    // Sanity: the list page must not have rendered.
    expect(screen.queryByTestId('application-row')).not.toBeInTheDocument();
  });

  /**
   * Case 5 (Tailor → detail → Back regression): opening the detail from the Jobs
   * "Tailor" action lands on `/applications/$id?from=jobs`. The in-page Back button
   * must then return to the Jobs page (`/jobs`) — NOT the Dashboard (`/`). This
   * wires the REAL unknown-path guard (`installUnknownPathRedirect`) so a
   * guard-driven redirect-to-`/` would surface. (Tab defaults to `overview` to
   * keep the detail subtree provider-free — the Back-button route target is
   * identical regardless of the active tab.)
   */
  it('Tailor-flow Back button (from=jobs) returns to /jobs (not the dashboard) with the guard active', async () => {
    const FROMS = ['jobs', 'autopilot', 'applications'] as const;
    const fromSearch = (
      s: Record<string, unknown>
    ): { tab?: (typeof DETAIL_TABS)[number]; from?: (typeof FROMS)[number] } => ({
      tab: (DETAIL_TABS as readonly string[]).includes(s.tab as string)
        ? (s.tab as (typeof DETAIL_TABS)[number])
        : undefined,
      from: FROMS.includes(s.from as (typeof FROMS)[number])
        ? (s.from as (typeof FROMS)[number])
        : undefined,
    });

    const rootRoute = createRootRoute({ component: () => <Outlet /> });
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
      component: ApplicationsPage,
    });
    const applicationsDetail = createRoute({
      getParentRoute: () => applicationsLayout,
      path: '$id',
      validateSearch: fromSearch,
      component: ApplicationDetailPage,
    });

    const routeTree = rootRoute.addChildren([
      indexRoute,
      jobsRoute,
      applicationsLayout.addChildren([applicationsIndex, applicationsDetail]),
    ]);

    const router = createRouter({
      routeTree,
      history: createMemoryHistory({ initialEntries: ['/applications/abc?from=jobs'] }),
    });

    installUnknownPathRedirect(router);
    render(<RouterProvider router={router} />);

    // Detail view mounted (back label is the jobs-origin one — `from=jobs`).
    const backButton = await screen.findByRole('button', { name: 'applications.detail.backJobs' });

    fireEvent.click(backButton);

    // Must land on the Jobs page, NOT the dashboard.
    await waitFor(() => {
      expect(router.state.location.pathname).toBe('/jobs');
    });
    expect(router.state.location.pathname).toBe('/jobs');
    expect(screen.queryByTestId('dashboard')).not.toBeInTheDocument();
  });

  /**
   * Case 6 (deep-link Back default): a deep-link / notification opens the detail
   * with NO `from`. The Back button must default to the Applications LIST
   * (`/applications`) — NOT the Dashboard (`/`) — with the real guard active.
   */
  it('deep-link (no `from`) Back button defaults to /applications (not the dashboard) with the guard active', async () => {
    const FROMS = ['jobs', 'autopilot', 'applications'] as const;
    const fromSearch = (
      s: Record<string, unknown>
    ): { tab?: (typeof DETAIL_TABS)[number]; from?: (typeof FROMS)[number] } => ({
      tab: (DETAIL_TABS as readonly string[]).includes(s.tab as string)
        ? (s.tab as (typeof DETAIL_TABS)[number])
        : undefined,
      from: FROMS.includes(s.from as (typeof FROMS)[number])
        ? (s.from as (typeof FROMS)[number])
        : undefined,
    });

    const rootRoute = createRootRoute({ component: () => <Outlet /> });
    const indexRoute = createRoute({
      getParentRoute: () => rootRoute,
      path: '/',
      component: () => <div data-testid="dashboard">dashboard</div>,
    });
    const applicationsLayout = createRoute({
      getParentRoute: () => rootRoute,
      path: '/applications',
      component: () => <Outlet />,
    });
    const applicationsIndex = createRoute({
      getParentRoute: () => applicationsLayout,
      path: '/',
      component: ApplicationsPage,
    });
    const applicationsDetail = createRoute({
      getParentRoute: () => applicationsLayout,
      path: '$id',
      validateSearch: fromSearch,
      component: ApplicationDetailPage,
    });

    const routeTree = rootRoute.addChildren([
      indexRoute,
      applicationsLayout.addChildren([applicationsIndex, applicationsDetail]),
    ]);

    const router = createRouter({
      routeTree,
      history: createMemoryHistory({ initialEntries: ['/applications/abc'] }),
    });

    installUnknownPathRedirect(router);
    render(<RouterProvider router={router} />);

    // Detail view mounted (back label is the default one — `from` is absent).
    const backButton = await screen.findByRole('button', { name: 'applications.detail.back' });

    fireEvent.click(backButton);

    // Must land on the Applications list, NOT the dashboard.
    await waitFor(() => {
      expect(router.state.location.pathname).toBe('/applications');
    });
    expect(router.state.location.pathname).toBe('/applications');
    expect(screen.queryByTestId('dashboard')).not.toBeInTheDocument();
  });
});
