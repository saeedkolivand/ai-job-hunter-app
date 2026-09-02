# ADR-042 — Dashboard next-step tile derived from live state, not stored; collapses to one line when setup is complete

**Status:** Accepted

**Date:** 2026-09-02

**Deciders:** owner (decision 2026-09-02, "go next"), main session

## Context

[ADR-041](adr-041-searchable-help-page-over-compiled-in-entries.md) deferred the "what should I do next" half of the discoverability gap as a separate decision about persistence on the Dashboard.

The onboarding wizard and spotlight tour are first-run-only and gated on a single persisted boolean; both the tour's skip and finish actions write the same value. This design means nothing can tell a user who read the tour from one who dismissed it. The owner's demonstration of the app to another person failed precisely because the wizard had been dismissed on that machine, and a one-time tour cannot answer a question that arises three screens later.

The Dashboard today renders a page header, four data-free quick-action tiles, and two cards with no getting-started surface. Eight orphaned `dashboard.*` translation keys show a richer dashboard was designed and never built.

## Decision

**1. One derived next-step row on the Dashboard**, placed between the page header and the quick-action grid, reusing the `ActionTile` primitive. Not a fifth quick-action tile — the grid is `@2xl:grid-cols-4` and the stagger class index is typed to the four existing stagger classes.

**2. It derives from live state only**, through existing React Query service hooks: `hasDocument` (embedding status document count, app-global), `aiUsable` (the same predicate used by Analyzer and the AI status card), `hasJob` (any interaction-log row of a tracked type — the same allowlist the pipeline overview uses, so a dismissed suggestion does not count). No preference, no store field, no migration, nothing persisted; re-derived on every mount.

**3. While any signal is still resolving, the row renders nothing**; the AI predicate's "no reason yet" state counts as resolving, and the predicate itself withholds a reason until its provider-key query has settled (it once answered "add an API key" mid-flight, which would have flashed the wrong step at every configured cloud user on every cold boot). If a signal query fails, the row falls back to a neutral line with the help link — it never disappears and never claims setup is complete. This accepts the small layout shift when signals arrive.

**4. Three steps only**: résumé, AI, job. Extension pairing is not a step — the bridge's connected flag is a live socket count, not a pairing record, and reading it would add an app-wide poll. Autopilot is not a step — its list query deliberately has `staleTime: 0` and would refetch on every visit home.

**5. Once every step is met, the row collapses to a single line** with a check icon, a title, and a link to Help & Support. It never disappears and is never dismissible. A surface that disappears is invisible to the user who is set up and still lost — the exact failure the plan set out to fix.

**6. The progress badge counts met steps**, not sequence position, because steps can become unmet again out of order (a user who deletes a document after finishing shows step résumé with "2 of 3 done").

**7. Applications are deliberately not a signal**: the applications list ships every row's job description and summary, and its app-global change listener would re-fetch it while the user sits on the home route, all for one boolean. A job tracked by hand or through the extension with no interaction row therefore does not satisfy the step, which is why the step says "find", never "track": opening any posting satisfies it.

**8. The résumé step uses the document count as a proxy**: documents carry no `kind` field, so a user with only an imported cover letter reads as ready. This is accepted to avoid adding `kind` to the document schema.

**9. `ActionTile` becomes keyboard-reachable**: `role="button"`, `tabIndex={0}`, and `onKeyDown` firing `onClick` on Enter and Space, only when `onClick` is provided. Fixes the same gap for existing quick-action tiles.

## Considered options

1. **A stored dismissal flag** (rejected): introduces a new persisted field with factory-reset and e2e-seed consequences, for a surface cheaper to derive on every mount.

2. **Disappearing on completion** (rejected by the owner): a surface that disappears is invisible to the user who is still lost after setup, which is the failure the plan set out to fix.

3. **A "continue working" mode** (deferred): suggests the next action from application data (requires per-application generation status the Dashboard does not load, and a rule set of its own).

4. **A fifth quick-action tile** (impossible): would require changing the grid definition and its stagger class index typing.

5. **Reviving the orphaned `dashboard.continueWorking` / `dashboard.insights.*` keys** (rejected): they carry fabricated numbers; ADR-041 § 6 forbids copy not verified against the UI.

## Consequences

### Positive

- **Persistent, state-aware pointer** at the next action and at help, visible on every home visit.

- **No persistence cost**: nothing new to migrate or reset; re-derived on mount.

- **No added query** on the home route: every signal dedupes onto a query the root layout or the pipeline overview already mounts.

### Tradeoffs and costs

- **Brief layout shift**: the row appears after signals resolve. Mitigated by rendering nothing while pending.

- **Document-count proxy**: a user with only an imported cover letter reads as having a résumé. Accepted to avoid adding schema fields.

- **Interaction log under-reports**: a viewed row is only written after a dwell in the detail pane, and applications are not consulted (decision 7), so a user who tracked a job by hand still sees the "find a job" step until they open a posting.

- **Extractor CI step is advisory**: translation keys are pinned by a renderer test, as in ADR-041, rather than failing the build.

## References

- Prior decision: [ADR-041](adr-041-searchable-help-page-over-compiled-in-entries.md) — the help page decision; the onboarding and tour context
- Persistence boundary: `docs/knowledge/persistence.md`
- Dashboard component: `apps/desktop/src/renderer/features/dashboard/components/Dashboard/index.tsx`
- NextStepTile component: `apps/desktop/src/renderer/features/dashboard/components/NextStepTile/index.tsx`
- Derivation logic: `apps/desktop/src/renderer/features/dashboard/lib/next-step.ts`
- ActionTile primitive: `packages/ui/src/components/ActionTile/ActionTile.tsx`
