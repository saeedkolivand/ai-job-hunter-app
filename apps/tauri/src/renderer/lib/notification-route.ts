/**
 * notification-route — validate backend-supplied route destinations.
 *
 * The Rust backend emits notification `route.to` strings at runtime.
 * TanStack Router's `navigate` is typed against a static union of known
 * paths; this helper validates the candidate string against that union
 * before navigation so an unknown destination falls back to '/' instead
 * of silently breaking.
 *
 * `KnownRoutePath` is sourced directly from `FileRouteTypes['to']` in
 * routeTree.gen.ts (the TanStack Router generated file). The exhaustive
 * `KNOWN_ROUTE_TABLE` below is keyed by that union, so adding or removing
 * a route in routeTree.gen.ts causes a typecheck failure here until the
 * table is updated — the runtime `KNOWN_ROUTE_PATHS` Set can never
 * silently drift from the router.
 */

import type { FileRouteTypes } from '@/routeTree.gen';

/** Union of the router's known `to` paths — sourced from routeTree.gen.ts. */
export type KnownRoutePath = FileRouteTypes['to'];

/** Fallback destination used for unknown routes. */
export const ROUTE_FALLBACK: KnownRoutePath = '/';

// Exhaustive over KnownRoutePath: a new/removed route in routeTree.gen
// fails typecheck here until this table matches. The runtime Set is derived
// from it, so KNOWN_ROUTE_PATHS can never silently drift from the router.
const KNOWN_ROUTE_TABLE: Record<KnownRoutePath, true> = {
  '/': true,
  '/ai-generate': true,
  '/analyze': true,
  '/applications': true,
  '/applications/$id': true,
  '/autopilot': true,
  '/build': true,
  '/jobs': true,
  '/monitoring': true,
  '/documents': true,
  '/search': true,
  '/settings': true,
  '/support': true,
};

/**
 * Build the known-paths Set from the exhaustive route table.
 * Kept as a `const` so callers can cache it across renders if desired.
 */
export const KNOWN_ROUTE_PATHS: Set<KnownRoutePath> = new Set(
  Object.keys(KNOWN_ROUTE_TABLE) as KnownRoutePath[]
);

/**
 * Pure core — accepts the set of known paths and a candidate string.
 * Returns `true` when the candidate is a member of the set.
 *
 * Accepts any `Set<string>` so unit tests can drive it directly
 * without importing the router.
 */
export function isKnownRoute(knownPaths: Set<string>, candidate: string): boolean {
  return knownPaths.has(candidate);
}

/**
 * Validate `candidate` against the known router paths.
 *
 * - Known path   → returns it typed as `KnownRoutePath`.
 * - Unknown path → logs a warning and returns `ROUTE_FALLBACK`.
 */
export function resolveNotificationRoute(candidate: string): KnownRoutePath {
  if (isKnownRoute(KNOWN_ROUTE_PATHS, candidate)) {
    return candidate as KnownRoutePath;
  }
  console.warn(
    `[notification-route] Unknown route destination "${candidate}" from backend notification. Falling back to "${ROUTE_FALLBACK}".`
  );
  return ROUTE_FALLBACK;
}
