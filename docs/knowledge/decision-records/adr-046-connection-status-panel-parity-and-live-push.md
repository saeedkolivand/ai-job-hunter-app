# ADR-046 — Connection-status panel parity: a shared module, live push over poll

**Status:** Accepted

**Date:** 2026-09-05

**Deciders:** owner, main session (implementation)

## Context

The popup has always shown a distinct view per connection phase — app not running, not paired, wrong token, outdated desktop, searching, connected — computed by `computeStatus()` (`apps/extension/src/background.ts`) and pushed to open extension pages via `broadcastStatus()`. The side panel had NONE of this: it always rendered the Answer/Job tools as if connected, so a click against a desktop app that wasn't running or paired just silently failed instead of the panel proactively saying so.

[ADR-045](adr-045-job-tools-panel-parity-and-trust-gate.md) already established the standing pattern this repo uses for panel parity — extract a shared `mount*(host, deps): *View` module the popup and panel both call, rather than fork a second implementation. This record applies that same pattern to connection status, and adds the one decision that pattern doesn't already answer: how the panel should learn about status changes, given it has no popup-style open/close lifecycle to hang a first-fetch-plus-listen model on.

## Decision

**1. A new shared module owns the pill/retry + the four non-connected views.** `apps/extension/src/connection-status/connection-status.ts` (`mountConnectionStatus(pillHost, viewsHost, deps): ConnectionStatusView`) builds the retry button + status pill into `pillHost`, and `view-pair` / `view-offline` / `view-outdated` / `view-searching` into `viewsHost` — moved from `popup.ts` essentially unchanged (same wire calls, same phase→view mapping, same offline-sticky/bad_token-message/pairing-confirmation behavior). The "connected" content (`view-import` in the popup, the job/answer tools in the panel) stays OUT of the module — each caller owns what it shows while connected, exactly as ADR-045 keeps "Mark as applied" out of `job-tools.ts`.

**2. Three narrow callbacks, not a full render-prop.** `ConnectionStatusDeps.onStatus` fires on every render (fresh or repeated) — the popup uses it to gate `view-import` and "Unpair this device" and to reset connected-only content; the panel uses it for its ONE responsibility, gating `#view-connected`. `onConnected` fires once per TRANSITION into `connected` — the popup's fire-and-forget `appliedCheck`/`fieldsProbe`/`answerScan` auto-checks hang off it; the panel doesn't need it (its own `follow()`/tab-activation logic already re-checks independently). `onPaired` fires once after a successful pair reaches `connected` — the popup uses it to move focus onto the Import button; the panel has no equivalent focus target and omits it. This mirrors `JobToolsDeps`'s shape (ADR-045 decision 5) rather than inventing a generic status-subscription API neither caller needs.

**3. Live push, not popup's poll-plus-push, for BOTH surfaces.** The popup historically used `refreshStatusWithTimeout()` (first fetch bounded by a timeout) plus an `onMessage` listener for the live `status` push `broadcastStatus()` sends on every bridge-phase change, because a popup can close mid-flow and needs a bounded first paint. The panel has no such lifecycle — it is created once per window and stays mounted — so `start()` (called once, by both callers) does the identical thing: one bounded first fetch, then the SAME `onMessage` listener for the rest of its life. No panel-specific poll loop was added; `broadcastStatus()` already reaches every open extension page (popup or panel) with one `browser.runtime.sendMessage` call, so a genuine push is strictly cheaper and no less current than a poll the panel would otherwise need to invent for itself.

**4. The panel's composition is a single boolean gate, not a state machine.** `sidepanel.ts` wraps the job/answer tools in `#view-connected` and sets `.hidden = status.phase !== 'connected'` from `onStatus` — no separate transition tracking, since the module's four non-connected views already cover every other phase and `[hidden]{display:none}` (via `popup.css`, which the panel already imports unchanged) makes the swap a simple visibility toggle.

## Alternatives considered

1. **A thinner "click the popup" pointer in the panel** instead of full view parity. Rejected — the owner explicitly confirmed full parity (pairing/offline/outdated screens replicated, not just referenced) after the bug report.
2. **A panel-specific poll loop** (mirroring `refreshUntilSettled`'s post-pairing safety net) as the panel's primary sync mechanism. Rejected: the panel has no reason to distrust the live push the way a JUST-REOPENED popup might have missed one while closed — `refreshUntilSettled` stays exactly where it was, used only by `savePairing()`'s post-pair settle window, identically for both callers.
3. **`surface: 'popup' | 'panel'` flag on `ConnectionStatusDeps`** to special-case the callback wiring. Rejected for the same reason ADR-045 rejected it for the trust gate: the module must be correct for either caller from its own logic, not a branch on which caller it is — a caller simply omits `onConnected`/`onPaired` when it has no use for them.

## Consequences

### Positive

- **The panel gains real connection-status awareness** — the exact bug reported: every phase (not just "connected") now renders an explanation instead of a silently-failing button.
- **`popup.ts` shrank substantially** (pill/retry/pairing/offline/outdated/searching logic, `savePairing`/`retry`/`openAppPairing`/`getApp`, all moved out) with no behavioral change — its own tests cover the callback wiring is correct instead of re-deriving the view logic.
- **One CSS surface stays true.** `sidepanel.ts` already imported `popup.css` directly (ADR-044); the module's DOM reuses the exact same classes (`.pill`, `.pill--*`, `.view`, `.token`, `.msg`), so no new stylesheet was needed.

### Tradeoffs

- **The popup's static `popup.html` markup for the pill/retry/pairing/offline/outdated/searching sections is gone**, replaced by two empty host elements the module fills at runtime — anything that reached those elements directly (tests, a future hand-edit) now goes through the module's own build step instead. Verified against the existing popup test suite (moved, not deleted — see `connection-status.test.ts`), which still passes.
- **A caller-ordering dependency inside `popup.ts`**: `connectionStatus.start()` must run before `wire()` registers its own `answerAssistProgress` listener, so tests that grab `onMessage.addListener`'s first registered call keep finding the status listener. Documented at the call site; not enforceable by the module itself.

## References

- The shared component: `apps/extension/src/connection-status/connection-status.ts` (+ co-located `connection-status.test.ts`)
- Callers: `apps/extension/src/popup/popup.ts`, `apps/extension/src/sidepanel/sidepanel.ts`
- The push this reuses: `broadcastStatus()`/`computeStatus()` (`apps/extension/src/background.ts`)
- Prior decisions: [ADR-044](adr-044-extension-answer-tools-side-panel-and-popup.md) (the panel itself, the shared-stylesheet decision), [ADR-045](adr-045-job-tools-panel-parity-and-trust-gate.md) (the extraction pattern this record reuses)
