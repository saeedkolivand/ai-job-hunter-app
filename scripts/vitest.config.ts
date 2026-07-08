import { defineConfig } from 'vitest/config';

// Node-environment project for the repo's build/release scripts (plain .cjs/.mjs,
// no renderer/jsdom aliases). Registered in the root vitest `projects` list so
// `pnpm test` picks these up alongside the package + app suites.
export default defineConfig({
  test: {
    name: 'scripts',
    environment: 'node',
    include: ['**/*.test.mjs'],
  },
});
