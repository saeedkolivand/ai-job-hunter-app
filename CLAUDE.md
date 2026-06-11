# AI Job Hunter — Project Rules for AI Assistants

Rules enforced by ESLint, TypeScript, and CI — violations block commits and fail the build.

---

## Auto-Invoked Skills (on by default — no slash command needed)

These skills are active automatically every session. Invoke them through the **Skill tool** without waiting for the user to type the slash command. A `SessionStart` hook (`.claude/hooks/style-policy.mjs`, wired in `.claude/settings.json`) re-injects this policy at the start of every session for deterministic activation — independent of this file being read or summarized. It fires only at session start, so the `stop caveman` off-switch still applies within a session.

### `caveman` — default output style (always on)

- At the **start of every session**, invoke `Skill(caveman)` and stay in caveman mode for all responses.
- Ultra-terse: drop articles / filler / pleasantries; keep **all** technical substance, code blocks, and exact error text.
- Honor the skill's **auto-clarity exception** — switch to normal prose for security warnings, irreversible-action confirmations, and multi-step sequences, then resume caveman.
- Off-switch: revert to normal prose only when the user says `stop caveman` / `normal mode`.
- Source: `~/.claude/skills/caveman/SKILL.md`.

### `grill-with-docs` — automatic before any plan is finalized

- Whenever you are about to present a plan, design, or multi-step approach (including before `ExitPlanMode`), first invoke `Skill(grill-with-docs)` to stress-test it against the repo's domain model and documented decisions — one question at a time, with a recommended answer each.
- Cross-reference terms against the repo glossary / `docs/` + ADRs (`docs/knowledge/`, `docs/PATTERNS.md`, `docs/ARCHITECTURE.md`); sharpen fuzzy language; capture resolved terms inline.
- Skip only for trivial / one-line / docs-config changes where there is no design decision to grill.
- Source: `~/.claude/skills/grill-with-docs/SKILL.md`.

---

## Path Privacy

- Never expose real local file system paths
- Never output absolute Windows, macOS, or Linux paths
- Always use repository-relative paths

❌ `C:\Users\username\project\apps\tauri\src\main.rs`
❌ `/home/username/project/apps/api/src/server.ts`
❌ `~/Projects/app/src/index.ts`

✅ `apps/tauri/src/main.rs`
✅ `apps/api/src/server.ts`

- Never expose usernames, home directories, drive letters, workspace roots, temp paths, or IDE-specific paths
- Sanitize absolute paths in logs, stack traces, screenshots, terminal output, PRs, commits, comments, and markdown
- Prefer repository-root-relative paths. If needed, use: `file:///app/<relative-path>`

---

## Shell & Tooling

- **Always use the Bash tool** (never PowerShell)
- **Prefix EVERY command with `rtk`** — `rtk pnpm build`, `rtk git status`, `rtk rg foo`, `rtk fd src`, `rtk bat file.ts`
- Meta commands: `rtk gain` (savings stats) · `rtk discover` (missed opportunities)
- Use `rtk rg` not `grep` · `rtk fd` not `find` · `rtk bat` not `cat` · `rtk pnpm` not `npm`/`yarn`
- Never `find -exec`, never PowerShell syntax
- Git Bash paths: `/c/Users/...` not `C:\Users\...`

---

## Architecture

Local-first desktop app in a pnpm monorepo. **Tauri is the shell.**

```
packages/shared       ← IPC contracts, Zod schemas, shared types (no UI, no Node)
packages/ui           ← React component library + design system (no app logic)
packages/prompts      ← AI prompt templates — provider-aware + locale-driven (pure TS, zero deps)
packages/translations ← i18next config + UI translation resources (en/de) → @ajh/translations (no app/IPC deps)
apps/tauri            ← Tauri app: Rust core (scraping, login, documents, AI) + React renderer
```

Renderer → shell communication: `AppClient` context only.

- `createTauriInvokeClient()` in `apps/tauri/src/tauri-client.ts`

IPC contract: `packages/shared/src/ipc/contracts.ts`.

**Default dev:** `pnpm dev` → Tauri app.

