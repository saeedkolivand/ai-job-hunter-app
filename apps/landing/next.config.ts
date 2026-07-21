import path from 'node:path';

import type { NextConfig } from 'next';

const nextConfig: NextConfig = {
  // Static export for GitHub Pages: `next build` emits ./out (HTML/CSS/JS), no
  // server runtime. This app has no server features and never will (ADR-0018).
  output: 'export',
  // Flat files: `/creature` → out/creature.html (not out/creature/index.html), so
  // both `/creature` and the legacy `/creature.html` keep resolving on Pages —
  // byte-shape parity with the old hand-authored static site.
  trailingSlash: false,
  // Companion to `output: export` — the default image optimizer needs a server.
  images: { unoptimized: true },
  // Next 16 removed the built-in ESLint integration (and rejects the `eslint`
  // config key), so `next build` no longer lints — the central eslint.config.mjs
  // (`pnpm lint:strict`) remains the lint gate.
  // This app lives in a pnpm workspace; point file tracing at the repo root so
  // Next resolves hoisted deps instead of walking out of the monorepo.
  outputFileTracingRoot: path.join(import.meta.dirname, '../../'),
};

export default nextConfig;
