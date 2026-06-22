import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// Load this config as native ESM (note `import.meta.url`, not `__dirname`) so
// `@vitejs/plugin-react` is imported via ESM rather than require()'d during
// vitest's concurrent project setup. The require()-of-ESM path races under
// node 22 (ERR_INTERNAL_ASSERTION, "module is not yet fully loaded"), crashing
// the whole run. Mirrors packages/ui/vitest.{config,storybook.config}.ts.
const here = dirname(fileURLToPath(import.meta.url));

// Resolve workspace packages to their TypeScript source so tests run without a
// prior `pnpm build:packages` step and coverage is attributed to source files.
const sharedSrc = resolve(here, '../../packages/shared/src');
const promptsSrc = resolve(here, '../../packages/prompts/src');
const uiSrc = resolve(here, '../../packages/ui/src');
const translationsSrc = resolve(here, '../../packages/translations/src');
const testIdsSrc = resolve(here, '../../packages/test-ids/src');

export default defineConfig({
  plugins: [react()],
  test: {
    name: 'renderer',
    globals: true,
    environment: 'jsdom',
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    globalSetup: [resolve(here, 'vitest.global-setup.ts')],
    setupFiles: [resolve(here, 'vitest.setup.ts')],
    passWithNoTests: true,
  },
  resolve: {
    alias: [
      // Subpath exports must come before the bare package alias (first match wins).
      { find: '@ajh/shared/ipc', replacement: resolve(sharedSrc, 'ipc/contracts/index.ts') },
      {
        find: '@ajh/shared/language-detection',
        replacement: resolve(sharedSrc, 'language-detection.ts'),
      },
      { find: '@ajh/shared/schemas', replacement: resolve(sharedSrc, 'schemas/index.ts') },
      { find: '@ajh/shared/types', replacement: resolve(sharedSrc, 'types/index.ts') },
      { find: '@ajh/shared/utils', replacement: resolve(sharedSrc, 'utils.ts') },
      { find: '@ajh/shared/ai-models', replacement: resolve(sharedSrc, 'ai-models.ts') },
      { find: '@ajh/shared', replacement: resolve(sharedSrc, 'index.ts') },
      { find: '@ajh/prompts/generate', replacement: resolve(promptsSrc, 'generate/index.ts') },
      { find: '@ajh/prompts/analyze', replacement: resolve(promptsSrc, 'analyze/index.ts') },
      { find: '@ajh/prompts/builder', replacement: resolve(promptsSrc, 'builder/index.ts') },
      {
        find: '@ajh/prompts/context-manager',
        replacement: resolve(promptsSrc, 'context-manager/index.ts'),
      },
      { find: '@ajh/prompts', replacement: resolve(promptsSrc, 'index.ts') },
      { find: '@ajh/ui', replacement: resolve(uiSrc, 'index.ts') },
      { find: '@ajh/translations', replacement: resolve(translationsSrc, 'index.ts') },
      { find: '@ajh/test-ids', replacement: resolve(testIdsSrc, 'index.ts') },
      { find: '@', replacement: resolve(here, 'src/renderer') },
    ],
  },
});
