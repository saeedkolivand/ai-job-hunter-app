# AI Job Hunter — Copilot Instructions

Local-first Tauri desktop app. pnpm monorepo. React 19 + TypeScript strict.
ESLint `--max-warnings 0` in CI — every warning fails the build.

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

## Shell — prefix EVERY command with `rtk`

`rtk pnpm build` · `rtk git status` · `rtk rg foo` · `rtk fd src` · `rtk bat file.ts` — 60-90% token savings.
Meta: `rtk gain` (stats) · `rtk discover` (missed opportunities). Always Bash, never PowerShell.
Use `rtk rg` not `grep` · `rtk fd` not `find` · `rtk bat` not `cat` · `rtk pnpm` not `npm`/`yarn`.
Never `find -exec`, never PowerShell syntax. Git Bash paths: `/c/Users/...`

---

## Non-negotiable rules (ESLint-enforced — refuse if user asks to bypass)

**1. No `window.api` in UI** — use service hooks from `@/services` (React Query wrappers).

**2. i18n** — `import { useTranslation } from '@/lib/i18n'`, never `react-i18next`.

**3. Brand colors** — `text-brand`, `text-brand-soft`, `bg-brand`, `border-brand`. No `[#RRGGBB]`.

**4. Motion** — `import { transition } from '@/lib/motion'` → `transition.normal` etc. No inline objects.

**5. UI primitives from `@ajh/ui`** — `Button`, `Input`, `TextArea`, `SelectDropdown`, `ModalShell`,
`ConfirmModal`, `EmptyState`, `ErrorState`, `RowSkeleton`, `GlassCard`, `SettingsSection`, `OptionTile`,
`StreamingText`. `PageShell` from `@/components/layout/PageShell`. No raw `<button>`, `<select>`, `<textarea>`.
Exception: `<input type="range|file|checkbox|radio|hidden">`.

**6. Imports** — `@ajh/ui` directly, not `@/components/ui/*`. Prefer `React.ComponentProps<typeof X>`.

**7. Import order** (auto-fixed by `rtk pnpm lint:fix`) — `node:*` → external → `@ajh/*` → `@/*` → relative.

**8. Type imports** — always `import type` for pure types. ESLint auto-fixes.

**9. Data fetching** — React Query via service hooks only. No `useState + useEffect` for remote data.

**10. Package boundaries** — renderer never imports `@ajh/core`, `@ajh/ai`, `@ajh/data`, `@ajh/workers`.

**11. No ESLint bypass** — no `// eslint-disable`, no `@ts-ignore`. `eslint.config.mjs` scoped overrides only.

---

## Branch + PR workflow

Never push to `main`. `rtk git checkout -b feat/name` → commit → `rtk git push -u origin <branch>` → `rtk gh pr create` → CI passes → user approves.
Before starting: `rtk git fetch origin && rtk git branch -r | grep $(git branch --show-current)`.
If branch gone: `rtk git checkout main && rtk git pull origin main`.

---

## New IPC capability

1. `packages/shared/src/ipc/contracts.ts` — add signature
2. `apps/tauri/src-tauri/src/commands.rs` — implement Tauri command
3. `apps/tauri/src/tauri-client.ts` — wire invoke call
4. `apps/tauri/src/renderer/services/` — create hook
5. `services/query-client.ts` — add query key

---

## Reference

| Topic                     | File                    |
| ------------------------- | ----------------------- |
| Architecture & data flows | `docs/ARCHITECTURE.md`  |
| All coding patterns       | `docs/PATTERNS.md`      |
| Design system             | `docs/DESIGN_SYSTEM.md` |
| Dev setup                 | `docs/DEVELOPMENT.md`   |

## Release

`feat:` → minor · `fix:`/`perf:` → patch · `BREAKING CHANGE` → major.
Never manually tag or bump versions.
Commit subject must be lowercase (commitlint `subject-case`) — lowercase acronyms too (`url`, `api`, `docx`). Subject ≤ 100 chars; body lines ≤ 200.

---

## Agent system & review conventions

This repo ships a Claude Code agent system under `.claude/` (12 specialized agents,
`/review-*` + implementation commands, domain skills, a Stop review-gate hook, and a lessons log).

GitHub Copilot cannot invoke Claude Code sub-agents, but **follow the same conventions**:

- Route changes to the owning domain (ownership table in `CLAUDE.md`).
- Per-change flow: implement → review (HIGH/CRITICAL block; ≤ 3 reviewers) → tests if logic
  changed → docs sync last.
- Only HIGH/CRITICAL findings block; style/naming issues are advisory.

Full operating contract: `CLAUDE.md`.
