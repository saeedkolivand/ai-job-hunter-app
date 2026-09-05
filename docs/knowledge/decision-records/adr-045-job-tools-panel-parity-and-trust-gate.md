# ADR-045 — Job-tools panel parity: a shared component and a new per-tab trust gate

**Status:** Accepted

**Date:** 2026-09-05

**Deciders:** owner (settled the design in a `grill-with-docs` session), main session (implementation)

## Context

[ADR-044](adr-044-extension-answer-tools-side-panel-and-popup.md) gave the Answer-tools section a side panel alongside the popup, as two views over one per-(tab, origin) state (`AnswerState`, `apps/extension/src/lib/answer-state.ts`) — decision 4 scoped that record around the application question as "the primary object." The popup separately has four page-scoped action buttons (`popup.ts`'s `doImport`/`doCheckFit`/`doFill`/`doSaveAnswers`) that the panel never had at all. This record extends the panel to those four controls too — a distinct decision from ADR-044's, with its own real trade-off (the gesture-re-arm design below), not an amendment to a record that was scoped around a different problem.

The reason this cannot just be "mount the same buttons in both places" is Chrome's `activeTab` grant model: it is issued by a qualifying user gesture (toolbar action click, context-menu click, a keyboard shortcut, an omnibox acceptance) — **never** by clicking a control already rendered inside an open side panel. Opening the popup IS always such a gesture, so the popup's copies of these four controls are implicitly fresh on every render. The panel has no equivalent: it persists across tab switches with no fresh gesture firing on every switch. The background functions these four controls call — `captureActiveTabFieldsProbe`/`runFieldsProbe` (`browser.scripting.executeScript`) and `runAppliedCheck`'s `activeTabUrl()` (`browser.tabs.query`) — both need a _live_ grant to answer correctly, not just to look right; an ungated call on a stale tab would produce wrong or failed results, not merely an ugly one.

## Decision

**1. Reuse `AnswerState.pageChanged` as the trust signal — no new field.** A tab is "trusted" when a record exists for it AND `pageChanged === false`; "no record at all" is treated as equivalent to `pageChanged: true` (untrusted) — see `isPageTrusted` in `apps/extension/src/job-tools/job-tools.ts`. This is a stricter reading than ADR-044 decision 3 gives the Answer-tools rows themselves (which show their own empty state, not a gated one, for a never-scanned tab): the four job-tools controls assert "you can act on this page right now," which a total absence of evidence cannot support, so it defaults closed rather than open.

**2. Every gesture path re-arms the flag, not just a scan.** Before this change, only a successful `runAnswerScan()` cleared `pageChanged` (background.ts's own "the scan itself IS the re-arm" comment). Both context-menu entries are gestures too, so both now force-write `pageChanged: false` for their tab via a new `rearmPageChangedForGesture` helper (background.ts) — no scan, no row mutation, since a bare right-click implies nothing about wanting to re-scan the page's Answer-tools questions. The plain-page/editable entry (`ANSWER_PANEL_MENU_ID`) had never touched `AnswerState` at all before this change; it now also creates the same minimal record `runAnswerAddRow` builds for a tab with no prior state, so a tab whose only interaction was that entry is still marked trusted.

**3. The probes never fire while untrusted, and are never cached across surfaces.** `runFieldsProbe`'s panel-side equivalent inside the new shared component checks the gate first and skips the call entirely when it reads untrusted, rather than firing it and discarding a fail-open result — the surface's whole control area is already replaced by one line in that state, so the result would go unused anyway, and skipping avoids acting on a tab that structurally cannot answer the query correctly. The popup's own `appliedCheck` auto-check is unaffected: it is not part of the shared module (see decision 5) and the popup never needs the gate. Each mounted instance keeps its own generation counter and its own trust flag — nothing is cached in `AnswerState` for this, matching the existing "cheap, single-message call, not worth a shared cache" reasoning the codebase already applies to the popup/panel split.

**4. Untrusted replaces the controls with one line — it does not merely disable them.** Same convention ADR-044 decision 3 established for the Answer-tools write controls: a disabled button still claims the capability exists. The line is a shared constant (`JOB_TOOLS_GATED_LINE`, `job-tools.ts`) covering both "no record" and "stale record" — the two cases give the user the same actionable instruction either way.

**5. A new shared module owns exactly the four controls, not "Mark as applied."** `apps/extension/src/job-tools/job-tools.ts` mirrors `answer-tools.ts`'s shape (`mountJobTools(host, deps): JobToolsView`, same test co-location) and owns Import/Check-fit/Fill/Save-answers verbatim from `popup.ts`, plus its own fields-probe equivalent for the Form group's visibility. "Mark as applied" and the adaptive Import re-label were deliberately **excluded**: they were never part of the requested parity (only the four wire verbs), and the module has no wire call for them. The popup's own (unmoved) `appliedCheck` auto-check still needs to reach the Import button the module now owns for its adaptive label — the module exposes one small setter (`JobToolsView.setImportLabel`) for exactly that, rather than duplicating `appliedCheck` into the panel.

## Alternatives considered

1. **Special-case the gate off for the popup** (e.g. a `surface: 'popup' | 'panel'` flag). Rejected: the gate must be correct in principle for either caller, not hard-coded around "popup never gates." The popup simply never feeds the module a real `AnswerState` (it has no use for the gated line), which achieves the same outcome without a branch in the shared logic.

2. **Move "Mark as applied" and the adaptive Import label into the shared module too**, giving the panel full "Job group" parity rather than just the four requested verbs. Rejected as scope creep beyond "no new capability": these were never named in the parity request, and duplicating `appliedCheck`'s call into a panel instance no caller needs it from would be exactly the kind of ungated-in-a-context-it-wasn't-built-for call this record's Context section warns about.

3. **Cache the fields-probe / trust decision in `AnswerState`** so both surfaces share one result. Rejected: the calls are cheap and single-message; duplicating them per mounted instance is simpler than adding shared-cache fields and their own staleness rules.

## Consequences

### Positive

- **The panel gains real parity** for Import/Check-fit/Fill/Save-answers, using the exact wire verbs the popup already used — no protocol change.
- **The popup's existing behavior is untouched.** Its own tests pass unmodified in substance (adapted only where the DOM the four controls render into moved from static popup.html markup to the shared module's own host); it never shows the gated line in practice, because it never feeds the module state to gate on.
- **A wrong-tab probe call is now structurally prevented** in the panel, not just discouraged — `captureActiveTabFieldsProbe`/`activeTabUrl` are never invoked from a panel instance the gate has marked untrusted.
- **Both context-menu gestures now correctly arm the SAME flag** the toolbar/popup path always armed, closing a gap where a right-click could open the panel onto a tab the panel's OWN gate would then (correctly, but confusingly) call untrusted.

### Tradeoffs

- **A second gate on top of ADR-044's `pageChanged`, read more strictly.** Two callers now derive different UI decisions from the same one boolean (Answer-tools' own empty-vs-gated split, and this module's active-vs-gated split), which is a bit of interpretive weight on one field — accepted because a second field would need its own re-arm discipline for no behavioral gain over deriving it.
- **The popup's Import button moved out of `popup.html`'s static markup** into DOM the shared module builds at runtime; anything that reached it directly (a focus call, a query) now goes through the module's host element instead. Verified against the existing popup test suite, which still passes.
- **One redundant fields-probe call is possible** on the rare tab where the module's optimistic startup default (trusted) is corrected moments later by the real `AnswerState`, before the popup's own connect-transition trigger lands — self-correcting via the same generation guard that already protects a genuine stale-response race, never a wrong result.

## References

- The shared component: `apps/extension/src/job-tools/job-tools.ts` (+ co-located `job-tools.test.ts`)
- Trust gate + re-arm: `isPageTrusted` (`job-tools.ts`), `rearmPageChangedForGesture` (`background.ts`)
- Callers: `apps/extension/src/popup/popup.ts`, `apps/extension/src/sidepanel/sidepanel.ts`
- Shared per-tab state this reuses: `apps/extension/src/lib/answer-state.ts` (`AnswerState.pageChanged`)
- Prior decision: [ADR-044](adr-044-extension-answer-tools-side-panel-and-popup.md) (the panel itself, the shared-state model, the gesture model, the context-menu entries this record's decision 2 extends)