---

## Rules

### 0. PRs only — never push to main

`rtk git checkout -b feat/name` → commit → `rtk git push -u origin <branch>` → `rtk gh pr create` → wait for approval.

### 1. Ports & Adapters — no `window.api` in UI

Use service hooks from `apps/tauri/src/renderer/services/`. They wrap IPC with React Query.
ESLint errors on `window.api.*` in features/, routes/, or components/.

### 2. i18n — import from `@ajh/translations`, never `react-i18next` directly

`useTranslation` / `TFunction` come from `@ajh/translations` (the translations package). The renderer init shim `@/i18n` owns init + the locale→main-process listener; `main.tsx` imports it once for the side-effect. ESLint bans direct `react-i18next` / `i18next` imports in the renderer.

### 3. Design system — no hardcoded brand colors

Use `text-brand`, `text-brand-soft`, `bg-brand`, `border-brand`, `ring-brand`.
CSS vars: `var(--color-brand)`, `var(--color-brand-soft)`.
ESLint errors on `[#RRGGBB]` in className strings.

### 4. Motion — no inline transition objects

Use `import { transition } from '@/lib/motion'` → `transition.fast / .normal / .relaxed / .slow / .spring / .modal / .overlay`.
ESLint errors on inline `{ duration, ease }` objects in feature/route files.

### 5. UI primitives — always use `@ajh/ui`

| Need                | Import                                             |
| ------------------- | -------------------------------------------------- |
| Button              | `Button` from `@ajh/ui`                            |
| Input / Textarea    | `Input` / `TextArea` from `@ajh/ui`                |
| Number input        | `NumberField` from `@ajh/ui`                       |
| Dropdown            | `SelectDropdown` from `@ajh/ui`                    |
| Switch / Toggle     | `Switch` from `@ajh/ui`                            |
| Modal / Confirm     | `ModalShell` / `ConfirmModal` from `@ajh/ui`       |
| Empty / Error state | `EmptyState` / `ErrorState` from `@ajh/ui`         |
| Skeletons           | `RowSkeleton` / `CardSkeleton` from `@ajh/ui`      |
| Card / Settings     | `GlassCard` / `SettingsSection` from `@ajh/ui`     |
| Tile / Stream       | `OptionTile` / `StreamingText` from `@ajh/ui`      |
| Page wrapper        | `PageShell` from `@/components/layout/PageShell`   |
| App-specific        | `UpdateBanner` from `@/components/ui/UpdateBanner` |

ESLint errors on raw `<button>`, `<select>`, `<textarea>`.
Exception: `<input type="range|file|checkbox|radio|hidden">`.

### 6. Imports — package entrypoints, not deep paths

- `@ajh/ui` directly, not `@/components/ui/*` (except `UpdateBanner`)
- Prefer `React.ComponentProps<typeof Button>` over importing named prop types
- Only import named types from `@ajh/ui` when extending them or for non-obvious types (`ToastVariant`, `ThemeId`)

### 7. Import ordering — auto-fixable, run `rtk pnpm lint:fix`

Groups (blank line between each): `node:*` → external → `@ajh/*` → `@/*` → relative.

### 8. Type imports — always `import type` for pure types

`@typescript-eslint/consistent-type-imports` auto-fixes this. Never suppress it.

### 9. File placement

```
renderer/
  features/          ← components owned by ONE route
  components/ui/     ← re-exports from @ajh/ui (UpdateBanner is the exception)
  components/layout/ ← Sidebar, Titlebar, StatusBar, PageShell
  services/          ← React Query hooks for all IPC namespaces
  lib/               ← pure utilities (cn, motion, greeting, machine, i18n)
  hooks/             ← shared React hooks (use-machine, use-mouse-parallax)
  providers/         ← React context providers
  lib/machines/      ← state machine definitions
  store/             ← Zustand stores
```

New component: one feature → `features/*/components/`; shared → `packages/ui`; chrome → `components/layout/`.
Never import across feature directories.

### 10. State machines for complex flows

3+ states → `lib/machines/`. Use `useMachine` from `@/hooks/use-machine`.

