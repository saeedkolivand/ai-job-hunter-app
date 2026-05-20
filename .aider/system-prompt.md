# AI Job Hunter — Aider System Prompt

You are working on a local-first Electron desktop app in a pnpm monorepo.
React 19, TypeScript strict, Tailwind CSS v4, TanStack Router + React Query.

## Architecture summary

- Renderer never calls `window.api.*` directly — use service hooks in `@/services/`
- IPC contract is the single source of truth: `packages/shared/src/ipc/contracts.ts`
- Backend packages (`@ajh/core`, `@ajh/ai`, `@ajh/data`) are main-process only
- `packages/ui` is the React component library — no app logic inside it

## Before editing any file, check:

1. Does a service hook already exist in `apps/desktop/src/renderer/services/`?
2. Is there a shared UI primitive in `apps/desktop/src/renderer/components/ui/`?
3. Does the IPC contract need updating in `packages/shared/src/ipc/contracts.ts`?

## Mandatory patterns

**Data fetching:**

```ts
import { useDocuments } from '@/services'; // not window.api directly
const { data, isLoading } = useDocuments();
```

**i18n:**

```ts
import { useTranslation } from '@/lib/i18n'; // not 'react-i18next'
```

**Motion:**

```ts
import { transition } from '@/lib/motion';
// use transition.normal | transition.fast | transition.relaxed | transition.spring
```

**Colors:**

```tsx
className = 'text-brand-soft bg-brand/15'; // not text-[#c084fc] bg-[#a855f7]/15
```

**Complex state flows:**

```ts
import { useMachine } from '@/hooks/use-machine';
// define machine in lib/machines/, use useMachine hook in component
```

**New IPC capability checklist:**

1. contracts.ts → 2. ipc/router.ts → 3. preload/index.ts → 4. services/ hook

## Reference documentation

Read these files for full details before making structural changes:

- `docs/ARCHITECTURE.md` — process model, package roles, data flows, dependency rules
- `docs/PATTERNS.md` — all 13 enforced coding patterns with examples
- `docs/DESIGN_SYSTEM.md` — color tokens, glass surfaces, typography, motion
- `docs/DEVELOPMENT.md` — commands, debugging, env config, CI
- `docs/RELEASE.md` — automated release pipeline, commit types, build artifacts

**Release:** Never manually tag releases or edit CHANGELOG.md/versions.
Releases are automated via semantic-release on push to main.
feat → minor, fix/perf → patch, BREAKING CHANGE → major.
