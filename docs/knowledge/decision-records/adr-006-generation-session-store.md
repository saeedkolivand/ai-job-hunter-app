# ADR-006: Single app-wide generation-session store

Last updated: 2026-07-16

**Status:** Accepted

## Context

Generation (résumé tailoring, cover letter) can be triggered from a modal (autopilot apply flow) and must survive the modal being closed or the user navigating away — desktop users expect background work to continue and results to be retrievable. Component-local state is lost on unmount; multiple isolated stores would diverge.

## Decision

A single [Zustand][zustand] store (`apps/desktop/src/renderer/store/generation-store/`) holds all active and completed generation sessions. Each session is keyed by a **caller-supplied context id** (e.g. `autopilot:<jobUrl>`). The store is the canonical source for any cross-surface generation state — `GenerationSession` tracks phase, streamed text (`resumeOut`, `coverOut`), reasoning (`thinking`), error, and result metadata. Surfaces read from and write to this store; they never duplicate session state locally.

## Consequences

- Closing a modal or navigating away does not cancel or lose an in-progress generation.
- Multiple surfaces displaying the same generation (e.g. modal + background indicator) stay in sync automatically.
- Context ids must be stable and collision-free within a session; callers own this convention.
- The store outlives any single component mount, so stale sessions must be explicitly cleared when no longer needed.

[zustand]: https://github.com/pmndrs/zustand
