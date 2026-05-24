/**
 * lint-staged — pre-commit configuration.
 *
 * Runs ONLY formatting on commit (fast, zero memory pressure).
 * ESLint, TypeScript, and tests run in the pre-push hook instead.
 *
 * Rationale: running eslint on 200+ staged files in parallel causes OOM/SIGKILL
 * on developer machines. Format-on-commit is instant and non-blocking. Lint
 * errors are caught by the editor in real-time and by pre-push before reaching
 * the remote — the right gate for that check.
 */
export default {
  '**/*.{ts,tsx}': ['prettier --write', 'git add'],
  '**/*.{js,mjs,cjs}': ['prettier --write', 'git add'],
  '**/*.{json,md,yml,yaml}': ['prettier --write', 'git add'],
  '**/*.css': ['prettier --write', 'git add'],
};
