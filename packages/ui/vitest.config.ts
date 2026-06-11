import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// ESM-load this config (note `import.meta.url`) so `@vitejs/plugin-react` is
// imported via ESM rather than require()'d during vitest's concurrent project
// setup — the require()-of-ESM path races under node 22 (ERR_INTERNAL_ASSERTION).
const here = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [react()],
  test: {
    name: 'ui',
    globals: true,
    environment: 'jsdom',
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    setupFiles: [resolve(here, 'vitest.setup.ts')],
    reporters: ['verbose'],
    passWithNoTests: true,
  },
});
