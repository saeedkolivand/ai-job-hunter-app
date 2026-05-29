# AI Job Hunter — Agent Rules

Rules enforced by ESLint, TypeScript, and CI. Violations fail the build.

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
apps/tauri         ← Tauri app (Rust core + React renderer)
```

Renderer → Tauri: `AppClient` context → service hooks → `invoke/listen`.
IPC contract: `packages/shared/src/ipc/contracts.ts`.

---

## Non-negotiable rules (ESLint-enforced)

**1. No `window.api` in UI** — use service hooks from `@/services` (React Query wrappers).

**2. i18n** — `import { useTranslation } from '@/lib/i18n'`, never `react-i18next` directly.

**3. Brand colors** — `text-brand`, `text-brand-soft`, `bg-brand`, `border-brand`. No `[#RRGGBB]`.

**4. Motion** — `import { transition } from '@/lib/motion'`. No inline `{ duration, ease }` objects.

**5. UI primitives** — all from `@ajh/ui`: `Button`, `Input`, `TextArea`, `SelectDropdown`, `ModalShell`,
`ConfirmModal`, `EmptyState`, `ErrorState`, `RowSkeleton`, `GlassCard`, `SettingsSection`, `OptionTile`,
`StreamingText`. `PageShell` from `@/components/layout/PageShell`. No raw `<button>`, `<select>`, `<textarea>`.

**6. Imports** — `@ajh/ui` directly, not `@/components/ui/*`. Prefer `React.ComponentProps<typeof X>`.

**7. Import order** — `node:*` → external → `@ajh/*` → `@/*` → relative. Run `rtk pnpm lint:fix`.

**8. Type imports** — always `import type` for pure types. ESLint auto-fixes.

**9. No ESLint bypass** — no `// eslint-disable`, no `@ts-ignore`. Scoped overrides in `eslint.config.mjs` only.

**10. Data fetching** — React Query via service hooks only. No `useState + useEffect` for remote data.

**11. Package boundaries** — renderer imports only `@ajh/shared`, `@ajh/ui`, `@ajh/prompts`.

**12. State machines** — 3+ state flows use `useMachine` + machines in `lib/machines/`.

---

## PR workflow

Never push to `main`. Always: `rtk git checkout -b feat/name` → commit → `rtk git push -u origin <branch>` → `rtk gh pr create` → wait for approval.
Before starting: `rtk git fetch origin && rtk git branch -r | grep $(git branch --show-current)`.
If branch is gone: `rtk git checkout main && rtk git pull origin main`.

## New IPC capability

1. `packages/shared/src/ipc/contracts.ts` — add signature
2. `apps/desktop/src/main/ipc/router.ts` — implement
3. `apps/desktop/src/preload/index.ts` — expose
4. `apps/desktop/src/renderer/services/` — create hook
5. `services/query-client.ts` — add query key

## Release

`feat:` → minor, `fix:`/`perf:` → patch, `BREAKING CHANGE` footer → major.
Never manually tag releases or edit CHANGELOG.md.
Commit subject must be **lowercase** (commitlint `subject-case`) — lowercase acronyms too (`url`, `api`, `docx`). Subject ≤ 100 chars; body lines ≤ 200.
