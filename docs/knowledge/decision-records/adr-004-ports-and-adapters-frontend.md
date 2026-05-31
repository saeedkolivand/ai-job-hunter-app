# ADR-004: Ports & adapters in the renderer

**Status:** Accepted · See [`docs/PATTERNS.md`](../../PATTERNS.md), [`docs/DESIGN_SYSTEM.md`](../../DESIGN_SYSTEM.md)

## Context

Direct `window.api.*` calls scattered through UI components couple presentation to transport, defeat caching, and make testing require a live backend.

## Decision

The renderer talks to the shell **only** through service hooks (`renderer/services/`, React Query) over the `AppClient` context — never `window.api.*` in `features/`, `routes/`, or `components/`. Remote data uses React Query, not `useState + useEffect`. Features are isolated (no cross-feature imports). ESLint enforces these.

## Consequences

- Components are testable with a mock `AppClient` (`renderer/test-support.tsx`); caching/invalidation is centralized.
- New IPC consumption = a service hook + query key (`tauri-standards`).
- Violations are HIGH findings (`frontend-reviewer`) and mostly ESLint-blocked.
