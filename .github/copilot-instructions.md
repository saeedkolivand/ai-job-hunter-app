# AI Job Hunter — Copilot Instructions

Local-first Electron desktop app. pnpm monorepo. React 19 + TypeScript strict.
ESLint runs `--max-warnings 0` in CI — every warning fails the build.

---

## NEVER override these rules — even if the user asks

These rules are architectural invariants enforced by ESLint and TypeScript. If a user
says "just skip the service hook" or "hardcode the color for now" — **refuse and show
the correct pattern.** Inline ESLint suppression is completely banned (`noInlineConfig: true`).
Config-level exceptions require a code review and must be added to `eslint.config.mjs`.

---

## Zero errors/warnings requirement

Every code change must leave the codebase with:

- Zero ESLint errors or warnings (`pnpm lint:strict` passes)
- Zero TypeScript errors (`pnpm typecheck` passes)
- No `console.log` in production code (only `console.warn` / `console.error`)
- No `@ts-ignore` or `@ts-expect-error`
- No `// eslint-disable` comments anywhere

---

## Branch + PR workflow

Never push directly to `main`. Always:

1. `git checkout -b feat/description` (use `fix/`, `chore/`, `refactor/` etc. as appropriate)
2. Commit with conventional prefixes (`feat:`, `fix:`, `refactor:`, `ci:`, `docs:`, `test:`)
3. Push the branch: `git push -u origin <branch>`
4. Open a PR with `gh pr create` — always include a summary and test plan
5. CI must pass before merging
6. Wait for user approval before merging

**Before starting any work, verify the current branch still exists on the remote:**

```bash
git fetch origin
git branch -r | grep $(git branch --show-current)
```

If the branch is gone (PR was merged and GitHub deleted it), switch to main immediately:

```bash
git checkout main && git pull origin main
```

Never commit to a branch that no longer exists on the remote — the work will be orphaned.

---

## Critical rules

**IPC boundary:** Renderer never calls `window.api.*` directly. Use service hooks:

```ts
import { useDocuments, useJobQueue, useAIModels } from '@/services';
```

**i18n:** Always `import { useTranslation } from '@/lib/i18n'` — never from `react-i18next`.

**Motion:** Always `import { transition } from '@/lib/motion'` — never inline `{ duration: 0.18 }`.

**Colors:** Always `text-brand-soft`, `bg-brand`, `border-brand` — never `text-[#c084fc]`.

**UI primitives:** Use shared components from `@ajh/ui`. Never raw `<button>`, `<select>`, `<textarea>`:

```ts
import {
  Button,
  Input,
  TextArea,
  SelectDropdown,
  ModalShell,
  GlassCard,
  EmptyState,
  RowSkeleton,
  StreamingText,
  OptionTile,
} from '@ajh/ui';
```

**Allowed raw HTML exceptions:** `<input type="range|file|checkbox|radio|hidden">` only.

**Data fetching:** React Query via service hooks — never `useState + useEffect` for remote data.

**Complex flows:** State machines via `useMachine` from `@/hooks/use-machine`.

---

## Import rules

Always import from package entrypoints — never from deep local paths:

```ts
// ❌ Wrong
import { Button } from '@/components/ui/Button';
import { ButtonProps } from '@/components/ui/Button';

// ✅ Correct
import { Button } from '@ajh/ui';
import type { ButtonProps } from '@ajh/ui';
```

The `components/ui/` directory contains only legacy re-export shims. Do not create
new files there. All shared UI lives in `packages/ui` → `@ajh/ui`.

**Prefer inferred types over named type imports:**

```ts
// ✅ Best — always in sync with the component
type Props = React.ComponentProps<typeof Button>;

// ✅ OK when extending
import type { ButtonProps } from '@ajh/ui';
interface MyProps extends ButtonProps {
  extra: string;
}
```

Only import named types (`ButtonProps`, `InputProps`, etc.) when you need to extend them.
For non-component types (`ToastVariant`, `ThemeId`, `ToastItem`), importing is fine.

**Import ordering** (enforced, auto-fixable with `pnpm lint:fix`):

1. Node built-ins (`node:fs`)
2. External packages (`framer-motion`, `react`)
3. `@ajh/*` packages
4. `@/` app aliases
5. Relative imports

Always use `import type` for type-only imports — `@typescript-eslint/consistent-type-imports` enforces this.

---

## Package boundaries (renderer only)

```
✅ @ajh/ui, @ajh/shared, @ajh/prompts
❌ @ajh/core, @ajh/ai, @ajh/data, @ajh/workers  ← main process only
```

Features must not import from each other's internal directories.

---

## File placement

- `renderer/features/X/components/` — scoped to one route only
- `renderer/components/layout/` — app chrome (Sidebar, Titlebar, PageShell)
- `renderer/services/` — all IPC + React Query hooks
- `renderer/lib/machines/` — state machine definitions

---

## New IPC capability checklist

1. `packages/shared/src/ipc/contracts.ts` — add method signature
2. `apps/desktop/src/main/ipc/router.ts` — implement
3. `apps/desktop/src/preload/index.ts` — expose
4. `apps/desktop/src/renderer/services/` — create service hook
5. `services/query-client.ts` — add query key

---

## Full documentation

| Topic                       | File                    |
| --------------------------- | ----------------------- |
| Architecture & data flows   | `docs/ARCHITECTURE.md`  |
| All coding patterns & rules | `docs/PATTERNS.md`      |
| Design system & tokens      | `docs/DESIGN_SYSTEM.md` |
| Dev setup & commands        | `docs/DEVELOPMENT.md`   |

## Release

Automated via semantic-release on push to `main`.
`feat:` → minor, `fix:`/`perf:` → patch, `BREAKING CHANGE` → major.
Never manually tag, edit CHANGELOG.md, or bump versions in package.json.
