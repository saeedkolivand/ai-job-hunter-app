---
name: frontend-standards
description: React renderer standards — ports & adapters (service hooks), design tokens, motion tokens, @ajh/ui primitives, i18n, feature isolation, React Query, a11y. Load for changes under apps/tauri/src/renderer/**.
---

# Frontend standards (mostly ESLint-enforced)

Authoritative: `docs/DESIGN_SYSTEM.md`, `docs/PATTERNS.md`.

## Ports & adapters (HIGH if violated)

- **No `window.api.*`** in `features/`, `routes/`, `components/` — use service hooks from `renderer/services/` (React Query). ESLint errors on direct access.
- **Data fetching** — React Query via service hooks only; no `useState + useEffect` for remote data.

## Design system

- **Tokens** — `text-brand`/`bg-brand`/`border-brand`/`ring-brand`; CSS vars `var(--color-brand)`. No `[#RRGGBB]` in className.
- **Motion** — `import { transition } from '@/lib/motion'` (`.fast/.normal/.relaxed/.slow/.spring/.modal/.overlay`); no inline `{ duration, ease }`.
- **Primitives** — `@ajh/ui` (`Button`, `Input`, `TextArea`, `SelectDropdown`, `ModalShell`, `GlassCard`, `EmptyState`, …). No raw `<button>/<select>/<textarea>` (exception: `<input type="range|file|checkbox|radio|hidden">`).
- **Imports** — import `@ajh/ui` directly, not `@/components/ui/*` (except `UpdateBanner`).

## i18n (HIGH if user-facing text is unwrapped)

Import from `@/lib/i18n`, never `react-i18next` directly. All user-facing strings localized.

## Accessibility

Interactive controls keyboard-reachable + labeled; focus management on modals; sufficient contrast (use tokens).

## Structure

- `features/*` own one route — never import across feature dirs.
- 3+ states → a state machine in `lib/machines/` via `useMachine`.
- File placement per `CLAUDE.md` §9.
