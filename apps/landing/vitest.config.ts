import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

const here = dirname(fileURLToPath(import.meta.url));

// Pure-logic unit tests default to node env — no DOM (e.g. the /download
// version-freshness comparison). Component tests that touch the DOM (e.g.
// DownloadFreshness, which reads/mutates `document`) opt in per-file via a
// `// @vitest-environment jsdom` pragma rather than flipping the whole
// project to jsdom. The faithful page markup is still verified end-to-end by
// scripts/check-parity.mjs against the built out/, not here.
export default defineConfig({
  plugins: [react()],
  resolve: {
    // Mirrors tsconfig.json's `@/*` path so vitest (unlike Next's own bundler)
    // can resolve it too — production files (e.g. app/download/page.tsx) use it.
    alias: { '@': resolve(here, 'src') },
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.{ts,tsx}'],
  },
});
