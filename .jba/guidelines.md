# AI Job Hunter — JetBrains AI Guidelines

Local-first Electron desktop app. pnpm monorepo. React 19 + TypeScript strict.

## Key rules

### IPC — always use service hooks

```ts
// ❌ window.api.documents.list()
// ✅ import { useDocuments } from '@/services';
```

### i18n — always use the adapter

```ts
// ❌ import { useTranslation } from 'react-i18next';
// ✅ import { useTranslation } from '@/lib/i18n';
```

### Motion — always use tokens

```ts
// ❌ transition={{ duration: 0.18, ease: [0.22,1,0.36,1] }}
// ✅ import { transition } from '@/lib/motion'; → transition={transition.normal}
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

✅ @ajh/ui @ajh/shared @ajh/prompts
❌ @ajh/core @ajh/ai @ajh/data @ajh/workers (main process only)

## Full documentation

- `docs/ARCHITECTURE.md` — complete system architecture and data flows
- `docs/PATTERNS.md` — all enforced coding patterns with examples
- `docs/DESIGN_SYSTEM.md` — UI tokens, glass surfaces, motion
- `docs/DEVELOPMENT.md` — setup, commands, debugging
- `docs/RELEASE.md` — release pipeline and commit conventions

### Release

Automated via semantic-release on push to `main`.
Never manually tag or edit CHANGELOG.md/package.json versions.
