import { defineConfig } from 'vitest/config';
import { resolve } from 'node:path';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    include: ['packages/*/src/**/*.{test,spec}.ts'],
    exclude: ['**/node_modules/**', '**/dist/**', '**/e2e/**'],
    passWithNoTests: true,
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      include: ['packages/*/src/**/*.ts'],
      exclude: [
        '**/*.d.ts',
        '**/*.test.ts',
        '**/*.spec.ts',
        '**/index.ts', // barrel re-exports — no logic to cover
      ],
      // thresholds: {
      //   lines: 60,
      //   functions: 60,
      //   branches: 50,
      // },
    },
  },
  resolve: {
    alias: {
      '@ajh/shared': resolve('./packages/shared/src/index.ts'),
      '@ajh/core': resolve('./packages/core/src/index.ts'),
      '@ajh/ai': resolve('./packages/ai/src/index.ts'),
      '@ajh/data': resolve('./packages/data/src/index.ts'),
    },
  },
});
