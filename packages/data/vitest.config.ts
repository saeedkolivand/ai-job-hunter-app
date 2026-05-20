import { defineConfig } from 'vitest/config';
import { resolve } from 'node:path';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    include: ['src/**/*.{test,spec}.ts'],
    reporters: ['verbose'],
  },
  resolve: {
    alias: {
      '@ajh/shared': resolve('../shared/src/index.ts'),
      '@ajh/core': resolve('../core/src/index.ts'),
      '@ajh/ai': resolve('../ai/src/index.ts'),
    },
  },
});
