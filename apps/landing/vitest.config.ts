import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { defineConfig } from 'vitest/config';

// Minimal node-env project for the landing app's pure logic (scroll p-space
// math + the pre-split geometry builder). No DOM: these modules are plain math
// over typed arrays / three BufferGeometry, so jsdom is unnecessary. Joins the
// root run via the `projects` array in the repo-root vitest.config.ts and runs
// standalone via `pnpm -F @ajh/landing test`.
const here = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  test: {
    name: 'landing',
    globals: true,
    environment: 'node',
    include: ['src/**/*.{test,spec}.ts'],
  },
  resolve: {
    // Mirror the tsconfig `@/* -> ./src/*` path so modules under test that use
    // the alias (e.g. presplit.ts imports `@/engine/pages`) resolve. Regex form
    // matches only the `@/` prefix, never bare `@`-scoped package names.
    alias: [{ find: /^@\//, replacement: `${resolve(here, 'src')}/` }],
  },
});
