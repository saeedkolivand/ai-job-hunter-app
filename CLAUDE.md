# AI Job Hunter — Project Rules for AI Assistants

Single source of truth for AI assistants. Rules below are enforced by ESLint, TypeScript, commitlint, and CI — violations block commits and fail the build. This file is an **index**: terse rules inline, detail behind pointers.

---

## Auto-invoked skills (always on — no slash command)

Re-injected every session by the `SessionStart` hook (`.claude/hooks/style-policy.mjs`, wired in `.claude/settings.json`), so activation is deterministic even if this file is summarized.

- **`ponytail`** — lazy-senior-dev default: laziest solution that works (YAGNI, stdlib/native over deps, one line over fifty). Intensity `full`; `/ponytail lite|full|ultra`. Off: `stop ponytail` / `normal mode`. Source: `ponytail@ponytail` plugin.
- **`grill-with-docs`** — before finalizing any non-trivial plan/design (incl. before `ExitPlanMode`), stress-test it against the domain model + ADRs, one question at a time. Skip for trivial/one-line/docs changes. Source: `~/.claude/skills/grill-with-docs/SKILL.md`.

---

## Path privacy

Never output absolute paths, usernames, home dirs, drive letters, or temp/IDE paths — anywhere (logs, stack traces, PRs, commits, markdown). Always repo-relative (`apps/desktop/src/main.rs`, not `C:\Users\…`). Git Bash form: `/c/Users/…`.

## Shell & tooling

- Use the **Bash tool** (never PowerShell). Prefix **every** command with `rtk` (`rtk pnpm …`, `rtk git …`).
- `rtk rg` not `grep` · `rtk fd` not `find` · `rtk bat` not `cat` · `rtk pnpm` not `npm`/`yarn`. Never `find -exec`.
- Meta: `rtk gain` (savings) · `rtk discover` (missed opportunities).

---

## Architecture

Local-first desktop app, pnpm monorepo. **Tauri is the shell.** Full detail → `docs/ARCHITECTURE.md`, status → `docs/ARCHITECTURE_STATUS.md`, principles → `docs/PATTERNS.md` §13.

```
packages/shared       ← IPC contracts, Zod schemas, shared types (no UI, no Node)
packages/ui           ← React component library + design system → @ajh/ui (no app logic)
packages/prompts      ← AI prompt templates, provider-aware + locale-driven (pure TS, zero deps)
packages/translations ← i18next + en/de resources → @ajh/translations (no app/IPC deps)
packages/test-ids     ← central TEST_IDS map → @ajh/test-ids
apps/desktop            ← Tauri app: Rust core (scraping, login, documents, AI) + React renderer
```

Renderer → shell only via `AppClient` (`createTauriInvokeClient()` in `apps/desktop/src/tauri-client.ts`). IPC contract: `packages/shared/src/ipc/contracts.ts`. **Dev:** `pnpm dev`.

---

## Rules (enforced — full config in `eslint.config.mjs`)

0. **PRs only, never push to `main`.** Branch → commit → push → `gh pr create` → wait for approval.
1. **No `window.api` in UI.** Use service hooks from `apps/desktop/src/renderer/services/` (React Query).
2. **i18n from `@ajh/translations`,** never `react-i18next`/`i18next` directly. Init shim: `@/i18n`.
3. **No hardcoded brand colors.** Use `text-brand`/`bg-brand`/… or `var(--color-brand)`. `[#RRGGBB]` errors.
4. **No inline transition objects.** `import { transition } from '@ajh/ui'` (`.fast`/`.normal`/`.spring`/…).
5. **Always `@ajh/ui` primitives** — Button, Input, TextArea, NumberField, SelectDropdown, Switch, ModalShell, ConfirmModal, EmptyState, ErrorState, RowSkeleton/CardSkeleton, GlassCard, SettingsSection, OptionTile, StreamingText. Raw `<button>`/`<select>`/`<textarea>` error (except `<input type=range|file|checkbox|radio|hidden>`). `PageShell` from `@/components/layout/PageShell`; `UpdateBanner` from `@/components/ui/UpdateBanner`.
6. **Package entrypoints, not deep paths.** `@ajh/ui` directly; prefer `React.ComponentProps<typeof X>`.
7. **Import order** (blank line between): `node:*` → external → `@ajh/*` → `@/*` → relative. `rtk pnpm lint:fix`.
8. **`import type` for pure types** (auto-fixed; never suppress).
9. **File placement** under `renderer/`: `features/` (one route), `components/ui/` (re-exports), `components/layout/` (chrome), `services/` (IPC hooks), `lib/` (pure utils + `machines/`), `hooks/`, `providers/`, `store/`. Never import across feature dirs.
10. **State machines** for 3+ states → `lib/machines/` + `useMachine` from `@/hooks/use-machine`.
11. **Remote data via React Query service hooks** — no `useState + useEffect` fetching.
12. **Package boundaries:** `shared` no React/Node · `ui` no Zustand/IPC/routing · `prompts` no UI/`window` · `translations` no app/IPC imports.
13. **Stale-branch check before work:** `rtk git fetch origin && rtk git branch -r | grep $(git branch --show-current)`.
14. **New IPC capability** (5 steps): `contracts.ts` → `commands.rs` → `tauri-client.ts` → a `services/` hook → query key in `services/query-client.ts`.
15. **Never bypass ESLint** — no `// eslint-disable`, no `@ts-ignore`. Scoped override in `eslint.config.mjs` with a reason. CI runs `lint:strict --max-warnings 0`.

