# AI Job Hunter — Project Rules for AI Assistants

Read this before writing any code. These rules are enforced by ESLint, TypeScript,
and CI — violations will block commits and fail the build.

---

## RTK — Token-Optimised Command Execution

**Always use the Bash tool** (never PowerShell) so the RTK hook auto-rewrites commands
for 60-90% token savings. This applies to the main agent and all subagents.

Meta commands must still be called explicitly:

```bash
rtk gain        # show token savings
rtk discover    # find missed opportunities
```

**Rule**: Use Bash for every shell command. Never use PowerShell.

---

## Architecture

This is a **local-first Electron desktop app** in a pnpm monorepo.

```
packages/shared    ← IPC contracts, Zod schemas, shared types (no UI, no Node)
packages/ui        ← React component library + design system (no app logic)
packages/prompts   ← AI prompt templates (pure TS, zero deps)
packages/core      ← Infrastructure: EventBus, JobQueue, Logger
packages/ai        ← Ollama client + AI runtime
packages/data      ← DB, scraping, matching, files
packages/workers   ← Worker thread pool
apps/desktop       ← Electron app (main + preload + renderer)
```

The renderer communicates with main **only** through `window.api.*` (IPC bridge).
The IPC contract lives in `packages/shared/src/ipc/contracts.ts`.

---

## Rules You Must Follow

### 1. Ports & Adapters — never call window.api directly in UI

**❌ Wrong — direct IPC in a component or route:**

```ts
const jobs = await window.api.scrape.listPostings();
```

**✅ Right — use the service hook:**

```ts
import { usePostings } from '@/services';
const { data: jobs, isLoading } = usePostings();
```

Service hooks live in `apps/desktop/src/renderer/services/`.
They wrap every IPC namespace with React Query. Always use them.
ESLint errors on `window.api.*` calls in features/, routes/, or components/.

---

### 2. i18n — never import from react-i18next directly

**❌ Wrong:**

```ts
import { useTranslation } from 'react-i18next';
```

**✅ Right:**

```ts
import { useTranslation } from '@/lib/i18n';
```

ESLint enforces this. The wrapper at `lib/i18n.ts` is the only allowed entry point.

---

### 3. Design system — never hardcode brand colors

**❌ Wrong:**

```tsx
<div className="text-[#c084fc] bg-[#a855f7]/15">
```

**✅ Right:**

```tsx
<div className="text-brand-soft bg-brand/15">
```

Brand tokens: `text-brand`, `text-brand-soft`, `bg-brand`, `border-brand`, `ring-brand`.
CSS vars: `var(--color-brand)`, `var(--color-brand-soft)`.
ESLint errors on any `[#RRGGBB]` in className strings.

---

### 4. Motion — never use inline transition objects

**❌ Wrong:**

```tsx
<motion.div transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}>
```

**✅ Right:**

```tsx
import { transition } from '@/lib/motion';
<motion.div transition={transition.normal}>
```

Available tokens: `transition.fast`, `transition.normal`, `transition.relaxed`,
`transition.slow`, `transition.spring`, `transition.modal`, `transition.overlay`.
ESLint errors on inline transition objects in feature/route files.

---

### 5. UI primitives — always use shared components

Never use raw HTML elements for interactive UI. Always import from `@ajh/ui`:

