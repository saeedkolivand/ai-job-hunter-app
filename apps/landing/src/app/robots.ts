import type { MetadataRoute } from 'next';

// Next emits this as a real file at out/robots.txt under `output: 'export'`.
// Everything is public except /mission-control, which is a live GitHub-API
// dashboard, not content — it already sets `robots: { index: false }` in its
// page metadata; this repeats it at the crawl layer so bots skip the fetch.
// Required by Next 16: metadata routes default to dynamic, which `output:
// 'export'` rejects outright.
export const dynamic = 'force-static';

export default function robots(): MetadataRoute.Robots {
  return {
    rules: { userAgent: '*', allow: '/', disallow: '/mission-control' },
    sitemap: 'https://aijobhunter.app/sitemap.xml',
  };
}
