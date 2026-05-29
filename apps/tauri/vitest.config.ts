import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

// Resolve workspace packages to their TypeScript source so tests run without a
// prior `pnpm build:packages` step and coverage is attributed to source files.
const sharedSrc = resolve(__dirname, '../../packages/shared/src');
const promptsSrc = resolve(__dirname, '../../packages/prompts/src');
const uiSrc = resolve(__dirname, '../../packages/ui/src');

export default defineConfig({
  plugins: [react()],
  test: {
    name: 'renderer',
    globals: true,
    environment: 'jsdom',
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    setupFiles: [resolve(__dirname, 'vitest.setup.ts')],
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
      { find: '@ajh/prompts/generate', replacement: resolve(promptsSrc, 'generate.ts') },
      { find: '@ajh/prompts/analyze', replacement: resolve(promptsSrc, 'analyze.ts') },
      {
        find: '@ajh/prompts/context-manager',
        replacement: resolve(promptsSrc, 'context-manager.ts'),
      },
      { find: '@ajh/prompts', replacement: resolve(promptsSrc, 'index.ts') },
      { find: '@ajh/ui', replacement: resolve(uiSrc, 'index.ts') },
      { find: '@', replacement: resolve(__dirname, 'src/renderer') },
    ],
  },
});
