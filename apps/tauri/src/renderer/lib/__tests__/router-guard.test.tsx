/**
 * installUnknownPathRedirect — regression guard for the dynamic-route bug.
 *
 * WHY THIS TEST EXISTS
 * --------------------
 * The previous unknown-path redirect did an exact-pathname lookup against
 * `router.routesByPath`, which is keyed by route PATTERNS (`/x/$id`) but was
 * queried with the RESOLVED pathname (`/x/123`). Static routes passed because
 * pathname === pattern, but every dynamic/param route fell through and was wrongly
 * redirected to `/` — clicking an application sent the user home.
 *
 * This test exercises the REAL extracted guard against a REAL router (param +
 * static + unknown paths) so a regression to exact-pathname matching fails here.
 */

import { afterEach, describe, expect, it } from 'vitest';
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from '@tanstack/react-router';
import { cleanup, render, waitFor } from '@testing-library/react';

import { installUnknownPathRedirect } from '../router-guard';

afterEach(() => {
  cleanup();
});

/**
 * Build a real route tree (index `/`, static `/static`, dynamic `/x/$id`), mount
 * it at `initialPath`, install the guard, and return the router so callers can
 * assert on `router.state.location.pathname`. Components are trivial stubs so no
 * app deps are pulled in.
 */
function mountAt(initialPath: string) {
  const rootRoute = createRootRoute({ component: () => <Outlet /> });

  const indexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/',
    component: () => <div>index</div>,
  });

  const staticRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/static',
    component: () => <div>static</div>,
  });

  const dynamicRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/x/$id',
    component: () => <div>dynamic</div>,
  });

  const routeTree = rootRoute.addChildren([indexRoute, staticRoute, dynamicRoute]);

  const router = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: [initialPath] }),
  });

  installUnknownPathRedirect(router);
  render(<RouterProvider router={router} />);

  return router;
}

describe('installUnknownPathRedirect', () => {
  it('keeps a matched dynamic /x/$id route — does NOT redirect', async () => {
    const router = mountAt('/x/123');

    await waitFor(() => {
      expect(router.state.location.pathname).toBe('/x/123');
    });
    expect(router.state.location.pathname).toBe('/x/123');
  });

  it('redirects a genuinely-unknown path to /', async () => {
    const router = mountAt('/totally-unknown');

    await waitFor(() => {
      expect(router.state.location.pathname).toBe('/');
    });
  });

  it('keeps a matched static /static route — does NOT redirect', async () => {
    const router = mountAt('/static');

    await waitFor(() => {
      expect(router.state.location.pathname).toBe('/static');
    });
    expect(router.state.location.pathname).toBe('/static');
  });
});
