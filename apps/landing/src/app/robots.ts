import type { MetadataRoute } from 'next';

// Next emits this as a real file at out/robots.txt under `output: 'export'`.
//
// Everything is crawlable, deliberately — including /mission-control, which is
// noindex (`robots: { index: false }` in its page metadata). Disallowing it here
// would not reinforce that, it would cancel it: a crawler that obeys Disallow
// never fetches the page, so it never reads the noindex, and the bare URL can
// still be indexed from external links. noindex needs the page to be fetchable.
// Required by Next 16: metadata routes default to dynamic, which `output:
// 'export'` rejects outright.
export const dynamic = 'force-static';

export default function robots(): MetadataRoute.Robots {
  return {
    rules: { userAgent: '*', allow: '/' },
    sitemap: 'https://aijobhunter.app/sitemap.xml',
  };
}
