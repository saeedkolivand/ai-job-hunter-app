/**
 * lint-staged — pre-commit configuration.
 *
 * Auto-fixes + formats ONLY the staged files (fast, low memory):
 *   1. `eslint --cache --fix` — corrects fixable lint issues so they land *in*
 *      the commit (the only place auto-fix actually reaches the push).
 *   2. `prettier --write` — formats, and has the final say so it never fights
 *      ESLint's stylistic fixes.
 *
 * Scope matters: lint-staged passes only the staged file paths, so this never
 * runs `eslint .` across the 200+ files in the repo (which OOM/SIGKILLs dev
 * machines). The exhaustive, non-fixing gate stays in the pre-push hook
 * (`pnpm lint:strict`, `--max-warnings 0`) and in CI.
 *
 * Markdown: `bump-last-updated.mjs` auto-bumps "Last updated: YYYY-MM-DD" headers
 * BEFORE prettier formatting, so timestamps stay current without manual updates.
 */
export default {
  '**/*.{ts,tsx}': ['eslint --cache --fix', 'prettier --write'],
  '**/*.{js,mjs,cjs}': ['eslint --cache --fix', 'prettier --write'],
  '**/*.md': ['node scripts/bump-last-updated.mjs', 'prettier --write'],
  '**/*.{json,yml,yaml}': ['prettier --write'],
  '**/*.css': ['prettier --write'],
};
