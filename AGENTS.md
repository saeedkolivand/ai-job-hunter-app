# AI Job Hunter — Agent Rules

**Canonical rules for every AI coding agent on this repo.** Enforced by ESLint, TypeScript, commitlint, and CI — violations block commits and fail the build.

Cursor and Windsurf read this file natively; Copilot does too, but not in every feature — which is why the thin `.github/copilot-instructions.md` pointer stays. Claude Code reads it through the `@AGENTS.md` import at the top of `CLAUDE.md`, which adds Claude-only orchestration (subagent fleet, review gates, model tiering) on top. Cline reads it via the pointer in `.clinerules`.

> **IMPORTANT: edit rules here, never fork them into a tool-specific file.** 13 parallel copies drifted badly enough to teach a stale UI-primitive set and hide a whole app; `pnpm check:agent-system` now guards against a repeat.

---

## Path privacy

Never output absolute paths, usernames, home dirs, drive letters, or temp/IDE paths — anywhere (logs, PRs, commits, docs, screenshots, stack traces). Always repo-relative (`apps/desktop/src-tauri/src/main.rs`); the same rule applies to Git Bash-style paths (`/c/Users/…`).

## Shell & tooling

Use **Bash** (never PowerShell). `rg` not `grep` · `fd` not `find` · `bat` not `cat` · `pnpm` not `npm`/`yarn`. Never `find -exec`.

---

## Architecture

Local-first desktop app, pnpm monorepo. **Tauri is the shell.** Detail → `docs/ARCHITECTURE.md`, status → `docs/ARCHITECTURE_STATUS.md`, principles → `docs/PATTERNS.md` §13.

Workspaces: `packages/{shared,ui,prompts,translations,test-ids}` (contracts · design system · prompt templates · i18n · test IDs) and `apps/{desktop,extension,landing}` (Tauri app · MV3 extension · Next.js site). Boundaries are rule 12 — they are enforced, not stylistic.

Renderer → shell only via `AppClient` (`createTauriInvokeClient()` in `apps/desktop/src/tauri-client/index.ts`). IPC contract: `packages/shared/src/ipc/contracts/`. **Dev:** `pnpm dev`.

---

## Rules (enforced — full config in `eslint.config.mjs`)

0. **PRs only, never push to `main`.** Branch → commit → push → `gh pr create` → wait for approval.
1. **No `window.api` in UI.** Use service hooks from `apps/desktop/src/renderer/services/` (React Query).
2. **i18n from `@ajh/translations`,** never `react-i18next`/`i18next` directly. Init shim: `@/i18n`.
3. **No hardcoded brand colors.** `text-brand`/`bg-brand`/… or `var(--color-brand)`. `[#RRGGBB]` errors.
4. **No inline transition objects.** `import { transition } from '@ajh/ui'`.
5. **Always `@ajh/ui` primitives** — Button, Input, TextArea, NumberField, Dropdown, Switch, ModalShell, ConfirmModal, EmptyState, ErrorState, RowSkeleton/CardSkeleton, GlassCard, SettingsSection, OptionTile, StreamingText. Raw `<button>`/`<select>`/`<textarea>` error (except `<input type=range|file|checkbox|radio|hidden>`). `PageShell` from `@/components/layout/PageShell`; `UpdateBanner` from `@/components/ui/UpdateBanner`.
6. **Package entrypoints, not deep paths.** `@ajh/ui` directly; prefer `React.ComponentProps<typeof X>`.
7. **Import order** (blank line between): `node:*` → external → `@ajh/*` → `@/*` → relative. `pnpm lint:fix`.
8. **`import type` for pure types** (auto-fixed; never suppress).
9. **File placement** under `renderer/`: `features/` (one route), `components/ui/`, `components/layout/`, `services/` (IPC hooks), `lib/` (pure utils + `machines/`), `hooks/`, `providers/`, `store/`. Never import across feature dirs.
10. **State machines** for 3+ states → `lib/machines/` + `useMachine` from `@/hooks/use-machine`.
11. **Remote data via React Query service hooks** — no `useState + useEffect` fetching.
12. **Package boundaries:** `shared` no React/Node · `ui` no Zustand/IPC/routing · `prompts` no UI/`window` · `translations` no app/IPC imports.
13. **Stale-branch check before work:** `git fetch origin && git branch -r | rg $(git branch --show-current)`.
14. **New IPC capability** (5 steps): `ipc/contracts/` → `commands.rs` → `tauri-client.ts` → a `services/` hook → query key in `services/query-client.ts`.
15. **Never bypass ESLint** — no `// eslint-disable`, no `@ts-ignore`. Scoped override in `eslint.config.mjs` with a reason. CI runs `lint:strict --max-warnings 0`.

---

## Quick reference (non-obvious locations)

| What                                       | Where                                                                                             |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| Board registry                             | `scraping/boards/mod.rs` (`SCRAPERS`)                                                             |
| Rust: config/paths · HTTP · errors · spans | `platform/config.rs` · `net/http.rs` · `error.rs` · `observability.rs`                            |
| Design tokens · motion tokens              | `packages/ui/src/css/tokens.css` · `packages/ui/src/lib/motion.ts`                                |
| Docs                                       | `docs/PATTERNS.md` · `docs/DESIGN_SYSTEM.md` · `docs/DEVELOPMENT.md` · `docs/EXPORT_TEMPLATES.md` |

---

## Release & commits

**Manual release** — Actions → "🚀 Release" → `action: release`. Never tag/bump manually; semantic-release derives the bump (`release.config.mjs`). `feat:` minor · `fix:`/`perf:` patch · `BREAKING CHANGE` minor while 0.x · `refactor/ui/style/test/docs/build/ci/chore/revert` none.

**Commit format** (commitlint, `commit-msg` hook): lowercase subject (acronyms too: `URL`→`url`), ≤100 chars, imperative, no trailing period; body lines ≤200 chars, blank line after subject; type ∈ `feat fix perf refactor ui style test docs build ci chore revert`.

---

## Code intelligence: graphify + codegraph

Prefer the graphs over raw `rg`/`fd`/file-browsing for "where/what calls/impact" and architecture questions. **codegraph** (structural, zero-token, auto-synced via file watcher) — MCP `codegraph_explore` first; CLI `codegraph callers/callees/impact/query`. **graphify** (semantic) — MCP `query_graph`/`shortest_path`/`get_*`; CLI `graphify query/path/explain`; broad nav `graphify-out/wiki/index.md`. Routing: structural → codegraph · semantic → graphify · `rg` only when neither answers. **After code changes:** `graphify update .` (codegraph syncs itself).

---

## Review conventions (all tools)

Agents that cannot spawn Claude Code's subagent fleet still follow the same flow: route the change to the owning domain, implement → review → tests if logic changed → docs sync last. Only HIGH/CRITICAL findings block; style and naming are advisory. The ownership table and the full pipeline live in `CLAUDE.md`.
