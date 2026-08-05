import type { MetadataRoute } from 'next';

// Next emits this as out/sitemap.xml under `output: 'export'`. Paths match each
// page's own `alternates.canonical` (root keeps its trailing slash; the rest are
// extensionless, per `trailingSlash: false`). /mission-control is deliberately
// absent — it is noindex (see robots.ts).
//
// ponytail: no lastModified/priority — Google ignores priority outright, and a
// build-time timestamp would just churn the file on every deploy. Add real
// git-mtime dates only if Search Console ever shows a crawl-staleness problem.
const ROUTES = [
  '/',
  '/download',
  '/how-it-works',
  '/privacy',
  '/accessibility',
  '/world',
  '/creature',
  '/architecture-map',
  '/agent-system',
  '/tech-radar',
];

// Required by Next 16 (see robots.ts).
export const dynamic = 'force-static';

export default function sitemap(): MetadataRoute.Sitemap {
  return ROUTES.map((route) => ({ url: `https://aijobhunter.app${route}` }));
}
