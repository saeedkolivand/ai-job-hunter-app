import { ROUTES } from '@/constants/routes';

/**
 * Maps a resolved pathname to its logical parent route for the global back button.
 * Returns null on top-level routes (no back button should render).
 * ponytail: single source of truth — add one line per new detail route.
 */
export function parentRoute(pathname: string): string | null {
  if (/^\/applications\/[^/]+$/.test(pathname)) return ROUTES.APPLICATIONS;
  return null;
}
