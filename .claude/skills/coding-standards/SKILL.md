---
name: coding-standards
description: General repo rules enforced by ESLint/TypeScript/CI — PRs-only, never bypass lint, import ordering, type imports, path privacy, commit format. Load for any code change.
---

# Coding standards (CI-enforced — violations fail the build)

- **PRs only** — never push to `main`. Branch → commit → push → `gh pr create` → wait for approval. Stale-branch check first (`rtk git fetch origin`).
- **Never bypass ESLint** — no `// eslint-disable`, no `@ts-ignore`. Scoped overrides go in `eslint.config.mjs` with a reason comment. `rtk pnpm lint:strict` runs in CI with `--max-warnings 0`.
- **Path privacy** — never expose absolute local paths, usernames, or home dirs; always repo-relative.
- **Imports** — package entrypoints not deep paths (`@ajh/ui`, not `@/components/ui/*`); group order `node:*` → external → `@ajh/*` → `@/*` → relative; `import type` for pure types. Auto-fix: `rtk pnpm lint:fix`.
- **Commits (commitlint, commit-msg hook)** — subject **lower-case**, ≤100 chars, imperative, no trailing period; body lines ≤200; type ∈ `feat|fix|perf|refactor|ui|style|test|docs|build|ci|chore|revert`. Only `feat/fix/perf` + `BREAKING CHANGE` trigger a release.
- **Tooling** — use the Bash tool; prefix dev commands with `rtk` (`rtk pnpm`, `rtk rg`, `rtk fd`, `rtk bat`). Never `find -exec`, never PowerShell syntax.
- **New IPC capability** — touches 5 files in order: `packages/shared/src/ipc/contracts.ts` → `apps/desktop/src-tauri/src/commands.rs` → `apps/desktop/src/tauri-client.ts` → a `renderer/services/` hook → `services/query-client.ts` query key.
