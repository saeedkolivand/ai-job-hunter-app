# AI Job Hunter — Agent Rules

Rules enforced by ESLint, TypeScript, and CI. Violations fail the build.

---

## Shell

Always use Bash (never PowerShell).
**Prefix every command with `rtk`** — e.g. `rtk pnpm build`, `rtk git status`, `rtk rg foo` — for 60-90% token savings.
Meta commands: `rtk gain` (savings stats) · `rtk discover` (missed opportunities).
Prefer `rg`, `fd`, `pnpm`; use Git Bash paths (`/c/Users/...`).

---

## Architecture

Local-first Electron desktop app in a pnpm monorepo.

```
packages/shared    ← IPC contracts, Zod schemas, shared types (no UI, no Node)
packages/ui        ← React component library (@ajh/ui — no app logic)
packages/prompts   ← AI prompt templates (pure TS, zero deps)
packages/core/ai/data/workers ← Main process only. Never import in renderer.
apps/desktop       ← Electron app (main + preload + renderer)
```

Renderer → main: `window.api.*` only (via `AppClient` context → service hooks).
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

**7. Import order** — `node:*` → external → `@ajh/*` → `@/*` → relative. Run `pnpm lint:fix`.

**8. Type imports** — always `import type` for pure types. ESLint auto-fixes.

**9. No ESLint bypass** — no `// eslint-disable`, no `@ts-ignore`. Scoped overrides in `eslint.config.mjs` only.

**10. Data fetching** — React Query via service hooks only. No `useState + useEffect` for remote data.

**11. Package boundaries** — renderer never imports `@ajh/core`, `@ajh/ai`, `@ajh/data`, `@ajh/workers`.

**12. State machines** — 3+ state flows use `useMachine` + machines in `lib/machines/`.

---

## PR workflow

Never push to `main`. Always: branch → commit → push → `gh pr create` → wait for approval.
Before starting: `git fetch origin && git branch -r | grep $(git branch --show-current)`.
If branch is gone, switch to main: `git checkout main && git pull origin main`.

## New IPC capability

1. `packages/shared/src/ipc/contracts.ts` — add signature
2. `apps/desktop/src/main/ipc/router.ts` — implement
3. `apps/desktop/src/preload/index.ts` — expose
4. `apps/desktop/src/renderer/services/` — create hook
5. `services/query-client.ts` — add query key

## Release

`feat:` → minor, `fix:`/`perf:` → patch, `BREAKING CHANGE` footer → major.
Never manually tag releases or edit CHANGELOG.md.
