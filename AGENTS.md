# AI Job Hunter — Agent Rules

> Canonical rules: see `CLAUDE.md` + `.claude/skills/*` — this file is a pointer + the load-bearing subset; CLAUDE.md wins on any conflict.

Rules enforced by ESLint, TypeScript, and CI. Violations fail the build.

---

## Auto-Invoked Skills (on by default — no slash command)

Active automatically every session; invoke via the Skill tool without waiting for the slash command.

- **ponytail** — lazy-senior-dev mode, every response. Simplest/shortest solution that works: YAGNI (question whether the task needs to exist), stdlib/native platform before dependencies, one line over fifty. Default intensity `full`; switch with `/ponytail lite|full|ultra`. Off only on `stop ponytail` / `normal mode`. Source: `ponytail@ponytail` plugin.
- **grill-with-docs** — before presenting any non-trivial plan/design (incl. before `ExitPlanMode`), first stress-test it against the repo domain model + ADRs, one question at a time. Skip for trivial / one-line / docs changes. Source: `~/.claude/skills/grill-with-docs`.

---

## Path Privacy

- Never expose real local file system paths
- Never output absolute Windows, macOS, or Linux paths
- Always use repository-relative paths

❌ `C:\Users\username\project\apps\desktop\src\main.rs`
❌ `/home/username/project/apps/api/src/server.ts`
❌ `~/Projects/app/src/index.ts`

✅ `apps/desktop/src/main.rs`
✅ `apps/api/src/server.ts`

- Never expose usernames, home directories, drive letters, workspace roots, temp paths, or IDE-specific paths
- Sanitize absolute paths in logs, stack traces, screenshots, terminal output, PRs, commits, comments, and markdown
- Prefer repository-root-relative paths. If needed, use: `file:///app/<relative-path>`

---

## Shell

Always use Bash (never PowerShell).
**Prefix EVERY command with `rtk`** — `rtk pnpm build`, `rtk git status`, `rtk rg foo`, `rtk fd src`, `rtk bat file.ts`
Meta commands: `rtk gain` (savings stats) · `rtk discover` (missed opportunities).
Use `rtk rg` not `grep` · `rtk fd` not `find` · `rtk bat` not `cat` · `rtk pnpm` not `npm`/`yarn`.
Never `find -exec`, never PowerShell syntax. Git Bash paths: `/c/Users/...`

---

## Architecture

Local-first Tauri desktop app in a pnpm monorepo.

```
packages/shared    ← IPC contracts, Zod schemas, shared types (no UI, no Node)
packages/ui        ← React component library (@ajh/ui — no app logic)
packages/prompts   ← AI prompt templates (pure TS, zero deps)
apps/desktop         ← Tauri app (Rust core + React renderer)
```

Renderer → Tauri: `AppClient` context → service hooks → `invoke/listen`.
IPC contract: `packages/shared/src/ipc/contracts.ts`.

---

## Non-negotiable rules (ESLint-enforced)

**1. No `window.api` in UI** — use service hooks from `@/services` (React Query wrappers).

**2. i18n** — `import { useTranslation } from '@ajh/translations'`, never `react-i18next` directly. Renderer init shim is `@/i18n`.

**3. Brand colors** — `text-brand`, `text-brand-soft`, `bg-brand`, `border-brand`, `ring-brand`. No `[#RRGGBB]`.

**4. Motion** — `import { transition } from '@ajh/ui'`. No inline `{ duration, ease }` objects.

**5. UI primitives** — all from `@ajh/ui`: `Button`, `Input`, `TextArea`, `SelectDropdown`, `ModalShell`,
`ConfirmModal`, `EmptyState`, `ErrorState`, `RowSkeleton`, `GlassCard`, `SettingsSection`, `OptionTile`,
`StreamingText`. `PageShell` from `@/components/layout/PageShell`. No raw `<button>`, `<select>`, `<textarea>`.