| Need            | Import                                                        |
| --------------- | ------------------------------------------------------------- |
| Button / action | `import { Button } from '@ajh/ui'`                            |
| Text input      | `import { Input } from '@ajh/ui'`                             |
| Textarea        | `import { TextArea } from '@ajh/ui'`                          |
| Dropdown        | `import { SelectDropdown } from '@ajh/ui'`                    |
| Modal / dialog  | `import { ModalShell } from '@ajh/ui'`                        |
| Confirm dialog  | `import { ConfirmModal } from '@ajh/ui'`                      |
| Empty state     | `import { EmptyState } from '@ajh/ui'`                        |
| Error state     | `import { ErrorState } from '@ajh/ui'`                        |
| Loading rows    | `import { RowSkeleton, CardSkeleton } from '@ajh/ui'`         |
| Card surface    | `import { GlassCard } from '@ajh/ui'`                         |
| Settings card   | `import { SettingsSection } from '@ajh/ui'`                   |
| Selectable tile | `import { OptionTile } from '@ajh/ui'`                        |
| AI text stream  | `import { StreamingText } from '@ajh/ui'`                     |
| Page wrapper    | `import { PageShell } from '@/components/layout/PageShell'`   |
| App-specific UI | `import { UpdateBanner } from '@/components/ui/UpdateBanner'` |

ESLint errors on raw `<button>`, `<select>`, `<textarea>` in feature/route/component files.
Exceptions: `<input type="range|file|checkbox|radio|hidden">` — no shared primitive exists.

---

### 6. Import style — always use package entrypoints, never deep paths

**❌ Wrong — deep paths through local re-export wrappers:**

```ts
import { Button } from '@/components/ui/Button';
import { ButtonProps } from '@/components/ui/Button';
```

**✅ Right — import directly from the package:**

```ts
import { Button } from '@ajh/ui';
import type { ButtonProps } from '@ajh/ui';
```

ESLint errors on `@/components/ui/*` imports for components that exist in `@ajh/ui`.
`UpdateBanner` is the only exception — it is app-specific and not in `@ajh/ui`.

#### Do you need to import ButtonProps at all?

Usually **no**. Prefer inferring props from the component:

```ts
// ✅ Preferred — inferred, always in sync
type MyProps = React.ComponentProps<typeof Button>;

// ✅ OK when you need to extend the type for a wrapper component
import type { ButtonProps } from '@ajh/ui';
interface MyButtonProps extends ButtonProps { extraProp: string; }

// ❌ Avoid — importing named types just to re-use them without extension
import type { ButtonProps } from '@ajh/ui';
const fn = (props: ButtonProps) => { ... };  // use React.ComponentProps instead
```

Only import a named type from `@ajh/ui` when you are extending it or it is a
non-obvious type like `ToastVariant`, `ThemeId`, or `ToastItem`.

---

### 7. Import ordering — enforced by ESLint (auto-fixable)

Imports must be sorted in this group order, each group separated by a blank line:

```ts
// 1. Node built-ins
import { readFile } from 'node:fs';

// 2. External packages
import { motion } from 'framer-motion';

// 3. @ajh/* packages
import { Button, Input } from '@ajh/ui';

// 4. App aliases (@/)
import { useJobs } from '@/services';
import { transition } from '@/lib/motion';

// 5. Relative imports
import { MyHelper } from './utils';
```

Run `pnpm lint:fix` to auto-sort. Do not manually reorder if the linter will fix it.

---

### 8. Type imports — always use `import type` for pure types

**❌ Wrong — runtime import of a pure type:**

```ts
import { ButtonProps } from '@ajh/ui';
```

**✅ Right:**

```ts
import type { ButtonProps } from '@ajh/ui';
```

`@typescript-eslint/consistent-type-imports` with `fixStyle: inline-type-imports`
enforces this and auto-fixes it. Never suppress this rule.

---

### 9. File placement — features vs shared

```
renderer/
  features/          ← components owned by ONE route
  components/ui/     ← shared primitives (all re-export from @ajh/ui; UpdateBanner is the exception)
  components/layout/ ← app chrome (Sidebar, Titlebar, StatusBar, PageShell)
  services/          ← React Query hooks for all IPC namespaces
  lib/               ← pure utilities (cn, motion, theme, greeting, machine, i18n)
  hooks/             ← shared React hooks
  providers/         ← React context providers
  lib/machines/      ← state machine definitions
  store/             ← Zustand stores (app-store, preferences-store)
```

**When adding a new component:**

