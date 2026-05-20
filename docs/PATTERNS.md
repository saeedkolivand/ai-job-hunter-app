# Patterns & Best Practices — AI Job Hunter

This document describes every enforced coding pattern in this project.
Most patterns are backed by ESLint rules that block commits on violation.

---

## 1. Ports & Adapters — never call `window.api.*` in UI code

The renderer communicates with the main process exclusively through **service hooks**.

```ts
// ❌ Direct IPC call in a component, route, or feature
const jobs = await window.api.scrape.listPostings();

// ✅ Use the service hook
import { usePostings } from '@/services';
const { data: jobs, isLoading } = usePostings();
```

Service hooks live in `apps/desktop/src/renderer/services/`.  
They wrap every IPC namespace with React Query.  
ESLint rule `no-restricted-syntax` warns on `window.api.*` in `features/`, `routes/`, and `components/`.

---

## 2. i18n — always import through the adapter

```ts
// ❌ Direct library import
import { useTranslation } from 'react-i18next';

// ✅ Project adapter
import { useTranslation } from '@/lib/i18n';
```

The adapter at `lib/i18n.ts` is the single entry point. If the library ever changes, only that file needs updating. ESLint enforces this.

**Interpolation with options:**

```ts
// t() is typed strictly — cast when passing options
(t as (key: string, opts: Record<string, unknown>) => string)('myKey', { value: someVar });
```

**Language resolution order on startup:**

1. Persisted preference in `usePreferencesStore` (localStorage)
2. System locale via `i18next-browser-languagedetector`
3. Fallback: `'en'`

---

## 3. Motion — always use tokens, never inline objects

```tsx
// ❌ Inline transition object
<motion.div transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}>

// ✅ Token from lib/motion
import { transition } from '@/lib/motion';
<motion.div transition={transition.normal}>
```

**Available tokens:**

| Token                | Use                                |
| -------------------- | ---------------------------------- |
| `transition.fast`    | Micro interactions, hover feedback |
| `transition.normal`  | Component mounts, list items       |
| `transition.relaxed` | Panels, drawers                    |
| `transition.modal`   | Modals, overlays                   |
| `transition.spring`  | Navigation pill, drag              |
| `transition.overlay` | Backdrop fade                      |

**Animation variants:**

```tsx
import { variants } from '@/lib/motion';

// Pre-built enter/exit pairs
<motion.div {...variants.fadeSlideUp}>
<motion.div {...variants.fadeSlideDown}>
<motion.div {...variants.scale}>
```

Always use `<AnimatePresence mode="wait">` when swapping between mutually exclusive views (tabs, wizard steps).

---

## 4. Brand colors — never hardcode hex

```tsx
// ❌ Hardcoded color
<div className="text-[#c084fc] bg-[#a855f7]/15">

// ✅ Brand token
<div className="text-brand-soft bg-brand/15">
```

**Available brand tokens:**

| Class             | Use                              |
| ----------------- | -------------------------------- |
| `text-brand`      | Primary brand purple             |
| `text-brand-soft` | Softer purple for icons, accents |
| `bg-brand`        | Brand background fills           |
| `border-brand`    | Brand borders                    |
| `ring-brand`      | Focus rings                      |

CSS variables: `var(--color-brand)`, `var(--color-brand-soft)`.

ESLint warns on any `[#RRGGBB]` in `className` strings.

---

## 5. UI primitives — always use shared components

Never use raw HTML elements for interactive UI. Use the primitives:

| Need            | Component                          | Import                            |
| --------------- | ---------------------------------- | --------------------------------- |
| Button / action | `<Button>`                         | `@/components/ui/Button`          |
| Text input      | `<Input>`                          | `@/components/ui/Input`           |
| Textarea        | `<TextArea>`                       | `@/components/ui/TextArea`        |
| Dropdown        | `<SelectDropdown>`                 | `@/components/ui/SelectDropdown`  |
| Modal / dialog  | `<ModalShell>`                     | `@/components/ui/ModalShell`      |
| Confirm dialog  | `<ConfirmModal>`                   | `@/components/ui/ConfirmModal`    |
| Empty state     | `<EmptyState>`                     | `@/components/ui/EmptyState`      |
| Error state     | `<ErrorState>`                     | `@/components/ui/ErrorState`      |
| Loading rows    | `<RowSkeleton>` / `<CardSkeleton>` | `@/components/ui/LoadingSkeleton` |
| Card surface    | `<GlassCard>`                      | `@/components/ui/GlassCard`       |
| Settings card   | `<SettingsSection>`                | `@/components/ui/SettingsSection` |
| Selectable tile | `<OptionTile>`                     | `@/components/ui/OptionTile`      |
| Page wrapper    | `<PageShell>`                      | `@/components/layout/PageShell`   |
| AI text stream  | `<StreamingText>`                  | `@/components/ui/StreamingText`   |

