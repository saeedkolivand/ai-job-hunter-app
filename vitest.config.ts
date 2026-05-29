import { defineConfig } from 'vitest/config';

// Root config orchestrates every workspace test project and owns the single
// aggregated coverage report. Each project (packages/*, apps/tauri) supplies
// its own environment + module aliases via its local vitest.config.ts.
export default defineConfig({
  test: {
    projects: ['packages/shared', 'packages/prompts', 'packages/ui', 'apps/tauri'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'text-summary', 'json', 'html'],
      reportsDirectory: './coverage',
      include: [
        'packages/shared/src/**/*.ts',
        'packages/prompts/src/**/*.ts',
        'packages/ui/src/**/*.{ts,tsx}',
      ],
      exclude: [
        '**/*.d.ts',
        '**/*.test.{ts,tsx}',
        '**/*.spec.{ts,tsx}',
        '**/*.stories.{ts,tsx}',
        '**/index.ts', // barrel re-exports — no logic to cover
        '**/types/**', // pure type declarations
      ],
      // Count every source file matched by `include`, even ones no test imports,
      // so the percentages reflect the whole surface rather than just touched files.
      all: true,
      thresholds: {
        lines: 80,
        functions: 80,
        branches: 80,
      },
    },
  },
});