### 11. Data fetching — React Query via service hooks

No `useState + useEffect` for remote data. Every IPC call goes through a service hook.

### 12. Package boundaries

- `packages/shared` — no React, no Node APIs
- `packages/ui` — no Zustand, no IPC, no routing
- `packages/prompts` — no UI, no `window`
- `packages/translations` — no app/IPC/renderer imports (the locale→main listener stays in the `@/i18n` shim)

### 13. Stale branch check — before any work

```bash
rtk git fetch origin && rtk git branch -r | grep $(git branch --show-current)
# If gone: rtk git checkout main && rtk git pull origin main
```

### 14. New IPC capability checklist

1. `packages/shared/src/ipc/contracts.ts` — add signature
2. `apps/tauri/src-tauri/src/commands.rs` — implement Tauri command
3. `apps/tauri/src/tauri-client.ts` — wire invoke call
4. `apps/tauri/src/renderer/services/` — create hook
5. `services/query-client.ts` — add query key

### 15. Never bypass ESLint

No `// eslint-disable`, no `@ts-ignore`. Add scoped overrides to `eslint.config.mjs` with a reason comment.
`rtk pnpm lint:strict` runs in CI with `--max-warnings 0`.

---

## Quick Reference

| What                    | Where                                                                                                         |
| ----------------------- | ------------------------------------------------------------------------------------------------------------- |
| IPC contract            | `packages/shared/src/ipc/contracts.ts`                                                                        |
| Tauri commands          | `apps/tauri/src-tauri/src/commands.rs`                                                                        |
| Tauri client (TS)       | `apps/tauri/src/tauri-client.ts`                                                                              |
| Service hooks           | `apps/tauri/src/renderer/services/`                                                                           |
| UI package              | `packages/ui/src/index.ts` → `@ajh/ui`                                                                        |
| Motion tokens           | `packages/ui/src/lib/motion.ts` (import via `@/lib/motion`)                                                   |
| State machines          | `apps/tauri/src/renderer/lib/machines/`                                                                       |
| Design tokens           | `packages/ui/src/css/tokens.css`                                                                              |
| i18n (translations)     | `packages/translations/src/index.ts` → `@ajh/translations`; init shim `apps/tauri/src/renderer/i18n/index.ts` |
| Config / paths (Rust)   | `apps/tauri/src-tauri/src/platform/config.rs` (`data_dir()`)                                                  |
| HTTP client (Rust)      | `apps/tauri/src-tauri/src/net/http.rs` (`shared()` / `build_client()`)                                        |
| Errors (Rust)           | `apps/tauri/src-tauri/src/error.rs` (`AppError` / `AppResult`)                                                |
| Trace spans (Rust)      | `apps/tauri/src-tauri/src/observability.rs` (`Span`)                                                          |
| Board registry          | `scraping/boards/mod.rs` (`SCRAPERS`) — no applier registry (apply engine removed)                            |
| Architecture principles | `docs/PATTERNS.md` §13                                                                                        |
| Architecture status     | `docs/ARCHITECTURE_STATUS.md`                                                                                 |
| Architecture (general)  | `docs/ARCHITECTURE.md`                                                                                        |
| Export templates        | `docs/EXPORT_TEMPLATES.md` (9-template + backend contract)                                                    |
| Patterns                | `docs/PATTERNS.md`                                                                                            |
| Design system           | `docs/DESIGN_SYSTEM.md`                                                                                       |
| Dev setup               | `docs/DEVELOPMENT.md`                                                                                         |

## Release Pipeline

Automated via semantic-release on push to `main`. Do not manually tag or bump versions.

| Prefix                                         | Triggers      |
| ---------------------------------------------- | ------------- |
| `feat:`                                        | minor (1.x.0) |
| `fix:`, `perf:`                                | patch (1.0.x) |
| `BREAKING CHANGE` footer                       | major (x.0.0) |
| `refactor:`, `docs:`, `chore:`, `ci:`, `test:` | no release    |

### Commit messages — enforced by commitlint (`commit-msg` hook)

