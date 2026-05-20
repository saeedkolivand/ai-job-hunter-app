# AI Job Hunter — Copilot Instructions

Local-first Electron desktop app. pnpm monorepo. React 19 + TypeScript strict.

## Critical rules

**IPC boundary:** Renderer never calls `window.api.*` directly. Use service hooks:

```ts
import { useDocuments, useJobQueue, useAIModels } from '@/services';
```

**i18n:** Always `import { useTranslation } from '@/lib/i18n'` — never from `react-i18next`.

**Motion:** Always `import { transition } from '@/lib/motion'` — never inline `{ duration: 0.18 }`.

**Colors:** Always `text-brand-soft`, `bg-brand`, `border-brand` — never `text-[#c084fc]`.

**Components:** Use shared primitives — `Button`, `Input`, `TextArea`, `SelectDropdown`,
`ModalShell`, `GlassCard`, `EmptyState`, `LoadingSkeleton`, `StreamingText`, `OptionTile`.

**Data fetching:** React Query via service hooks — never `useState + useEffect` for remote data.

**Complex flows:** State machines via `useMachine` from `@/hooks/use-machine`.

## File placement

- `renderer/features/X/components/` — scoped to one route
- `renderer/components/ui/` — shared across features
- `renderer/services/` — all IPC + React Query hooks
- `renderer/lib/machines/` — state machine definitions

## Package imports (renderer only)

✅ `@ajh/ui`, `@ajh/shared`, `@ajh/prompts`
❌ `@ajh/core`, `@ajh/ai`, `@ajh/data`, `@ajh/workers` — main process only

## Full documentation

| Topic                       | File                    |
| --------------------------- | ----------------------- |
| Architecture & data flows   | `docs/ARCHITECTURE.md`  |
| All coding patterns & rules | `docs/PATTERNS.md`      |
| Design system & tokens      | `docs/DESIGN_SYSTEM.md` |
| Dev setup & commands        | `docs/DEVELOPMENT.md`   |
| Release pipeline            | `docs/RELEASE.md`       |

## Release

Releases are automated via semantic-release on push to `main`.
`feat:` → minor, `fix:`/`perf:` → patch, `BREAKING CHANGE` → major.
Never manually tag, edit CHANGELOG.md, or bump versions in package.json.