- Used by one feature → `features/feature-name/components/`
- Used by multiple features → add to `packages/ui` and export from `@ajh/ui`
- App chrome / layout → `components/layout/`
- Never import across feature directories — features are not allowed to depend on each other's internals

---

### 10. State machines for complex flows

Multi-step UI with error recovery (wizards, AI generation flows) must use state machines.

```ts
import { useMachine } from '@/hooks/use-machine';
import { aiGenerateMachine } from '@/lib/machines/ai-generate.machine';

const [state, send, { busy, error }] = useMachine(aiGenerateMachine, 'idle');
send('SUBMIT');
```

Add new machines in `lib/machines/` for any flow with 3+ states.

---

### 11. Data fetching — React Query via service hooks

No manual `useState + useEffect` for remote data. Every IPC call goes through a service hook.

```ts
import { useDocuments, useImportDocument } from '@/services';

const { data, isLoading, error } = useDocuments();
const importDoc = useImportDocument();
importDoc.mutate(req);
```

---

### 12. Package boundaries

- `packages/shared` — no React, no Node-specific APIs
- `packages/ui` — no app logic (no Zustand, no IPC, no routing)
- `packages/prompts` — no UI imports, no `window` access
- Renderer code **NEVER** imports from `@ajh/core`, `@ajh/ai`, `@ajh/data`, `@ajh/workers`

ESLint errors on main-process package imports in any renderer file.

---

### 13. New IPC capabilities

1. Add method signature to `packages/shared/src/ipc/contracts.ts`
2. Implement in `apps/desktop/src/main/ipc/router.ts`
3. Expose in `apps/desktop/src/preload/index.ts`
4. Create a service hook in `apps/desktop/src/renderer/services/`
5. Add a query key to `services/query-client.ts`

Never skip steps 1–3. The contract is the single source of truth.

---

### 14. Never bypass ESLint

- Never add `// eslint-disable` comments — all inline suppression is banned by `noInlineConfig: true`
- Never use `@ts-ignore` or `@ts-expect-error` without a team discussion
- If a rule needs an exception, add it to `eslint.config.mjs` as a scoped override with a comment explaining why
- `pnpm lint:strict` runs in CI with `--max-warnings 0` — any warning fails the build

---

## Quick Reference

| What                | Where                                      |
| ------------------- | ------------------------------------------ |
| IPC contract        | `packages/shared/src/ipc/contracts.ts`     |
| Service hooks       | `apps/desktop/src/renderer/services/`      |
| UI package          | `packages/ui/src/index.ts` → `@ajh/ui`     |
| UI re-exports       | `apps/desktop/src/renderer/components/ui/` |
| Motion tokens       | `apps/desktop/src/renderer/lib/motion.ts`  |
| State machines      | `apps/desktop/src/renderer/lib/machines/`  |
| Design tokens (CSS) | `packages/ui/src/css/tokens.css`           |
| i18n wrapper        | `apps/desktop/src/renderer/lib/i18n.ts`    |
| Full architecture   | `docs/ARCHITECTURE.md`                     |
| Patterns & rules    | `docs/PATTERNS.md`                         |
| Design system       | `docs/DESIGN_SYSTEM.md`                    |
| Dev setup           | `docs/DEVELOPMENT.md`                      |

## Release Pipeline

Releases are **fully automated** via semantic-release on every push to `main`.

| Commit prefix                                  | Triggers              |
| ---------------------------------------------- | --------------------- |
| `feat:`                                        | minor release (1.x.0) |
| `fix:`, `perf:`                                | patch release (1.0.x) |
| `BREAKING CHANGE` footer                       | major release (x.0.0) |
| `refactor:`, `docs:`, `chore:`, `ci:`, `test:` | no release            |

**Do NOT manually tag releases** — let semantic-release handle versioning.
**Do NOT edit CHANGELOG.md or bump versions in package.json** — semantic-release does this.
