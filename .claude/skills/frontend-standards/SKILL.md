---
name: frontend-standards
description: React renderer standards — ports & adapters (service hooks), design tokens, motion tokens, @ajh/ui primitives, i18n, feature isolation, React Query, a11y. Load for changes under apps/tauri/src/renderer/**.
---

# Frontend standards (mostly ESLint-enforced)

Authoritative: `docs/DESIGN_SYSTEM.md`, `docs/PATTERNS.md`.

## Ports & adapters (HIGH if violated)

- **No `window.api.*`** in `features/`, `routes/`, `components/` — use service hooks from `renderer/services/` (React Query). ESLint errors on direct access.
- **Data fetching** — React Query via service hooks only; no `useState + useEffect` for remote data.

## React Query & async mutations (HIGH on correctness)

- **Invalidation must match** — `invalidateQueries({ queryKey })` is a **prefix** match; a key factory that appends a trailing `undefined`/optional segment (`keys.x.y()` → `['x','y',undefined]`) will **not** match the typed queries (`['x','y','viewed']`). Fix it with the **correct key**: the shorter prefix (`['x','y']`), or a deliberate `predicate` — note `exact: true` does **not** fix it (an exact match on the wrong shape `['x','y',undefined]` still misses `['x','y','viewed']`). After a mutation, confirm the key you invalidate hits the queries that render the affected UI (#486: shipped this twice → stale viewed/saved badges).
- **Mutations** — never show success / apply optimistic state / emit tracking events **before** the mutation resolves. Two cases: (a) **awaited** — `await mutateAsync()`, then run the success effects, in `try/catch`; (b) **detached** (`void`/fire-and-forget) — attach a `.catch` surfacing a fixed `@ajh/translations` error key (never raw `err.message`) and put the success effects in `.then`, not on the line after the call.
- **Nullable at the IPC boundary** — guard a contract-optional/`null` value (a field a command may return absent) before string/array ops (`.toLowerCase()`/`[0]`/`.split()`).

## Design system

- **Tokens** — `text-brand`/`bg-brand`/`border-brand`/`ring-brand`; CSS vars `var(--color-brand)`. No `[#RRGGBB]` in className.
- **Motion** — `import { transition } from '@ajh/ui'` (`.fast/.normal/.relaxed/.slow/.spring/.modal/.overlay`); no inline `{ duration, ease }`.
- **Primitives** — `@ajh/ui` (`Button`, `Input`, `TextArea`, `SelectDropdown`, `ModalShell`, `GlassCard`, `EmptyState`, …). No raw `<button>/<select>/<textarea>` (exception: `<input type="range|file|checkbox|radio|hidden">`).
- **Imports** — import `@ajh/ui` directly, not `@/components/ui/*` (except `UpdateBanner`).

## i18n (HIGH if user-facing text is unwrapped)

Import `useTranslation` / `TFunction` from `@ajh/translations`, never `react-i18next` directly (the renderer init shim is `@/i18n`). All user-facing strings localized.

## Accessibility (WCAG 2.2 AA floor — non-negotiable; the design-audit recurring gaps)

- **Visible focus** — every interactive element has a `:focus-visible` ring (≥2px, `outline-offset`, visible against its bg). NEVER strip native focus with `all: unset` / `border: none` / `background: none` without replacing it; a focusable custom widget needs more than a stroke/color shift.
- **Reduced motion** — gate every animation/transition behind `@media (prefers-reduced-motion: reduce)`; a blanket `* { animation: none }` must NOT also kill focus-visible transitions. Reveal-on-scroll under reduce: drop the translate (no positional jump), keep opacity.
- **Contrast** — meet AA (4.5:1 normal, 3:1 for ≥24px/bold). Muted text on light/"paper" backgrounds is the usual failure (the audit found ≈3.3–3.8:1) — darken the muted token, don't ship it.
- **Real controls** — prefer an `@ajh/ui` primitive (`Button`/`Switch`/…) over a hand-rolled control; raw `<button>` is banned in the renderer (see Primitives). If a click handler must live on a `<div>`/`<span>`, it needs `role="button"` + React `tabIndex={0}` + an `onKeyDown` Enter/Space handler. Icon-only controls need `aria-label` (not just `title`); toggle/filter/segmented controls need `aria-pressed`.
- **Dialogs/overlays** — a custom modal needs `role="dialog"`/`"alertdialog"` + `aria-modal="true"` + `aria-labelledby`/`-describedby` + focus management/trap; transient banners get `role="status"` + `aria-live`.
- **SVG** — informative SVG: `role="img"` + `aria-label` on the root, no conflicting roles on children; decorative SVG `aria-hidden="true"`. A focusable SVG node needs a real visible focus indicator.
- **aria reference guards** — `aria-controls`/`aria-*` id references use the **same** render guard as the element they point to (no reference to never-rendered DOM); Enter/Space activation gates on the same enable/disable predicate as the click.

## Structure

- `features/*` own one route — never import across feature dirs.
- 3+ states → a state machine in `lib/machines/` via `useMachine`.
- File placement per `CLAUDE.md` §9.

## External standards & best-practices (verified 2026-06-19)

> React 19 + **React Compiler 1.0 GA** (2025-10-07). WCAG **2.2 AA** is the only a11y target (3.0 is a 2026 Working Draft — not yet). Core Web Vitals triad unchanged: **LCP / INP / CLS**.

- **React 19** — Actions/`useActionState`/`useOptimistic` for async transitions; `use(resource)` reads a Promise/Context (suspends); `ref` is a plain prop (`forwardRef` deprecated). https://react.dev/blog/2024/12/05/react-19
- **You Might Not Need an Effect** — NO effect for: deriving data for render (compute in body / `useMemo`), resetting state on prop change (use `key`), or event-driven side effects (put in the handler). Effect only to sync with an external system. Test: "because shown" → effect; "because user did X" → handler. https://react.dev/learn/you-might-not-need-an-effect
- **React Compiler 1.0** — build-time auto-memoization; `useMemo`/`useCallback`/`memo` become measured escape hatches, not defaults; requires Rules-of-React (run the ESLint plugin). https://react.dev/blog/2025/10/07/react-compiler-1
- **WCAG 2.2 AA** most-relevant SC for a keyboard-driven SPA: 2.4.7 Focus Visible + 2.4.3 Focus Order; **2.4.11 Focus Not Obscured** (new 2.2 — sticky chrome must not hide focus); **2.5.8 Target Size ≥ 24×24px** (new); **2.5.7 Dragging Movements** has a single-pointer alternative; 4.1.2 Name/Role/Value; honor `prefers-reduced-motion`. https://www.w3.org/WAI/standards-guidelines/wcag/new-in-22/
- **ARIA APG** — build dialog/menu/tabs/combobox/listbox/disclosure/accordion/tooltip to the APG roles+states+keyboard spec; don't hand-roll. https://www.w3.org/WAI/ARIA/apg/patterns/
- **Core Web Vitals** (p75): LCP ≤ 2.5s · INP ≤ 200ms · CLS ≤ 0.1 (INP replaced FID in 2024 — ignore SEO blogs claiming a 2.0s LCP). https://web.dev/articles/vitals

**Common mistakes:** effects for derived state / event reactions; hand-sprinkling `useMemo`/`useCallback` with Compiler on (or ripping existing ones out); keeping `forwardRef` boilerplate on new components; custom widgets missing role/state/name or APG keyboard handling; targets < 24×24px or focus obscured by sticky chrome; ignoring `prefers-reduced-motion`; treating WCAG 3.0/APCA as a current requirement.
