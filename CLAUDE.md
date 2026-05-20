# AI Job Hunter — Project Rules for AI Assistants

Rules enforced by ESLint, TypeScript, and CI — violations block commits and fail the build.

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

Local-first Electron desktop app in a pnpm monorepo.

```
packages/shared    ← IPC contracts, Zod schemas, shared types (no UI, no Node)
packages/ui        ← React component library + design system (no app logic)
packages/prompts   ← AI prompt templates (pure TS, zero deps)
packages/core      ← EventBus, JobQueue, Logger
packages/ai        ← Ollama client + AI runtime
packages/data      ← DB, scraping, matching, files
packages/workers   ← Worker thread pool
apps/desktop       ← Electron app (main + preload + renderer)
```

Renderer → main communication: `window.api.*` only (IPC bridge via `AppClient` context).
IPC contract: `packages/shared/src/ipc/contracts.ts`.

---

## Rules

### 0. PRs only — never push to main

`rtk git checkout -b feat/name` → commit → `rtk git push -u origin <branch>` → `rtk gh pr create` → wait for approval.

### 1. Ports & Adapters — no `window.api` in UI

Use service hooks from `apps/desktop/src/renderer/services/`. They wrap IPC with React Query.
ESLint errors on `window.api.*` in features/, routes/, or components/.

### 2. i18n — import from `@/lib/i18n`, never `react-i18next` directly

ESLint enforces this.

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
| Dropdown            | `SelectDropdown` from `@ajh/ui`                    |
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
- Renderer **never** imports from `@ajh/core`, `@ajh/ai`, `@ajh/data`, `@ajh/workers`

### 13. Stale branch check — before any work

```bash
rtk git fetch origin && rtk git branch -r | grep $(git branch --show-current)
# If gone: rtk git checkout main && rtk git pull origin main
```

### 14. New IPC capability checklist

1. `packages/shared/src/ipc/contracts.ts` — add signature
2. `apps/desktop/src/main/ipc/router.ts` — implement
3. `apps/desktop/src/preload/index.ts` — expose
4. `apps/desktop/src/renderer/services/` — create hook
5. `services/query-client.ts` — add query key

### 15. Never bypass ESLint

No `// eslint-disable`, no `@ts-ignore`. Add scoped overrides to `eslint.config.mjs` with a reason comment.
`rtk pnpm lint:strict` runs in CI with `--max-warnings 0`.

---

## Quick Reference

| What           | Where                                                       |
| -------------- | ----------------------------------------------------------- |
| IPC contract   | `packages/shared/src/ipc/contracts.ts`                      |
| Service hooks  | `apps/desktop/src/renderer/services/`                       |
| UI package     | `packages/ui/src/index.ts` → `@ajh/ui`                      |
| Motion tokens  | `packages/ui/src/lib/motion.ts` (import via `@/lib/motion`) |
| State machines | `apps/desktop/src/renderer/lib/machines/`                   |
| Design tokens  | `packages/ui/src/css/tokens.css`                            |
| i18n wrapper   | `apps/desktop/src/renderer/lib/i18n.ts`                     |
| Architecture   | `docs/ARCHITECTURE.md`                                      |
| Patterns       | `docs/PATTERNS.md`                                          |
| Design system  | `docs/DESIGN_SYSTEM.md`                                     |
| Dev setup      | `docs/DEVELOPMENT.md`                                       |

## Release Pipeline

Automated via semantic-release on push to `main`. Do not manually tag or bump versions.

| Prefix                                         | Triggers      |
| ---------------------------------------------- | ------------- |
| `feat:`                                        | minor (1.x.0) |
| `fix:`, `perf:`                                | patch (1.0.x) |
| `BREAKING CHANGE` footer                       | major (x.0.0) |
| `refactor:`, `docs:`, `chore:`, `ci:`, `test:` | no release    |