---

## Quick reference

| What                                       | Where                                                                                                     |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------- |
| IPC contract / Tauri commands / TS client  | `packages/shared/src/ipc/contracts.ts` · `src-tauri/src/commands.rs` · `apps/desktop/src/tauri-client.ts` |
| Service hooks                              | `apps/desktop/src/renderer/services/`                                                                     |
| UI package / design tokens / motion tokens | `packages/ui/src/index.ts` · `packages/ui/src/css/tokens.css` · `packages/ui/src/lib/motion.ts`           |
| State machines                             | `apps/desktop/src/renderer/lib/machines/`                                                                 |
| i18n                                       | `@ajh/translations`; init shim `apps/desktop/src/renderer/i18n/index.ts`                                  |
| Rust: config/paths · HTTP · errors · spans | `platform/config.rs` · `net/http.rs` · `error.rs` · `observability.rs`                                    |
| Board registry                             | `scraping/boards/mod.rs` (`SCRAPERS`) — no applier registry (apply engine removed)                        |
| Docs                                       | `docs/PATTERNS.md` · `docs/DESIGN_SYSTEM.md` · `docs/DEVELOPMENT.md` · `docs/EXPORT_TEMPLATES.md`         |

---

## Release & commits

**Manual release** — Actions → "🚀 Release" → `action: release`. Nothing auto-runs on push to `main`; do not tag/bump manually. semantic-release derives the bump. Config: `.releaserc.json`, `commitlint.config.mjs`.

| Commit prefix | Release                |
| ------------- | ---------------------- |
| `feat:`       | minor · `fix:`/`perf:` | patch · `BREAKING CHANGE` footer | major · `refactor/ui/style/test/docs/build/ci/chore/revert` | none |

**Commit format** (commitlint, `commit-msg` hook — fails the commit): lowercase subject (acronyms too: `URL`→`url`), ≤100 chars, imperative, no trailing period; body lines ≤200 chars, blank line after subject; type ∈ `feat fix perf refactor ui style test docs build ci chore revert`.

---

## Code intelligence: graphify + codegraph

Two graphs back codebase questions — **prefer them over raw `rg`/`fd`/file-browsing** for "where/what calls/impact" and architecture questions.

- **codegraph** (structural, zero-token, SQLite `.codegraph/`, auto-synced) — symbols, calls, imports, impact. MCP `codegraph_explore` first; CLI `codegraph callers|callees|impact|query`. For "who calls / what breaks if I change X".
- **graphify** (semantic, `graphify-out/`) — meaning, rationale, cross-doc. MCP `query_graph`/`shortest_path`/`get_*` (`.mcp.json` → `graphify-mcp`); CLI `graphify query|path|explain`. Broad nav: `graphify-out/wiki/index.md`; `GRAPH_REPORT.md` only for whole-architecture review.
- **Routing:** structural → codegraph · semantic/rationale → graphify · `rg`/`fd` only when neither answers.
- **After code changes:** `graphify update .` then `codegraph sync`.

---

## Knowledge base & agent system

Full pipeline, model tiering, and rationale → **`.claude/`** (agents/skills/commands), routing → **`.claude/review-routes.json`**, visual map → **`landing/agent-system.html`**, knowledge base → **`docs/knowledge/`** (thin pointers into source — no copied literals). The drift guard `pnpm check:agent-system` (pre-push + CI) keeps agents ⇄ routes ⇄ this file ⇄ explainer ⇄ docs in sync.