---

## 6. Data fetching — React Query via service hooks

No manual `useState + useEffect` for remote data. Every IPC call goes through a service hook.

```ts
// All service hooks live in @/services
import { useDocuments, useImportDocument } from '@/services';

const { data, isLoading, error } = useDocuments();
const importDoc = useImportDocument();
importDoc.mutate(req); // cache auto-invalidated on success
```

**Query key conventions** — defined in `services/query-client.ts`:

```ts
export const QUERY_KEYS = {
  documents: ['documents'],
  jobs: ['jobs'],
  aiModels: ['ai', 'models'],
  // ...
};
```

Invalidate by key after mutations. Never hardcode query key strings in components.

---

## 7. State machines for complex flows

Any multi-step UI with error recovery (wizards, AI generation flows) must use a state machine.

```ts
import { useMachine } from '@/hooks/use-machine';
import { aiGenerateMachine } from '@/lib/machines/ai-generate.machine';

const [state, send, { busy, error }] = useMachine(aiGenerateMachine, 'idle');
send('SUBMIT'); // invalid transitions are silent no-ops
```

**When to use a state machine:**

- Flow has 3 or more distinct named states
- Transitions have guards or side effects
- Illegal state combinations need to be prevented

Existing machines in `lib/machines/`:

- `ai-generate.machine.ts` — resume/cover letter generation flow
- `autopilot-wizard.machine.ts` — autopilot creation wizard

Add new machines here for any new wizard or multi-step flow.

---

## 8. File placement

```
renderer/
  features/          Components owned by ONE route only
  components/ui/     Shared primitives used across multiple features
  components/layout/ App chrome — Sidebar, Titlebar, StatusBar, PageShell
  services/          React Query hooks for all IPC namespaces
  lib/               Pure utilities — cn, motion, theme, greeting, i18n
  hooks/             Shared React hooks
  providers/         React context providers
  lib/machines/      State machine definitions
  store/             Zustand stores
```

**Decision rule:**

- Used only by one feature page → `features/feature-name/components/`
- Used by two or more features → `components/ui/`
- App chrome / layout → `components/layout/`

---

## 9. Package boundaries

| Package            | Restrictions                                                         |
| ------------------ | -------------------------------------------------------------------- |
| `packages/shared`  | No React, no Node-specific APIs                                      |
| `packages/ui`      | No app logic — no Zustand, no IPC, no routing                        |
| `packages/prompts` | No UI imports, no `window` access                                    |
| Renderer code      | Never import from `packages/core`, `packages/ai`, or `packages/data` |

---

## 10. New IPC capabilities — 5-step checklist

1. Add method signature to `packages/shared/src/ipc/contracts.ts`
2. Add Zod schema to `packages/shared/src/schemas/index.ts`
3. Implement handler in `apps/desktop/src/main/ipc/router.ts`
4. Expose via `contextBridge` in `apps/desktop/src/preload/index.ts`
5. Create a service hook in `apps/desktop/src/renderer/services/`

Never skip steps 1–4. The contract and preload are the type boundary.

---

## 11. Zustand store conventions

```ts
// Selector pattern — subscribe to only what you need
const userName = usePreferencesStore((s) => s.userName);

// Named selector exports for common slices
export const useUserName = () => usePreferencesStore((s) => s.userName);

// Direct state write for one-off resets (e.g. re-triggering onboarding)
usePreferencesStore.setState((s) => ({ ...s, onboardingCompleted: false }));
```

Stores with persistence use `zustand/middleware`'s `persist` with `localStorage`.  
The persistence key is `'ai-job-hunter-preferences'`.  
Always add a migration when bumping the store version.

---

## 12. TypeScript strictness

- `strict: true` in all tsconfigs
- No `any` except at unavoidable boundaries (legacy APIs, DB results) — use `unknown` then narrow
- Non-null assertions (`!`) only when the value is guaranteed by an earlier guard
- Use `as const` on literal arrays and objects used for type inference

---

## 13. Comments

Write no comments by default. Only add one when the **why** is non-obvious:

- A hidden constraint or workaround for a specific bug
- A subtle invariant that would surprise a reader
- An explicit decision between two valid approaches

Never comment what the code does — well-named identifiers already do that.
