# ADR-046 — Connection-status panel parity: a shared module, live push over poll

**Status:** Accepted

**Date:** 2026-09-05

**Deciders:** owner, main session (implementation)

## Context

The popup has always shown a distinct view per connection phase — app not running, not paired, wrong token, outdated desktop, searching, connected — computed by `computeStatus()` (`apps/extension/src/background.ts`) and pushed to open extension pages via `broadcastStatus()`. The side panel had NONE of this: it always rendered the Answer/Job tools as if connected, so a click against a desktop app that wasn't running or paired just silently failed instead of the panel proactively saying so.

[ADR-045](adr-045-job-tools-panel-parity-and-trust-gate.md) already established the standing pattern this repo uses for panel parity — extract a shared `mount*(host, deps): *View` module the popup and panel both call, rather than fork a second implementation. This record applies that same pattern to connection status, and adds the one decision that pattern doesn't already answer: how the panel should learn about status changes, given it has no popup-style open/close lifecycle to hang a first-fetch-plus-listen model on.

## Decision

**1. A new shared module owns the connection-status chrome; each caller keeps its own connected content.** `apps/extension/src/connection-status/connection-status.ts` (`mountConnectionStatus`) is the single source for everything that isn't phase-specific to a caller, moved out of `popup.ts` without a behavior change. What each caller shows while connected stays with the caller — the module has no opinion on it, exactly as ADR-045 keeps "Mark as applied" out of `job-tools.ts`. See the module's own doc comment and exported types for the current DOM/API shape; this record does not restate it.

**2. A small set of narrow callbacks, not a full render-prop.** `ConnectionStatusDeps` exposes exactly the hooks the two current callers need (a per-render sync point, a once-per-connect hook, a once-per-pair hook) — mirroring `JobToolsDeps`'s shape (ADR-045 decision 5) rather than a generic status-subscription API. Each caller supplies only the callbacks it has a use for. See the module's own `ConnectionStatusDeps` doc comments for what each one fires on, and `popup.ts`/`sidepanel.ts` for how each caller uses them — those are the authoritative, non-drifting source.

**3. Live push, not popup's poll-plus-push, for BOTH surfaces.** The popup historically bounded its first status fetch with a timeout because a popup can close mid-flow; the panel has no such lifecycle — it is created once per window and stays mounted. Rather than give the panel its own poll loop, both callers share one `start()` that does the identical bounded-first-fetch-then-listen sequence, backed by the SAME background broadcast both surfaces already reach. A poll loop the panel would otherwise need to invent for itself is strictly worse: no cheaper, no more current, and a second implementation to keep in sync.

**4. The panel's composition is a single boolean gate, not a state machine.** The panel shows its connected content only while the module reports the connected phase, and the module's own non-connected views cover every other phase — no separate transition tracking needed on the panel's side. See `sidepanel.ts` for the current wiring.

## Alternatives considered

1. **A thinner "click the popup" pointer in the panel** instead of full view parity. Rejected — the owner explicitly confirmed full parity (pairing/offline/outdated screens replicated, not just referenced) after the bug report.
2. **A panel-specific poll loop** (mirroring `refreshUntilSettled`'s post-pairing safety net) as the panel's primary sync mechanism. Rejected: the panel has no reason to distrust the live push the way a JUST-REOPENED popup might have missed one while closed — `refreshUntilSettled` stays exactly where it was, used only by `savePairing()`'s post-pair settle window, identically for both callers.
3. **`surface: 'popup' | 'panel'` flag on `ConnectionStatusDeps`** to special-case the callback wiring. Rejected for the same reason ADR-045 rejected it for the trust gate: the module must be correct for either caller from its own logic, not a branch on which caller it is — a caller simply omits `onConnected`/`onPaired` when it has no use for them.

## Consequences

### Positive

- **The panel gains real connection-status awareness** — the exact bug reported: every phase (not just "connected") now renders an explanation instead of a silently-failing button.
- **`popup.ts` shrank substantially** with no behavioral change — its own tests now cover its callback wiring instead of re-deriving view logic the shared module owns.
- **One CSS surface stays true.** `sidepanel.ts` already imported the popup's stylesheet directly (ADR-044); the module's DOM reuses it as-is, so no new stylesheet was needed.

### Tradeoffs

- **The popup's static markup for the moved sections is gone**, replaced by empty host elements the module fills at runtime — anything that reached those elements directly (tests, a future hand-edit) now goes through the module's own build step instead. Verified against the existing popup test suite (moved, not deleted — see `connection-status.test.ts`), which still passes.
- **A caller-ordering dependency inside `popup.ts`** between the module's own listener registration and the popup's own remaining listener — documented at the call site, not enforceable by the module itself.

## References

- The shared component: `apps/extension/src/connection-status/connection-status.ts` (+ co-located `connection-status.test.ts`)
- Callers: `apps/extension/src/popup/popup.ts`, `apps/extension/src/sidepanel/sidepanel.ts`
- The push this reuses: `broadcastStatus()`/`computeStatus()` (`apps/extension/src/background.ts`)
- Prior decisions: [ADR-044](adr-044-extension-answer-tools-side-panel-and-popup.md) (the panel itself, the shared-stylesheet decision), [ADR-045](adr-045-job-tools-panel-parity-and-trust-gate.md) (the extraction pattern this record reuses)
