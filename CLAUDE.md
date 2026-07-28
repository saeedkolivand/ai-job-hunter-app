# AI Job Hunter — Project Rules for AI Assistants

Single source of truth for AI assistants. Rules are enforced by ESLint, TypeScript, commitlint, and CI — violations block commits and fail the build. This file is an **index**: terse rules inline, detail behind pointers.

---

## Auto-invoked skills (always on — no slash command)

Re-injected every session by the `SessionStart` hook (`.claude/hooks/style-policy.mjs`).

- **`ponytail`** — lazy-senior-dev default: laziest solution that works (YAGNI, stdlib/native over deps). Intensity `full`; off: `stop ponytail` / `normal mode`. Source: `ponytail@ponytail` plugin.
- **`grill-with-docs`** — before finalizing any non-trivial plan/design (incl. before `ExitPlanMode`), stress-test it against the domain model + ADRs. Skip for trivial/one-line/docs changes. Source: `~/.claude/skills/grill-with-docs/SKILL.md`.

## Path privacy

Never output absolute paths, usernames, home dirs, drive letters, or temp/IDE paths — anywhere. Always repo-relative (`apps/desktop/src-tauri/src/main.rs`). Git Bash form: `/c/Users/…`.

## Shell & tooling

Use the **Bash tool** (never PowerShell). `rg` not `grep` · `fd` not `find` · `bat` not `cat` · `pnpm` not `npm`/`yarn`. Never `find -exec`.

---

## Architecture

Local-first desktop app, pnpm monorepo. **Tauri is the shell.** Detail → `docs/ARCHITECTURE.md`, status → `docs/ARCHITECTURE_STATUS.md`, principles → `docs/PATTERNS.md` §13.

```text
packages/shared       ← IPC contracts, Zod schemas, shared types (no UI, no Node)
packages/ui           ← React component library + design system → @ajh/ui (no app logic)
packages/prompts      ← AI prompt templates, provider-aware + locale-driven (pure TS, zero deps)
packages/translations ← i18next + en/de resources → @ajh/translations (no app/IPC deps)
packages/test-ids     ← central TEST_IDS map → @ajh/test-ids
apps/desktop           ← Tauri app: Rust core (scraping, login, documents, AI) + React renderer
apps/extension         ← MV3 browser extension (Chrome + Firefox): job import + opt-in autofill over the loopback bridge
```

Renderer → shell only via `AppClient` (`createTauriInvokeClient()` in `apps/desktop/src/tauri-client/index.ts`). IPC contract: `packages/shared/src/ipc/contracts/`. **Dev:** `pnpm dev`.

---

## Rules (enforced — full config in `eslint.config.mjs`)

0. **PRs only, never push to `main`.** Branch → commit → push → `gh pr create` → wait for approval.
1. **No `window.api` in UI.** Use service hooks from `apps/desktop/src/renderer/services/` (React Query).
2. **i18n from `@ajh/translations`,** never `react-i18next`/`i18next` directly. Init shim: `@/i18n`.
3. **No hardcoded brand colors.** `text-brand`/`bg-brand`/… or `var(--color-brand)`. `[#RRGGBB]` errors.
4. **No inline transition objects.** `import { transition } from '@ajh/ui'`.
5. **Always `@ajh/ui` primitives** — Button, Input, TextArea, NumberField, SelectDropdown, Switch, ModalShell, ConfirmModal, EmptyState, ErrorState, RowSkeleton/CardSkeleton, GlassCard, SettingsSection, OptionTile, StreamingText. Raw `<button>`/`<select>`/`<textarea>` error (except `<input type=range|file|checkbox|radio|hidden>`). `PageShell` from `@/components/layout/PageShell`; `UpdateBanner` from `@/components/ui/UpdateBanner`.
6. **Package entrypoints, not deep paths.** `@ajh/ui` directly; prefer `React.ComponentProps<typeof X>`.
7. **Import order** (blank line between): `node:*` → external → `@ajh/*` → `@/*` → relative. `pnpm lint:fix`.
8. **`import type` for pure types** (auto-fixed; never suppress).
9. **File placement** under `renderer/`: `features/` (one route), `components/ui/`, `components/layout/`, `services/` (IPC hooks), `lib/` (pure utils + `machines/`), `hooks/`, `providers/`, `store/`. Never import across feature dirs.
10. **State machines** for 3+ states → `lib/machines/` + `useMachine` from `@/hooks/use-machine`.
11. **Remote data via React Query service hooks** — no `useState + useEffect` fetching.
12. **Package boundaries:** `shared` no React/Node · `ui` no Zustand/IPC/routing · `prompts` no UI/`window` · `translations` no app/IPC imports.
13. **Stale-branch check before work:** `git fetch origin && git branch -r | grep $(git branch --show-current)`.
14. **New IPC capability** (5 steps): `contracts.ts` → `commands.rs` → `tauri-client.ts` → a `services/` hook → query key in `services/query-client.ts`.
15. **Never bypass ESLint** — no `// eslint-disable`, no `@ts-ignore`. Scoped override in `eslint.config.mjs` with a reason. CI runs `lint:strict --max-warnings 0`.

