import { defineConfig } from 'vitest/config';

// Root config orchestrates every workspace test project and owns the single
// aggregated coverage report. Each project (packages/*, apps/desktop) supplies
// its own environment + module aliases via its local vitest.config.ts.
export default defineConfig({
  test: {
    projects: [
      'packages/shared',
      'packages/prompts',
      'packages/test-ids',
      'packages/ui',
      // Storybook browser-test project (headless Chromium via Playwright). Runs
      // every story as a test; selectable on its own with `--project storybook`.
      'packages/ui/vitest.storybook.config.ts',
      'apps/landing',
      'apps/desktop',
      // Browser extension (MV3, jsdom). Also runs standalone in the
      // path-filtered `extension` CI job via turbo; listed here so its tests
      // and sources join the single aggregated coverage report.
      'apps/extension',
      // Node-env project for build/release scripts (e.g. the empty-release-notes
      // guard plugin). Config: scripts/vitest.config.ts.
      'scripts',
    ],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'text-summary', 'json', 'json-summary', 'html'],
      reportsDirectory: './coverage',
      include: [
        'packages/shared/src/**/*.ts',
        'packages/prompts/src/**/*.ts',
        'packages/ui/src/**/*.{ts,tsx}',
        'apps/desktop/src/**/*.{ts,tsx}',
        'apps/landing/src/**/*.{ts,tsx}',
        'apps/extension/src/**/*.ts',
      ],
      exclude: [
        '**/*.d.ts',
        '**/*.test.{ts,tsx}',
        '**/*.spec.{ts,tsx}',
        '**/*.stories.{ts,tsx}',
        '**/index.ts', // barrel re-exports — no logic to cover
        '**/types/**', // pure type declarations
        '**/*.gen.ts', // generated route tree
        'apps/desktop/src/main.tsx', // app entry / bootstrap
        'apps/desktop/src/renderer/main.tsx',
        'apps/desktop/src/renderer/routes/**', // file-based route trees (thin wrappers)
        'apps/desktop/src/renderer/i18n/**', // i18n locale resource bundles
        // Feature pages and shared chrome are presentational composition of the
        // (unit-tested) primitives, service hooks, and stores. They are covered
        // by the Playwright E2E suite rather than unit tests — see e2e/.
        'apps/desktop/src/renderer/features/**',
        'apps/desktop/src/renderer/components/**',
        'apps/desktop/src/TauriWindowControls.tsx', // native window-chrome (E2E)
        '**/test-support.tsx', // shared test harness (not production code)
        '**/mock-client.ts', // test/storybook/web-adapter stub, not runtime code
        // Next.js route shells only — thin composition over the covered
        // components, same rationale as the desktop routes above; rendered
        // output is gated by the landing a11y + parity checks. Scoped to the
        // framework filenames so co-located logic (e.g. app/world/world-config.ts,
        // which has its own tests) stays measured.
        'apps/landing/src/app/**/{page,layout,route,not-found,error,loading,template,default,sitemap,robots,manifest,icon,apple-icon,opengraph-image,twitter-image}.{ts,tsx}',
        'apps/extension/src/manifest.ts', // build-time MV3 manifest, emitted by vite.config
      ],
      // Count every source file matched by `include`, even ones no test imports,
      // so the percentages reflect the whole surface rather than just touched files.
      all: true,
      // Per-area gates. The TS packages and the renderer's logic/data/service
      // layers are held to 80/80/80. Feature pages + shared chrome are excluded
      // above (E2E-covered), so they are neither measured nor gated here.
      thresholds: {
        'packages/{shared,prompts,ui}/src/**': { lines: 80, functions: 80, branches: 80 },
        // Renderer logic/data/service layers: lines + functions held at 80.
        // Branch coverage on these layers sits at ~67% (many defensive
        // `??`/optional-chain fallbacks in async stream + store code); held at 65
        // as an honest floor and raised as those paths get covered.
        'apps/desktop/src/**': { lines: 80, functions: 80, branches: 65 },
      },
    },
  },
});