**6. Imports** — `@ajh/ui` directly, not `@/components/ui/*`. Prefer `React.ComponentProps<typeof X>`.

**7. Import order** — `node:*` → external → `@ajh/*` → `@/*` → relative. Run `rtk pnpm lint:fix`.

**8. Type imports** — always `import type` for pure types. ESLint auto-fixes.

**9. No ESLint bypass** — no `// eslint-disable`, no `@ts-ignore`. Scoped overrides in `eslint.config.mjs` only.

**10. Data fetching** — React Query via service hooks only. No `useState + useEffect` for remote data.

**11. Package boundaries** — renderer imports only `@ajh/shared`, `@ajh/ui`, `@ajh/prompts`, `@ajh/translations`, `@ajh/test-ids`.

**12. State machines** — 3+ state flows use `useMachine` + machines in `lib/machines/`.

---

## PR workflow

Never push to `main`. Always: `rtk git checkout -b feat/name` → commit → `rtk git push -u origin <branch>` → `rtk gh pr create` → wait for approval.
Before starting: `rtk git fetch origin && rtk git branch -r | grep $(git branch --show-current)`.
If branch is gone: `rtk git checkout main && rtk git pull origin main`.

## New IPC capability (5 steps)

1. `packages/shared/src/ipc/contracts.ts` — add signature
2. `apps/desktop/src-tauri/src/commands.rs` — implement Tauri command
3. `apps/desktop/src/tauri-client.ts` — wire invoke call
4. `apps/desktop/src/renderer/services/` — create hook
5. `services/query-client.ts` — add query key

## Release

`feat:` → minor, `fix:`/`perf:` → patch, `BREAKING CHANGE` footer → major.
Never manually tag releases or edit CHANGELOG.md.
Commit subject must be **lowercase** (commitlint `subject-case`) — lowercase acronyms too (`url`, `api`, `docx`). Subject ≤ 100 chars; body lines ≤ 200.

---

## Code graphs (codegraph + graphify)

Two complementary indexes for codebase questions — prefer them over raw `rg`/`fd`.

- **codegraph** — deterministic, zero-token structural index (SQLite at `.codegraph/`, auto-synced via watcher). Use for structural facts: `codegraph callers/callees/impact <symbol>`, `codegraph query <search>`. Wired as an MCP server in `.mcp.json`; via MCP prefer the `codegraph_explore` tool first.
- **graphify** — semantic / cross-document graph (`graphify-out/`). Use for meaning, rationale, architecture narrative: `graphify query/explain/path`. After code changes: `graphify update .`.
- Routing: structural (symbols / calls / imports / impact) → **codegraph**; meaning / rationale / cross-doc synthesis → **graphify**; raw `rg`/`fd` only when neither has the answer.

---

## Agent system & review conventions

This repo ships a Claude Code agent system under `.claude/` with 24 specialized agents (paired author + critic per domain, plus cross-cutting cleanup / project-steward / pr-reviewer),
`/review-*` + `/implement-feature`/`/fix-bug`/`/refactor-module`/`/add-tests`/`/update-docs`/`/prepare-release`
commands, domain skills/checklists, a Stop review-gate hook, and a lessons log.

You cannot invoke those Claude Code sub-agents directly, but **follow the same conventions**:

- Route changes to the owning domain (see the ownership table in `CLAUDE.md`).
- Per-change flow: implement → review pass (HIGH/CRITICAL findings block; ≤ 3 reviewers) →
  tests if logic changed → docs sync last.
- Only HIGH/CRITICAL findings block; style/naming issues are advisory.
- Model tiering (agent `model:` frontmatter): **Opus** for correctness-critical, **Sonnet** for balanced implement/review, **Haiku** for mechanical docs — see the canonical per-agent map in `CLAUDE.md` (§ Model & effort tiering); this file defers to it.
- Effort: extended thinking for architecture / security / concurrency / data-loss; normal for routine UI / docs / config.

Full operating contract: `CLAUDE.md`.