Violations **fail the commit**. See `commitlint.config.mjs`.

- **Subject MUST be lower-case** (`subject-case`). `fix: admit website links` ✅ — `fix: Admit Website URLs` ❌ (capitalized words and acronyms like `URL`/`API`/`DOCX` are rejected in the subject; reword or lowercase them).
- **Subject ≤ 100 chars**; **body lines ≤ 200 chars**; blank line between subject and body.
- **Type** is one of: `feat`, `fix`, `perf`, `refactor`, `ui`, `style`, `test`, `docs`, `build`, `ci`, `chore`, `revert` (`type-enum`). Only the types in the table above affect releases.
- Imperative mood, no trailing period in the subject.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:

- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost), then `codegraph sync` to refresh the structural index.

## codegraph (structural code graph)

Deterministic, zero-token code index (SQLite at `.codegraph/`, 968 files indexed via tree-sitter). **Complements graphify** — codegraph answers _structural_ questions, graphify answers _semantic / cross-document_ ones. Installed globally (`@colbymchenry/codegraph`); the watcher auto-syncs the index, so no manual rebuild is normally needed.

Rules:

- "Who calls / what does this call / what breaks if I change X" → codegraph: `codegraph callers <symbol>`, `codegraph callees <symbol>`, `codegraph impact <symbol>`, `codegraph query <search>`. Faster and cheaper than grepping for symbol / call / import / impact lookups.
- "What is X connected to semantically", concepts spanning code + docs, or architecture narrative → graphify (`graphify query / explain / path`).
- Routing: structural facts (symbols, calls, imports, impact) → **codegraph**; meaning, rationale, cross-doc synthesis → **graphify**; raw `rg`/`fd` only when neither graph has the answer.
- **NEVER** use `rg`, `grep`, `fd`, or raw file browsing to answer "where is X", "what calls X", "what does X call", or "what breaks if I change X" — those are codegraph questions. **NEVER** browse raw source for architecture or cross-file questions — those are graphify questions. Reach for grep only after both graphs return no relevant result.
- Exposed to agents as an MCP server via `.mcp.json` (`codegraph serve --mcp`). When connected, prefer the `codegraph_explore` tool first — the server injects full tool guidance at connect, so no need to duplicate it here. If the index looks stale, run `codegraph sync`.

## Knowledge base & agent system

This repo ships a Claude Code agent system under `.claude/` (tracked) plus a knowledge base under `docs/knowledge/`.

### Mandatory default — always route through agents, auto-selected

**Every task and code change runs through this agent system by default — no slash command and no explicit agent name required.** If the user names no agent, you still pick and run the right one automatically; never wait to be told which agent to use. Agents auto-select by their `description` — match the touched area to its **Primary Owner**:

| Touched area                                                                           | Primary Owner agent                                                      |
| -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| React renderer (`apps/tauri/src/renderer/**`, components, routes, UI state)            | `frontend-reviewer`                                                      |
| Rust/Tauri backend (`apps/tauri/src-tauri/src/**`, not owned by a more specific agent) | `rust-backend-architect`                                                 |
| Resume/export, DocumentModel, templates, theme, locale                                 | `resume-export-expert` (review) · `pdf-docx-generator` (write/rendering) |
| ATS scoring, job analysis, resume↔job matching, cover-letter relevance                 | `job-match-expert`                                                       |
| AI providers, model routing, embeddings, prompts, streaming                            | `ai-provider-expert`                                                     |
| Scraping, browser automation, application automation, registries                       | `scraping-applier-expert`                                                |
| Security config / any risk-bearing change                                              | `tauri-security-reviewer` (default **Secondary**)                        |
| Docs / knowledge base / ADRs / lessons / release                                       | `project-steward` (sole writer)                                          |
| Tests (write) / test audit                                                             | `test-author` / `testing-reviewer`                                       |

