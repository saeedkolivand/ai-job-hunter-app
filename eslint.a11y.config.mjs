// Advisory accessibility lint — deliberately SEPARATE from eslint.config.mjs so
// the strict main lint (--max-warnings 0) stays unaffected while jsx-a11y
// findings are surfaced advisorily in CI (see .github/workflows/quality.yml).
// Promote rules into eslint.config.mjs once the renderer is clean.
import jsxA11y from 'eslint-plugin-jsx-a11y';
import tseslint from 'typescript-eslint';

export default [
  {
    // Test fixtures simulate clicks on divs and are not shipped UI.
    ignores: [
      '**/dist/**',
      '**/out/**',
      '**/node_modules/**',
      '**/*.gen.ts',
      '**/*.test.tsx',
      '**/*.spec.tsx',
    ],
  },
  {
    files: ['apps/desktop/src/renderer/**/*.tsx'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    plugins: { 'jsx-a11y': jsxA11y },
    rules: jsxA11y.flatConfigs.recommended.rules,
  },
];
