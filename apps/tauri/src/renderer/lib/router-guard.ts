import { type AnyRouteMatch, type AnyRouter, rootRouteId } from '@tanstack/react-router';

/**
 * Redirect genuinely-unknown paths to home, keeping matched dynamic/param routes.
 *
 * WHY NOT `routesByPath`:
 * The previous guard did an exact-pathname lookup against `router.routesByPath`.
 * That map is keyed by route PATTERNS (e.g. `/applications/$id`) but was looked
 * up by the RESOLVED pathname (e.g. `/applications/abc123`). Static routes happen
 * to pass because pathname === pattern, but every dynamic/param route fell through
 * the lookup and was wrongly redirected to `/` (clicking an application sent the
 * user home).
 *
 * Instead we read the resolved match chain (`router.state.matches`), which is the
 * leaf-to-root list of matches TanStack actually resolved for the current
 * location. A real route — static OR dynamic — yields a non-root leaf match; a
 * genuinely-unknown path yields only the root route as leaf and/or a match flagged
 * `globalNotFound` / `status === 'notFound'`. Using the match chain treats matched
 * dynamic routes as known, which exact-pathname matching could never do.
 *
 * @param router the live router instance
 * @returns the unsubscribe function (call to detach the `onResolved` listener)
 */
export function installUnknownPathRedirect(router: AnyRouter): () => void {
  return router.subscribe('onResolved', () => {
    const matches = router.state.matches;
    const leaf: AnyRouteMatch | undefined = matches[matches.length - 1];

    // A real (known) route produces a non-root leaf that isn't a not-found match.
    const matched =
      !!leaf && leaf.routeId !== rootRouteId && leaf.status !== 'notFound' && !leaf.globalNotFound;

    if (router.state.location.pathname !== '/' && !matched) {
      void router.navigate({ to: '/', replace: true });
    }
  });
}
