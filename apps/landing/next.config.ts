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
  // The central eslint.config.mjs (run via `pnpm lint:strict`) is the lint gate;
  // don't run Next's bundled ESLint during build (it wants eslint-config-next).
  eslint: { ignoreDuringBuilds: true },
  // This app lives in a pnpm workspace; point file tracing at the repo root so
  // Next resolves hoisted deps instead of walking out of the monorepo.
  outputFileTracingRoot: path.join(import.meta.dirname, '../../'),
};

export default nextConfig;
