import type { MetadataRoute } from 'next';

// Next emits this as out/sitemap.xml under `output: 'export'`. Paths match each
// page's own `alternates.canonical` (root keeps its trailing slash; the rest are
// extensionless, per `trailingSlash: false`). /mission-control is deliberately
// absent — it is noindex (see robots.ts).
//
// ponytail: no priority (Google ignores it outright) and lastmod is a hand-kept
// literal, not a build-time timestamp — a fresh date every deploy is the churn
// Google learns to distrust. Derived from each route's last content-bearing
// commit; bump the date only when the page's *content* materially changes (a
// lint sweep or refactor is not a change a crawler cares about). Too-old is the
// safe failure direction: Google just doesn't re-prioritise the URL.
const ROUTES: [route: string, lastModified: string][] = [
  ['/', '2026-08-06'],
  ['/download', '2026-08-05'],
  ['/how-it-works', '2026-08-05'],
  ['/privacy', '2026-08-05'],
  ['/accessibility', '2026-08-05'],
  ['/world', '2026-08-01'],
  ['/creature', '2026-07-30'],
  ['/architecture-map', '2026-08-05'],
  ['/agent-system', '2026-08-05'],
  ['/tech-radar', '2026-08-05'],
];

// Required by Next 16 (see robots.ts).
export const dynamic = 'force-static';

export default function sitemap(): MetadataRoute.Sitemap {
  return ROUTES.map(([route, lastModified]) => ({
    url: `https://aijobhunter.app${route}`,
    lastModified,
  }));
}