---

## Quick reference

| What                                       | Where                                                                                                                    |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| IPC contract / Tauri commands / TS client  | `packages/shared/src/ipc/contracts/` · `apps/desktop/src-tauri/src/commands/` · `apps/desktop/src/tauri-client/index.ts` |
| Service hooks                              | `apps/desktop/src/renderer/services/`                                                                                    |
| UI package / design tokens / motion tokens | `packages/ui/src/index.ts` · `packages/ui/src/css/tokens.css` · `packages/ui/src/lib/motion.ts`                          |
| State machines                             | `apps/desktop/src/renderer/lib/machines/`                                                                                |
| i18n                                       | `@ajh/translations`; init shim `apps/desktop/src/renderer/i18n/index.ts`                                                 |
| Rust: config/paths · HTTP · errors · spans | `platform/config.rs` · `net/http.rs` · `error.rs` · `observability.rs`                                                   |
| Board registry                             | `scraping/boards/mod.rs` (`SCRAPERS`)                                                                                    |
| Docs                                       | `docs/PATTERNS.md` · `docs/DESIGN_SYSTEM.md` · `docs/DEVELOPMENT.md` · `docs/EXPORT_TEMPLATES.md`                        |

---

## Release & commits

**Manual release** — Actions → "🚀 Release" → `action: release`. Never tag/bump manually; semantic-release derives the bump (`release.config.mjs`). `feat:` minor · `fix:`/`perf:` patch · `BREAKING CHANGE` minor while 0.x · `refactor/ui/style/test/docs/build/ci/chore/revert` none.

**Commit format** (commitlint, `commit-msg` hook): lowercase subject (acronyms too: `URL`→`url`), ≤100 chars, imperative, no trailing period; body lines ≤200 chars, blank line after subject; type ∈ `feat fix perf refactor ui style test docs build ci chore revert`.

---

## Code intelligence: graphify + codegraph

Prefer the graphs over raw `rg`/`fd`/file-browsing for "where/what calls/impact" and architecture questions. **codegraph** (structural, zero-token, auto-synced via file watcher) — MCP `codegraph_explore` first; CLI `codegraph callers/callees/impact/query`. **graphify** (semantic) — MCP `query_graph`/`shortest_path`/`get_*`; CLI `graphify query/path/explain`; broad nav `graphify-out/wiki/index.md`. Routing: structural → codegraph · semantic → graphify · `rg` only when neither answers. **After code changes:** `graphify update .` (codegraph syncs itself).

---

## Agent system

Domain pairs — a write-capable **author** implements, an independent **critic** audits (authors never approve their own work). Full pipeline detail → `.claude/` (agents/skills/commands), routing → `.claude/review-routes.json`, knowledge base → `docs/knowledge/`. Drift guard: `pnpm check:agent-system` (pre-push + CI).

