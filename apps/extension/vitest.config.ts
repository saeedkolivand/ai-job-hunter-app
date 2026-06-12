import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { defineConfig } from 'vitest/config';

const here = dirname(fileURLToPath(import.meta.url));

// Resolve @ajh/shared to its TypeScript source so tests run without a
// prior build step and Zod schemas are available directly.
const sharedSrc = resolve(here, '../../packages/shared/src');

export default defineConfig({
  test: {
    name: 'extension',
    globals: true,
    // jsdom for DOM/storage tests; bridge tests use their own WebSocket mock.
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    setupFiles: [resolve(here, 'vitest.setup.ts')],
    passWithNoTests: true,
  },
  resolve: {
    alias: [
      { find: '@ajh/shared/ipc', replacement: resolve(sharedSrc, 'ipc/contracts/index.ts') },
      { find: '@ajh/shared', replacement: resolve(sharedSrc, 'index.ts') },
    ],
  },
});
