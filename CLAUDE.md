# AI Job Hunter — Project Rules for AI Assistants

Read this before writing any code. These rules are enforced by ESLint, TypeScript,
and CI — violations will block commits and fail the build.

---

## Shell Commands

**Always prefix every shell command with `rtk`.**

```bash
# ✅ correct
git status
git add -A
git commit -m "..."
npx tsc --noEmit
pnpm install

# ❌ never
git status
npx tsc --noEmit
pnpm install
```

rtk is a token-optimising proxy installed on this machine. It filters verbose
output and saves 60-90% of context on dev operations. No exceptions.

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
ESLint will warn if you call `window.api.*` in features/, routes/, or components/.

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
ESLint will warn on any `[#RRGGBB]` in className strings.

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
ESLint will warn on inline transition objects in feature/route files.

---

### 5. UI primitives — always use shared components

Never use raw HTML elements for interactive UI. Use the shared primitives:

| Need            | Use                                                                       |
| --------------- | ------------------------------------------------------------------------- |
| Button / action | `<Button>` from `@/components/ui/Button`                                  |
| Text input      | `<Input>` from `@/components/ui/Input`                                    |
| Textarea        | `<TextArea>` from `@/components/ui/TextArea`                              |
| Dropdown        | `<SelectDropdown>` from `@/components/ui/SelectDropdown`                  |
| Modal / dialog  | `<ModalShell>` from `@/components/ui/ModalShell`                          |
| Confirm dialog  | `<ConfirmModal>` from `@/components/ui/ConfirmModal`                      |
| Empty state     | `<EmptyState>` from `@/components/ui/EmptyState`                          |
| Error state     | `<ErrorState>` from `@/components/ui/ErrorState`                          |
| Loading rows    | `<RowSkeleton>` / `<CardSkeleton>` from `@/components/ui/LoadingSkeleton` |
| Card surface    | `<GlassCard>` from `@/components/ui/GlassCard`                            |
| Settings card   | `<SettingsSection>` from `@/components/ui/SettingsSection`                |
| Selectable tile | `<OptionTile>` from `@/components/ui/OptionTile`                          |
| Page wrapper    | `<PageShell>` from `@/components/layout/PageShell`                        |
| AI text stream  | `<StreamingText>` from `@/components/ui/StreamingText`                    |

---

### 6. File placement — features vs shared

```
renderer/
  features/        ← components owned by ONE route (dashboard, settings, support, ai-workspace)
  components/ui/   ← shared primitives used by MULTIPLE features
  components/layout/ ← app chrome (Sidebar, Titlebar, StatusBar, PageShell)
  services/        ← React Query hooks for all IPC namespaces
  lib/             ← pure utilities (cn, motion, theme, greeting, machine, i18n)
  hooks/           ← shared React hooks
  providers/       ← React context providers
  lib/machines/    ← state machine definitions
  store/           ← Zustand stores (app-store, preferences-store)
```

**When adding a new component:**

- Used only by one feature page → `features/feature-name/components/`
- Used by multiple features → `components/ui/`
- App chrome / layout → `components/layout/`

---

### 7. State machines for complex flows

Multi-step UI with error recovery (wizards, AI generation flows) must use state machines.

```ts
import { useMachine } from '@/hooks/use-machine';
import { aiGenerateMachine } from '@/lib/machines/ai-generate.machine';

const [state, send, { busy, error }] = useMachine(aiGenerateMachine, 'idle');
send('SUBMIT'); // transitions state; invalid transitions are no-ops
```

Existing machines: `ai-generate.machine.ts`, `autopilot-wizard.machine.ts`.
Add new machines in `lib/machines/` for any flow with 3+ states.

---

### 8. Data fetching — React Query via service hooks

No manual `useState + useEffect` for remote data. Every IPC call goes through a service hook.

```ts
// All service hooks are in @/services/
import { useDocuments, useImportDocument } from '@/services';

const { data, isLoading, error } = useDocuments();
const importDoc = useImportDocument();
importDoc.mutate(req); // cache auto-invalidated on success
```

---

### 9. Package boundaries

- `packages/shared` — no React, no Node-specific APIs
- `packages/ui` — no app logic (no Zustand, no IPC, no routing)
- `packages/prompts` — no UI imports, no `window` access
- Renderer code NEVER imports from `packages/core`, `packages/ai`, or `packages/data`
  (those are main-process-only packages)

---

### 10. New IPC capabilities

If you need a new backend capability:

1. Add method signature to `packages/shared/src/ipc/contracts.ts`
2. Implement in `apps/desktop/src/main/ipc/router.ts`
3. Expose in `apps/desktop/src/preload/index.ts`
4. Create a service hook in `apps/desktop/src/renderer/services/`
5. Add a query key to `services/query-client.ts`

Never skip steps 1–3. The contract is the single source of truth.

---

## Quick Reference

| What                | Where                                      |
| ------------------- | ------------------------------------------ |
| IPC contract        | `packages/shared/src/ipc/contracts.ts`     |
| Service hooks       | `apps/desktop/src/renderer/services/`      |
| UI primitives       | `apps/desktop/src/renderer/components/ui/` |
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

**What happens on a triggering push:**

1. `release.yml` runs `semantic-release` → creates GitHub Release + git tag + updates CHANGELOG.md
2. `build.yml` is triggered → builds Windows (NSIS), Linux (AppImage+deb), macOS (dmg+zip) installers
3. All installers are attached to the GitHub Release automatically

**Do NOT manually tag releases** — let semantic-release handle versioning.
**Do NOT edit CHANGELOG.md or bump versions in package.json** — semantic-release does this.
