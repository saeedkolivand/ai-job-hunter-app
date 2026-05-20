# Patterns & Best Practices — AI Job Hunter

All patterns below are ESLint-enforced and block commits on violation.

---

## 1. Ports & Adapters — service hooks only

Renderer calls `window.api.*` only through service hooks in `@/services`. Never call it directly in components or routes.

```ts
// ❌  import { useDocuments } from '@/services';  →  ✅
const { data, isLoading } = useDocuments();
```

---

## 2. i18n — always import through the adapter

```ts
import { useTranslation } from '@/lib/i18n'; // ✅
// Never: import { useTranslation } from 'react-i18next'
```

Language resolution: persisted preference → system locale → `'en'` fallback.

For interpolation with options: `(t as (k: string, o: Record<string, unknown>) => string)('key', { value })`.

---

## 3. Motion — tokens, never inline objects

```ts
import { transition, variants } from '@/lib/motion';
<motion.div transition={transition.normal} {...variants.fadeSlideUp}>
```

| Token                | Use                       |
| -------------------- | ------------------------- |
| `transition.fast`    | Hover, micro interactions |
| `transition.normal`  | Component mounts          |
| `transition.relaxed` | Panels, drawers           |
| `transition.modal`   | Modals                    |
| `transition.spring`  | Nav pill, drag            |
| `transition.overlay` | Backdrop fade             |

Use `<AnimatePresence mode="wait">` when swapping mutually exclusive views.

---

## 4. Brand colors — tokens only

```ts
// ✅  text-brand  text-brand-soft  bg-brand  border-brand  ring-brand
// ❌  text-[#c084fc]  bg-[#a855f7]/15
```

CSS vars: `var(--color-brand)`, `var(--color-brand-soft)`.

---

## 5. UI primitives — always from `@ajh/ui`

| Need             | Component                                        |
| ---------------- | ------------------------------------------------ |
| Button           | `Button`                                         |
| Input / Textarea | `Input` / `TextArea`                             |
| Dropdown         | `SelectDropdown`                                 |
| Modal / Confirm  | `ModalShell` / `ConfirmModal`                    |
| Empty / Error    | `EmptyState` / `ErrorState`                      |
| Skeletons        | `RowSkeleton` / `CardSkeleton`                   |
| Card / Settings  | `GlassCard` / `SettingsSection`                  |
| Tile / Stream    | `OptionTile` / `StreamingText`                   |
| Page wrapper     | `PageShell` from `@/components/layout/PageShell` |

All from `@ajh/ui` (not `@/components/ui/*`). No raw `<button>`, `<select>`, `<textarea>`.
Exception: `<input type="range|file|checkbox|radio|hidden">`.

---

## 6. Data fetching — React Query via service hooks

No `useState + useEffect` for remote data. Every IPC call has a service hook.

```ts
import { useDocuments, useImportDocument } from '@/services';
const { data, isLoading } = useDocuments();
const importDoc = useImportDocument();
importDoc.mutate(req); // cache auto-invalidated on success
```

Query keys are defined in `services/query-client.ts`. Never hardcode key strings in components.

---

## 7. State machines for complex flows

3+ states with guards or error recovery → use a state machine.

```ts
import { useMachine } from '@/hooks/use-machine';
import { aiGenerateMachine } from '@/lib/machines/ai-generate.machine';
const [state, send, { busy, error }] = useMachine(aiGenerateMachine, 'idle');
```

Existing: `ai-generate.machine.ts`, `autopilot-wizard.machine.ts`. Add new machines to `lib/machines/`.

---

## 8. File placement

```
features/X/components/   ← owned by ONE route
components/layout/       ← Sidebar, Titlebar, PageShell
services/                ← all IPC + React Query hooks
lib/machines/            ← state machines
```

Used by one feature → `features/*/components/`. Used by 2+ → `packages/ui`. Chrome → `components/layout/`.
Features must not import from each other's internal directories.

---

## 9. Package boundaries

| Package            | Restriction                                                      |
| ------------------ | ---------------------------------------------------------------- |
| `packages/shared`  | No React, no Node APIs                                           |
| `packages/ui`      | No Zustand, no IPC, no routing                                   |
| `packages/prompts` | No UI, no `window`                                               |
| Renderer           | Never import `@ajh/core`, `@ajh/ai`, `@ajh/data`, `@ajh/workers` |

---

## 10. New IPC capability — 5-step checklist

1. `packages/shared/src/ipc/contracts.ts` — add signature
2. `packages/shared/src/schemas/index.ts` — add Zod schema
3. `apps/desktop/src/main/ipc/router.ts` — implement handler
4. `apps/desktop/src/preload/index.ts` — expose via contextBridge
5. `apps/desktop/src/renderer/services/` — create React Query hook

---

## 11. Zustand conventions

```ts
// Selector — subscribe to only what you need
const userName = usePreferencesStore((s) => s.userName);

// Named selector exports for common slices
export const useUserName = () => usePreferencesStore((s) => s.userName);

// Direct state write for one-off resets
usePreferencesStore.setState((s) => ({ ...s, onboardingCompleted: false }));
```

Persistence key: `'ai-job-hunter-preferences'`. Always add a migration when bumping store version.

---

## 12. TypeScript

- `strict: true` everywhere
- No `any` — use `unknown` then narrow
- Non-null `!` only when guaranteed by a prior guard
- `as const` on literal arrays/objects used for type inference

---

## 13. Comments

Write no comments by default. Only add one when the **why** is non-obvious: a hidden constraint, a workaround for a specific bug, or a non-obvious invariant. Never comment what the code does.