**Per-change sequence** (skip stages that don't apply): **Primary Owner implements → review pass (resolve HIGH/CRITICAL before continuing; LOW/MEDIUM advisory) → if the change touches testable logic, `test-author` writes tests then `testing-reviewer` audits the changed code → `project-steward` closes (docs/`docs/knowledge`/lessons sync + `graphify update .` + `codegraph sync`)**. Stay within **≤3 reviewers** (Primary + risk-justified Secondaries; `tauri-security-reviewer` is the default Secondary on risk-bearing changes). **Orchestrate every sub-agent from the main session** — agents can't call agents, so sequence them yourself; never tell one agent to "hand off" to another.

**Code modification blocker (absolute):** The main Claude session **MUST NEVER** directly edit, write, or delete source files. ALL code changes must be delegated to the appropriate project agent via the `Agent` tool. Exceptions: `CLAUDE.md` (meta/config), plan files, and single-character typo fixes. If no agent clearly owns the area, spawn `general-purpose` to identify the right agent first.

**Stay light.** Trivial diffs (docs / config / rename / one-liners) **skip the swarm** — just make the change; the Stop review-gate reviews the real diff regardless. This is a strong default for AI sessions; the only _hard_ enforcement is the Stop review-gate hook + CI (ESLint, commitlint, architecture tests).

- **Agents** (`.claude/agents/`) — 12 specialized reviewers/producers: `resume-export-expert`, `job-match-expert`, `scraping-applier-expert`, `ai-provider-expert`, `rust-backend-architect`, `tauri-security-reviewer`, `frontend-reviewer`, `performance-profiler`, `testing-reviewer`, `test-author`, `pdf-docx-generator`, `project-steward`.
- **Commands** (`.claude/commands/`) — `/review-{rust,security,performance,ats,resume,template,export,frontend,scraping,ai}`, `/implement-feature`, `/fix-bug`, `/refactor-module`, `/add-tests`, `/analyze-job-ad`, `/improve-ats-score`, `/update-docs`, `/prepare-release`.
- **Skills** (`.claude/skills/`) — domain standards/checklists + `token-efficiency`, `review-workflow`, `lessons`.
- **Stop review-gate** (`.claude/hooks/review-gate.mjs`, routed by `.claude/review-routes.json`) — on finish, reviews the diff with the owning agent's checklist: deterministic arch-guards + one batched LLM pass; **only HIGH/CRITICAL block** (architecture-rule violation; untested error/security path on changed code; credential/IPC/updater exploit; data loss/corruption — LOW/MEDIUM are advisory); **≤3 reviewers per task** (Primary Owner → optional risk Secondary), with a separate conditional `test-author → testing-reviewer` stage. It **blocks once per finish-chain**, then lets the next finish through — run a `/review-*` command for a second enforced pass — and is **inert in plan mode** (empty diff). It never edits tracked files; a stale-docs finding is advisory → `/update-docs`.
- **Lessons** (`.claude/hooks/lessons.mjs` → `.claude/memory/lessons.jsonl`, local) — distilled experiential memory; **only `project-steward` writes** (others propose via `LESSON · category · Context/Decision/Outcome`).

### Model & effort tiering

Agents are model-tiered by task difficulty (each agent's `model:` frontmatter):

- **Opus** — correctness / reasoning-critical: `rust-backend-architect`, `tauri-security-reviewer`, `ai-provider-expert`, `job-match-expert`, `resume-export-expert`.
- **Sonnet** — balanced implement/review: `frontend-reviewer`, `scraping-applier-expert`, `pdf-docx-generator`, `test-author`, `testing-reviewer`, `performance-profiler`.
- **Haiku** — mechanical: `project-steward` (docs / lessons sync).

Effort (thinking budget) tracks the same axis: extended thinking ("think hard" / "ultrathink") for architecture, security, concurrency, data-loss, and cross-module work; normal effort for routine UI, docs, config, and renames. Override per spawn with the Agent tool's `model` parameter when a task is unusually hard or trivial.

**Context-source priority for codebase questions: codegraph (structural) / graphify (semantic) → source (authoritative) → `docs/knowledge/` → lessons.** Read the minimum; stop at ~90% confidence. `docs/knowledge/` is thin pointers into source/docs — keep it that way (no copied literals; point at the owning symbol). `project-steward` keeps it in sync.
