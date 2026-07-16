# AI Job Hunter — JetBrains AI Guidelines

Local-first Tauri desktop app. pnpm monorepo. React 19 + TypeScript strict.

Canonical rules: `CLAUDE.md` (single source of truth — these guidelines are a load-bearing subset; CLAUDE.md wins on conflict).

## Key rules

### IPC — always use service hooks

```ts
// ❌ window.api.documents.list()
// ✅ import { useDocuments } from '@/services';
```

### i18n — always use the adapter

```ts
// ❌ import { useTranslation } from 'react-i18next';
// ✅ import { useTranslation } from '@ajh/translations';
```

### Motion — always use tokens

```ts
// ❌ transition={{ duration: 0.18, ease: [0.22,1,0.36,1] }}
// ✅ import { transition } from '@ajh/ui'; → transition={transition.normal}
```

### Colors — always use brand tokens

```tsx
// ❌ text-[#c084fc]    ✅ text-brand-soft
// ❌ bg-[#a855f7]/15   ✅ bg-brand/15
```

### UI primitives

Button · Input · TextArea · SelectDropdown · ModalShell · GlassCard ·
EmptyState · LoadingSkeleton · StreamingText → all from `@/components/ui/`

### Data fetching

React Query via service hooks only. No `useState + useEffect` for remote data.

### State machines

```ts
import { useMachine } from '@/hooks/use-machine';
// machine definitions in lib/machines/
```

### File placement

```
features/X/    → scoped to one route
components/ui/ → shared
services/      → IPC + React Query
lib/machines/  → state machines
```

### Allowed renderer imports

✅ @ajh/ui @ajh/shared @ajh/prompts @ajh/translations

Backend/business logic lives in the Rust core (`apps/desktop/src-tauri/`) — reachable from the
renderer only via IPC service hooks, never imported directly.

## Full documentation

- `docs/ARCHITECTURE.md` — complete system architecture and data flows
- `docs/PATTERNS.md` — all enforced coding patterns with examples
- `docs/DESIGN_SYSTEM.md` — UI tokens, glass surfaces, motion
- `docs/DEVELOPMENT.md` — setup, commands, debugging
- `CLAUDE.md` § Release & commits — release pipeline and commit conventions

### Release

Manual only — Actions → "🚀 Release" → `action: release`; nothing auto-runs on push to `main`. semantic-release derives the version bump from commit types.
Never manually tag or edit CHANGELOG.md/package.json versions.
