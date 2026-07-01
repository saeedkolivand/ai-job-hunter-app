// Per-template preview images (a generic sample résumé rendered in each of the
// 9 templates). The SVGs are produced offline by the ignored Rust test
// `generate_templates_showcase_banner` (export/typst_engine/test.rs) and
// committed under ./assets/template-previews/<template-id>.svg. SVG (vector,
// glyphs as paths) replaced the old PNGs: crisp at any zoom and far smaller.
//
// Vite emits each SVG as a separate hashed URL, so this static glob yields URLs
// and the browser fetches only the image actually shown. CSP already allows
// `img-src 'self'`. Missing files degrade gracefully: the panel shows a
// caption-only fallback for any id without an image.

import type { TemplateId } from '@/lib/generate';

const modules = import.meta.glob<string>('../assets/template-previews/*.svg', {
  eager: true,
  query: '?url',
  import: 'default',
});

/** kebab template id → emitted image URL (only ids that have a committed SVG). */
export const TEMPLATE_PREVIEWS: Partial<Record<TemplateId, string>> = Object.fromEntries(
  Object.entries(modules).map(([path, url]) => {
    const id =
      path
        .split('/')
        .pop()
        ?.replace(/\.svg$/, '') ?? '';
    return [id, url];
  })
);
