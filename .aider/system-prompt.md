# AI Job Hunter — Aider System Prompt

> Canonical rules: see `CLAUDE.md` + `.claude/skills/*` — this file is a pointer + the load-bearing subset; CLAUDE.md wins on any conflict.

You are working on a local-first **Tauri** desktop app in a pnpm monorepo.
React 19, TypeScript strict, Tailwind CSS v4, TanStack Router + React Query.
The Rust core lives in `apps/desktop/src-tauri/`; the React renderer in `apps/desktop/src/renderer/`.

## Architecture summary

- The renderer never calls `window.api.*` directly — use service hooks in `apps/desktop/src/renderer/services/`.
- IPC contract is the single source of truth: `packages/shared/src/ipc/contracts.ts`.
- The four workspace packages are exactly `@ajh/shared`, `@ajh/ui`, `@ajh/prompts`, `@ajh/translations`.
  Backend business logic lives in the Rust core (`apps/desktop/src-tauri/`), not in a TS package.

## Before editing any file, check:

1. Does a service hook already exist in `apps/desktop/src/renderer/services/`?
2. Is there a shared UI primitive in `@ajh/ui`?
3. Does the IPC contract need updating in `packages/shared/src/ipc/contracts.ts`?

## Mandatory patterns

**Data fetching:**

```ts
import { useDocuments } from '@/services'; // not window.api directly
const { data, isLoading } = useDocuments();
```

**i18n:**

```ts
import { useTranslation } from '@ajh/translations'; // not 'react-i18next'; init shim is @/i18n
```

**Motion:**

```ts
import { transition } from '@ajh/ui';
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

**New IPC capability (5 steps):**

1. `packages/shared/src/ipc/contracts.ts` → 2. `apps/desktop/src-tauri/src/commands.rs`
   → 3. `apps/desktop/src/tauri-client.ts` → 4. `apps/desktop/src/renderer/services/` hook
   → 5. `services/query-client.ts` query key

## Reference documentation

Read these files for full details before making structural changes:

- `docs/ARCHITECTURE.md` — process model, package roles, data flows, dependency rules
- `docs/PATTERNS.md` — enforced coding patterns with examples
- `docs/DESIGN_SYSTEM.md` — color tokens, glass surfaces, typography, motion
- `docs/DEVELOPMENT.md` — commands, debugging, env config, CI

**Release:** Never manually tag releases or edit CHANGELOG.md/versions.
Releases run via semantic-release. feat → minor, fix/perf → patch, BREAKING CHANGE → major.
Commit subject must be lowercase (commitlint subject-case) — lowercase acronyms too (url, api, docx). Subject ≤ 100 chars; body lines ≤ 200.
