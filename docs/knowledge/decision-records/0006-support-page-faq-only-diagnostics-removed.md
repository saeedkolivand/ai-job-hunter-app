---
status: accepted
---

# Support is FAQ-only; the diagnostics/health/recovery dashboard is removed

## Context

The `support` feature directory contains 23 components. Only `SupportPage` is reachable — it
renders a FAQ (an accordion of question/answer sections from `support-data`) and imports none
of the others. The remaining 22 form a complete but **unrendered** diagnostics/health/recovery/
knowledge-base dashboard: `SystemHealthDashboard`, `AIRuntimeDiagnostics`, `LogsDiagnostics`,
`ScrapingDiagnostics`, `DocumentTroubleshooting`, `KnowledgeBase`, `RecoveryTools`,
`ContactFeedback`, and ~14 cards.

A full-history audit (2026-07) found this subtree has been unreachable since the initial
release — never wired to a route. Its backend was never built: of the ~13 Rust commands the
panels invoke through the `support` client namespace, only `export_diagnostics` and a
`get_system_info` stub are registered. `RecoveryAction` awaits a void-wrapped mutation, so it
would report **false "success" for destructive resets that never run**. The subtree also owns
~45 orphaned i18n keys that break the `i18n:extract` drift check.

Completing the dashboard would mean building a whole diagnostics/health/recovery backend —
including destructive reset operations that require a security review — and fixing the
false-success handler. That is a major, security-sensitive feature, not a reconnection.

## Decision

**The Support page is intentionally FAQ-only.** The diagnostics/health/recovery/knowledge-base
dashboard is an abandoned direction and is **removed**: all 22 unrendered components, the
diagnostic parts of `support-data`, the unregistered `support`/recovery command contracts and
client-namespace methods, and the ~45 orphaned i18n keys they own.

**One capability is salvaged:** the export-diagnostics "submit a bug report" action
(REQ-14020). Its backend command `export_diagnostics` is already registered and the flow was
reviewed and translated. It is **moved to a reachable location in Settings** (a single action),
rather than deleted with the rest.

If a diagnostics/health surface is wanted in the future, it is built deliberately against real
need — not by resurrecting this speculative, backendless subtree.

## Considered options

1. **Delete the dashboard, keep FAQ, salvage export-diagnostics into Settings (chosen).**
   Removes a large speculative dead surface and its i18n drift, keeps the one piece with a real
   backend and a clear use (bug reports). Cost: loses the invested dashboard UI (recoverable
   from git if ever wanted).
2. **Complete the dashboard.** Wire it to a route, register ~13 commands (incl. destructive
   resets), fix the false-success. Rejected for now: a major, security-sensitive build for a
   surface with no users and no backend; revisit only if self-service diagnostics becomes a
   real product goal (own ADR + security review).
3. **Leave it as unreachable dead code.** Rejected: it fails the audit's coherence bar, carries
   a latent false-success-on-destructive-reset hazard, and jams the i18n drift check.

## Consequences

- **~22 components + their contracts + ~45 i18n keys are deleted.** This also resolves audit
  findings `renderer-feat-3-001`, `p2-b1-ipc-chain-001/002`, and a large share of the missing/
  orphaned-i18n-key findings in one move.
- **`export_diagnostics` gains a real, reachable home in Settings** — the bug-report path
  becomes usable for the first time.
- **The `support` client namespace shrinks** to what SupportPage + the salvaged action need;
  run `gen:ipc:check` after removing contracts.
- **Recoverable:** the deleted dashboard lives in git history if a future diagnostics feature
  wants to reference it.

## References

- Reachable page: `apps/desktop/src/renderer/features/support/components/SupportPage/index.tsx`
  (FAQ via `support-data.ts` `getSupportSections`).
- Orphaned subtree: the other 22 components under `apps/desktop/src/renderer/features/support/components/`.
- Backend: `export_diagnostics` + `get_system_info` registration in `apps/desktop/src-tauri/src/lib.rs`; `support` client namespace in `apps/desktop/src/tauri-client/namespaces/support/`.
- Audit findings: `renderer-feat-3-001`, `p2-b1-ipc-chain-001`, `p2-b1-ipc-chain-002` (AUDIT_REPORT.md §4).
