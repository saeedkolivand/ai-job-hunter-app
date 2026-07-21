// Self-hosted web fonts. The stylesheet + woff2 files live in public/fonts/
// (same-origin, no third-party CDN) per ADR-0018's origin invariant — see
// CspMeta.tsx. React 19 hoists this <link> into <head>. Regenerate the assets
// with `pnpm --filter @ajh/landing gen:fonts` (scripts/selfhost-fonts.mjs).
export function Fonts() {
  return <link rel="stylesheet" href="/fonts/fonts.css" />;
}
