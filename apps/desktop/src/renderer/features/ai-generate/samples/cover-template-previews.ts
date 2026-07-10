// Per-template preview images (a sample cover letter rendered in each
// template). The SVGs are produced offline by the ignored Rust test
// `generate_cover_template_previews` (export/typst_engine/test.rs) and
// committed under ./assets/cover-template-previews/<template-id>.svg.
//
// Vite emits each SVG as a separate hashed URL so this static glob yields URLs
// and the browser fetches only the image actually shown. CSP already allows
// `img-src 'self'`. Missing files degrade gracefully: the panel shows a
// caption-only fallback for any id without an image.

import type { TemplateId } from '@/lib/generate';

const modules = import.meta.glob<string>('../assets/cover-template-previews/*.svg', {
  eager: true,
  query: '?url',
  import: 'default',
});

/** kebab template id → emitted SVG URL (only ids that have a committed SVG). */
export const COVER_TEMPLATE_PREVIEWS: Partial<Record<TemplateId, string>> = Object.fromEntries(
  Object.entries(modules).map(([path, url]) => {
    const id =
      path
        .split('/')
        .pop()
        ?.replace(/\.svg$/, '') ?? '';
    return [id, url];
  })
);