**Default: every change auto-routes through the agent fleet** — no slash command needed. Each domain is a **pair**: a write-capable **author** implements, an independent **critic** audits (authors never approve their own work). Pick by touched area:

| Touched area                                                | Author                          | Critic(s)                                        |
| ----------------------------------------------------------- | ------------------------------- | ------------------------------------------------ |
| React renderer                                              | `frontend-author`               | `frontend-reviewer` · `ui-ux-expert` (visual/UX) |
| Rust/Tauri backend                                          | `rust-backend-author`           | `rust-backend-architect`                         |
| Resume/export, DocumentModel, templates, theme, locale      | `pdf-docx-generator`            | `resume-export-expert`                           |
| ATS scoring, job analysis, matching, cover-letter relevance | `job-match-author`              | `job-match-expert`                               |
| AI providers, routing, embeddings, prompts, streaming       | `ai-provider-author`            | `ai-provider-expert`                             |
| Scraping, browser automation, registries                    | `scraping-applier-author`       | `scraping-applier-expert`                        |
| Browser extension + bridge + protocol                       | `extension-author`              | `extension-reviewer`                             |
| Tests                                                       | `test-author`                   | `testing-reviewer`                               |
| Code quality (on-demand)                                    | `code-quality-author`           | `code-quality-reviewer`                          |
| Docs / knowledge / ADRs / lessons / release                 | `project-steward` (sole writer) | `project-steward`                                |

Cross-cutting critics (no author — fixes route to the owning domain author): `tauri-security-reviewer` (default Secondary on any risk-bearing change), `performance-profiler` (perf-sensitive only), `cleanup` (dead-code, always just before steward), `pr-reviewer` (strict pre-PR gate).

**Per-change sequence** (skip what doesn't apply): author implements → sibling critic audits (resolve HIGH/CRITICAL; LOW/MEDIUM advisory) → if testable logic, `test-author` → `testing-reviewer` → `cleanup` → `project-steward` closes (docs/lessons sync + `graphify update .` + `codegraph sync`). Context flows via the per-task handoff file `.claude/scratch/<task>.md`. **≤3 critics/task.** Critic count scales with risk: trivial diffs rely on the Stop review-gate alone; small single-domain diffs get ONE sibling critic; the full trio (incl. security) only for risk-bearing/multi-domain changes. Orchestrate all sub-agents from the main session (agents can't call agents). **Before a PR:** `/review-security` (`tauri-security-reviewer`) — HIGH/CRITICAL block — then `/review` (`pr-reviewer`) as the final gate (🔴+🟠 block); both complement CodeRabbit.

**Hard rules:**

- **Main session never edits source directly** — delegate all code changes to a domain author via `Agent`. Exceptions: `CLAUDE.md`, `.claude/**` meta-config, plan files, single-char typo fixes.
- **Trivial diffs skip the swarm** (docs/config/rename/one-liners) — the Stop review-gate (`.claude/hooks/review-gate.mjs`) reviews the real diff regardless; only HIGH/CRITICAL block, once per finish-chain, inert in plan mode.
- **Lessons** (`.claude/memory/lessons.jsonl`) — only `project-steward` writes; others propose via `LESSON · category · Context/Decision/Outcome`.
- **Cross-session recall** — all agents may call `mcp__mcp-search` (claude-mem: `search`/`timeline`/`get_observations`/`memory_search`/`memory_context`) for prior-session context. Provided by the user-installed `thedotmack/claude-mem` plugin, not the repo — absent if the plugin isn't installed (the allowlist entry then just no-ops). Honor `docs/` path-privacy + `<private>` for PII.

**Model tiering** (per agent `model:` frontmatter): Opus for last-line critics (`rust-backend-architect`, `tauri-security-reviewer`, `ai-provider-expert`, `job-match-expert`, `pr-reviewer`); Sonnet for authors + balanced critics; Haiku for `project-steward`. Authors escalate to Opus per spawn for genuinely hard work (Rust concurrency/`unsafe`, new provider streaming, schema migration). Use extended thinking for architecture/security/concurrency/data-loss; normal effort for routine UI/docs/renames.

**Context priority:** codegraph/graphify → source → `docs/knowledge/` → lessons. Read the minimum; stop at ~90% confidence.