| Touched area                                                | Author                          | Critic(s)                                        |
| ----------------------------------------------------------- | ------------------------------- | ------------------------------------------------ |
| React renderer                                              | `frontend-author`               | `frontend-reviewer` · `ui-ux-expert` (visual/UX) |
| Landing site (apps/landing — Next.js 16 static export)      | `frontend-author`               | `frontend-reviewer` · `ui-ux-expert` (visual)    |
| Rust/Tauri backend                                          | `rust-backend-author`           | `rust-backend-architect`                         |
| Resume/export, DocumentModel, templates, theme, locale      | `pdf-docx-generator`            | `resume-export-expert`                           |
| ATS scoring, job analysis, matching, cover-letter relevance | `job-match-author`              | `job-match-expert`                               |
| AI providers, routing, embeddings, prompts, streaming       | `ai-provider-author`            | `ai-provider-expert`                             |
| Scraping, browser automation, registries                    | `scraping-applier-author`       | `scraping-applier-expert`                        |
| Browser extension + bridge + protocol                       | `extension-author`              | `extension-reviewer`                             |
| Tests                                                       | `test-author`                   | `testing-reviewer`                               |
| Code quality (on-demand)                                    | `code-quality-author`           | `code-quality-reviewer`                          |
| Docs / knowledge / ADRs / lessons / release                 | `project-steward` (sole writer) | `project-steward`                                |

Cross-cutting critics (no author — fixes route to the owning domain author): `tauri-security-reviewer` (Secondary on risk-bearing changes), `performance-profiler` (perf-sensitive only), `cleanup` (dead-code, pre-PR), `pr-reviewer` (strict pre-PR gate), `finding-verifier` (per-finding judge for /review, ≤5 spawns). The dormant GL fleet + skills live in `.claude/dormant/` (no GL surface since ADR-0017). Every critic loads `.claude/skills/critic-contract/SKILL.md`; every author loads `author-contract`.

**Per-change defaults (cost-tiered — ADR-025):**

- **Trivial diffs** (docs/config/comments/renames/single-file ≤10 lines): the **main session edits directly** — no swarm. The deterministic Tier-0 Stop gate + the pre-push AI review still cover them.
- **Single-domain, non-risk changes**: author → **ONE sibling critic** (resolve HIGH/CRITICAL; LOW/MEDIUM advisory) → if testable logic, `test-author` → `testing-reviewer`.
- **Risk-bearing** (any `tauri-security-reviewer` secondary glob) **or multi-domain**: full trio incl. security critic (≤3 critics/task).
- `project-steward` closes **once per PR** (docs/lessons sync + `graphify update .`), not once per change. Context flows via `.claude/scratch/<task>.md`. Orchestrate all sub-agents from the main session (agents can't call agents).
- **Before a PR:** `/review-security` (HIGH/CRITICAL block) then `/review` (🔴+🟠 block) — both complement CodeRabbit.

**Review gates:** the Stop hook (`.claude/hooks/review-gate.mjs`) is **deterministic-only** (ast-grep Tier-0 + ledger re-emits — near-free, blocks on introduced arch violations and unresolved findings). The **LLM review runs at pre-push** (`scripts/pre-push-review.mjs`, ADR-0008 ratchet) and in CI; metrics in `.claude/.review-metrics.jsonl` now include token usage + cost.

**Hard rules:**

- Non-trivial source changes go through a domain author via `Agent`; the main session edits directly only for trivial diffs (above), `CLAUDE.md`, `.claude/**` meta-config, and plan files.
- **Lessons** (`.claude/memory/lessons.jsonl`) — only `project-steward` writes; others propose via `LESSON · category · Context/Decision/Outcome`.
- **Cross-session recall** — agents may call `mcp__mcp-search` (claude-mem plugin; absent if not installed). Honor `docs/` path-privacy + `<private>` for PII.

**Model & effort tiering** (per-agent frontmatter; aliases track the current family): Opus for last-line critics — `pr-reviewer` + `tauri-security-reviewer` pin `effort: xhigh`; the other Opus critics (`rust-backend-architect`, `ai-provider-expert`, `job-match-expert`) run `high`. Sonnet for authors + balanced critics (default `high`). Haiku for `project-steward` + `finding-verifier` (`low` fine for mechanical runs). Escalate a spawn to Opus for genuinely hard work; to Fable only for hard, ambiguous, cross-domain work — never as a frontmatter default, never for security review.

**Context priority:** codegraph/graphify → source → `docs/knowledge/` → lessons. Read the minimum; stop at ~90% confidence.
