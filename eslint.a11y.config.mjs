// Advisory accessibility lint — deliberately SEPARATE from eslint.config.mjs so
// the strict main lint (--max-warnings 0) stays unaffected while jsx-a11y
// findings are surfaced advisorily in CI (see .github/workflows/quality.yml).
// Promote rules into eslint.config.mjs once the renderer is clean.
import jsxA11y from 'eslint-plugin-jsx-a11y';
import tseslint from 'typescript-eslint';

export default [
  {
    // Test fixtures simulate clicks on divs and are not shipped UI; stories are
    // Storybook harnesses (one deliberately renders a label with no control to
    // demo the hint styling), so they are not shipped UI either.
    ignores: [
      '**/dist/**',
      '**/out/**',
      '**/node_modules/**',
      '**/*.gen.ts',
      '**/*.test.tsx',
      '**/*.spec.tsx',
      '**/*.stories.tsx',
    ],
  },
  {
    // The design system is linted alongside the renderer: a primitive's a11y
    // defect is inherited by every consumer, so it is the higher-leverage tree.
    files: ['apps/desktop/src/renderer/**/*.tsx', 'packages/ui/src/**/*.tsx'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    plugins: { 'jsx-a11y': jsxA11y },
    rules: {
      ...jsxA11y.flatConfigs.recommended.rules,
      // `role="list"` on a `list-none` <ul> is deliberate, not redundant: Safari
      // + VoiceOver drop list semantics from a list whose markers are removed.
      'jsx-a11y/no-redundant-roles': ['error', { ul: ['list'] }],
      // autoFocus on the first field of a just-opened dialog is the APG
      // recommendation (initial focus inside the dialog). Sensitivity loss:
      // `ignoreNonDOM` skips autoFocus on EVERY custom component, not just the
      // link dialog's <Input> — an autofocused custom field elsewhere in a
      // non-dialog surface would no longer be reported.
      'jsx-a11y/no-autofocus': ['error', { ignoreNonDOM: true }],
    },
  },
];
