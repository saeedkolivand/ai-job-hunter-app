# Next Issues — to plan after the audit-triage branch lands

Captured 2026-07-08. Work these in **plan mode** once `fix/audit-triage-p1` is
through review + PR. Each needs investigation before a fix.

## 1. Company web-search in tailoring — default ON when the model supports it

Keep the "search company" option (web-grounded company research used when tailoring
a résumé / cover letter) **enabled by default IF the selected AI model supports search
/ grounding**. Today it appears to default off (or unconditionally). Gate the default on
a provider/model _capability_ (search/grounding support), not a hardcode — respect the
"new model/provider works with zero changes" preference. Likely area: ai-provider model
capabilities + the tailoring/cover-letter flow's default for the research toggle.

## 2. Email subject not copyable

An email subject field somewhere in the app cannot be copied (no copy affordance / not
selectable). Find the email-subject UI (likely an outreach / referral / application email
surface) and make it copyable (a copy button or selectable text), consistent with other
copy affordances in the app.

## 3. [CLOSED] Indeed import from the Applications page → wrong status + missing docs

**Resolution (PR #630, 2026-07-11):** The extension import flow now correctly performs full
dedup-merge into pre-existing Applications by normalized URL, surfacing matched Application
metadata (title, company, status, appliedAt) via the new `applied.check` bridge verb. When an
already-saved job is imported again, the popup now shows its current status instead of
reporting "not found", and the Applications page honors the matched Application's existing
status without creating a duplicate. The dedup-merge logic in `handle_import` + the
`applied.check` read-only bridge verb together close this issue. Existing generation docs are
now discoverable for matched Applications via the standard Applications page UI.

## 4. CLI providers gemini / antigravity / codex don't work

Tested the gemini CLI, antigravity CLI, and codex CLI as AI providers — none worked. The app
supports CLI-based providers (e.g. Claude Code CLI). Investigate the CLI provider adapter:
detection, invocation, arg/format differences per CLI, and error surfacing. Determine per-CLI
what fails (not found? wrong invocation? unsupported output mode?) and either fix the adapters
or clearly report which are supported.

## 5. [CLOSED] Flip `agent_run` off a request-supplied `base_url`

**Resolution (`fix/agent-run-backend-provider`):** `agent_run` (`commands/agent.rs` →
`run_agent_live`) now resolves via `Completer::from_active`, same as every other generation
command. `ToolContext` (`agent/tools.rs`) carries only `job_id`; `provider`/`model`/`base_url`
were dropped from `AgentRunRequest` in both the Rust struct and the Zod schema. The
request-driven `Completer::resolve` constructor was deleted (`pipeline/mod.rs`), leaving
`from_active` as the only egress-binding path. tauri-security-reviewer + rust-backend-architect
approved with no HIGH/CRITICAL. See [ADR-0012](adr/0012-ai-provider-base-url-provenance.md)
— its closure claim now covers all generation paths.

## 6. [LOW] Drop the dead autopilot `assistant_provider/model/base_url` fields

Task #16 moved autopilot's assistant-notes provider resolution onto the backend
`AiConfigStore` (`Completer::from_active`); the renderer wizard now always sends
`assistantProvider`/`assistantModel`/`assistantBaseUrl` as `undefined` (left vestigial
per the "leave the rest intact" call at the time). Remove the now-dead fields from the
renderer `WizardState`/schema (`apps/desktop/src/renderer/features/autopilot/types.ts`,
`lib/schema.ts`, `lib/wizard-state.ts`) and the corresponding struct/deserialize/update
fields on the Rust `Autopilot` record, once nothing reads them.

## 7. [LOW] Write-path mutate-arg assertions for the new AI-provider setter hooks

`apps/desktop/src/renderer/services/use-ai-provider/use-ai-provider.test.ts` (task #16)
only has one generic smoke test (`exerciseServiceHooks` — renders every exported hook
without crashing). None of `useSetActiveProvider`/`useSetProviderSettings`/
`useConfigureActiveProvider` has a test asserting the exact mutate-argument shape sent
to `tauri-client` (provider/model/baseUrl) or that `keys.ai.activeConfig` is invalidated
on success. Pre-existing test gap — same class of assertion the extension-bridge boolean
mutation test (`use-extension-bridge.test.ts`) already has for its own setter.

## 8. [CLOSED] REQ-16240 "keep /resumes" clause — superseded

**Resolution:** annotated in the `AUDIT_REPORT.md` requirements ledger — REQ-16240's
route-path clause is superseded_by REQ-09023; `/documents` is the canonical route (zero
`/resumes` references remain), no redirect needed.

---

# Follow-ups from the 2026-07-27/28 bug batch (#883–#895)

Captured by `project-steward` while closing the batch. Each is a deferred advisory, not a
regression — nothing below blocks a release.

## 9. [BLOCKING NEXT EDIT] Split `extension_bridge/mod.rs` — at the R8 LOC cap

`apps/desktop/src-tauri/src/extension_bridge/mod.rs` is 1398 lines against the 1400-line
hard cap in `apps/desktop/src-tauri/tests/architecture.rs` (rule R8) — two lines of
headroom. PR #895 already had to lift `msg.rs` and `revoke.rs` out to fit. Any edit that
adds more than a line or two fails `cargo test --test architecture`, so do the split
first: extract a cohesive block verbatim into a sibling file under the same module and
re-export the public names — never trim the feature to fit the counter.

## 10. Design-system primitives — light-scheme contrast sweep

Run the contrast audit over `@ajh/ui` primitives in the light color scheme, not just dark.
Related concrete instance: the company-avatar tint needs a `bg-brand/15`-class bump to
clear AA on light backgrounds.

## 11. ModalShell focus-return hardening

`packages/ui/src/components/Drawer/Drawer.tsx` captures the pre-open `activeElement` and
restores it on close (with a `returnFocusTo` fallback, and a liveness check that refuses
`document.body`). `ModalShell` has no equivalent. Port the same treatment, and add a
return-focus test that renders the call site's real initial-focus mechanism.

## 12. `Tag.closeLabel` defaults to hardcoded English

`packages/ui/src/components/Tag/Tag.tsx` defaults `closeLabel` to `'Close'`, which reaches
the `aria-label` verbatim when a caller forgets to pass a translated string. Either make
the prop required, or make an omitted label a lint/type error — `@ajh/ui` must not ship an
untranslated a11y string.

## 13. Missing cache invalidation + error handling in two `aiGenerations.save` callers

`AiGenerationSaveResult` is now a discriminated union (`{ id; success: true } | { error }`),
which exposed call sites that treat every settled promise as a success. Two remain:
`apps/desktop/src/renderer/hooks/use-interview-questions.ts` and
`apps/desktop/src/renderer/features/documents/components/TailorFlow/useApplicationAnswers.ts`
call `api.aiGenerations.save(...)` directly instead of going through
`services/use-ai-generations`, so they neither surface the in-band `error` nor invalidate
the applications cache.

## 14. Email draft-delete path

`AiGenerationStore::update_texts` already models "leave untouched vs. overwrite" as
`Option<String>` per field, which is what lets a user blank out saved text. The email
columns (`email_subject`/`email_body`) have no equivalent path, so a user cannot clear a
persisted draft. Extend the same `Option` pattern to the email fields.

## 15. Key the email-generation save on `applicationId`

`ApplyByEmailTab` bails out entirely when `application.jobUrl` is empty, because
`ai_generations_save` keys the merge-upsert on the normalized job url and `''` means "no
match" — each save would mint a fresh row. Not persisting is the correct interim behavior;
the real fix is letting the save key on the Application id.

## 16. RAII guard for the scrape cancellation-token registry

`apps/desktop/src-tauri/src/commands/scrape.rs` calls `engine.register_token(...)` and then
`engine.unregister_token(...)` manually at every exit path — one missed early return leaks
a slot. A `Drop`-based guard is the fix, but `unregister_token` is `async`; the guard needs
a synchronous lock (parking_lot) on the registry first. Prerequisite, then guard.

## 17. Extension bounded-retry "re-pair?" hint

`token.revoked` only reaches browsers connected at the moment of rotation. A browser closed
at rotation reconnects with the dead token, gets the deliberately silent close, and retries
forever. Add a client-side bounded-retry counter in `apps/extension/src/lib/bridge.ts` that
degrades to a "re-pair?" hint instead of an unbounded backoff loop. See
`docs/knowledge/extension-domain.md` § Revocation reach.

## 18. `PipelineStrip` row density at narrow widths

The strip's rows crowd at narrow container widths; `@container` queries are the intended
mitigation rather than a viewport breakpoint. (Surface lands with the applications-page
redesign.)

## 19. `GenerationCard` empty badges on fresh inserts

A freshly-inserted generation renders its badge row before the derived metadata exists, so
empty badges flash. Either derive the badges synchronously or reserve the row.

---

# Follow-ups from PRs #896–#897 (2026-07-28)

## 20. Metered scraping has no daily backstop

AI providers carry a coarse per-provider, per-UTC-day runaway-cost ceiling
(`PROVIDER_DAILY_MAX` in `apps/desktop/src-tauri/src/limits/mod.rs`). Scraping has only a
rolling-window rate limit (`SCRAPE_RATE_MAX` per `RATE_WINDOW`) plus a concurrency cap — no
daily equivalent — yet Adzuna/JSearch/Apify all cost real money or quota. The window ceiling
was raised twice during the #896 review as paging multiplied the per-search call count, which
is the signal that the window is the wrong shape for the risk: it bounds burst rate, not the
day's spend. Add a per-provider daily charge on the metered aggregator providers, mirroring
`Limiter::charge_provider_daily`.

## 21. Byte-aware client guard for the status-note cap

`MAX_STATUS_NOTE_BYTES` (`apps/desktop/src-tauri/src/commands/applications.rs`) bounds a
status note in BYTES — consistent with every other server cap in that module — while the
renderer's `StatusNoteModal` `maxLength` counts CHARS. For multi-byte text (German, any
non-ASCII), a note the textarea accepts can exceed the server cap, so the user gets a
Validation error at save time instead of being stopped while typing. Correct and recoverable,
but the client should measure bytes (e.g. `TextEncoder`) so the two agree.

## 22. No UI signal that scheduled runs skip the paid LinkedIn tier

PR #896 deliberately stopped scheduled/autopilot runs from buying Apify LinkedIn results:
`apify_cap()` returns `0` when a search carries no upstream spend target, and only the manual
search path sets one. The Apify opt-in toggle in Settings still reads as simply "on", so a
user who enabled and paid for it has no way to learn their scheduled runs never use it.
Surface it where the toggle lives (and/or in the autopilot run step log). See
`docs/knowledge/scraping-domain.md` § Aggregator page loop & spend budget.

---

# Accessibility — open items after PR #924 (`/accessibility` conformance statement)

Measured 2026-08-01 via axe-core 4.12 (WCAG 2.0/2.1/2.2 A+AA tags) plus a scripted keyboard
tab-through. **The violations the audit originally found were fixed in the same PR** — landing
site is 0 violations across all 9 pages, desktop home view is 0 in both colour schemes. What
remains below is the unaudited surface area, not known failures. None blocks a release.

## Desktop renderer — the `text-foreground/NN` opacity ramp (largest open item)

The renderer has ~1,100 `text-foreground/NN` usages; roughly 460 sit at steps (`/30`, `/35`,
`/40`, `/45`) that measure **below 4.5:1** for small text. PR #924 fixed only the 11
nodes that the home-view scan actually measured, swapping them to the existing
`text-muted-foreground` token (5.1:1 on white, 4.8:1 on `#f8f8f8`, and clear in dark too).
(`/55` and `/60` no longer belong on this list — both are now centrally remapped in
`packages/ui/src/css/utilities.css`, pinned by `light-scheme-contrast.test.ts`.
Residual ordering wrinkles from that remap: raw `/65` (11 call sites) and raw `/70`
(143 call sites — the most-used step in the ramp) both render *less* opaque than the
newly-mapped `/60` at 72%. Both still clear AA on their own, so this is cosmetic
token-ordering, not an accessibility failure — fold them into this sweep when it runs.)

Every other view is unscanned and very likely carries the same defect. The fix is mechanical —
swap sub-AA steps to `text-muted-foreground` — but it is a wide visual diff that needs its own
PR and a Lost Pixel review. Publicly disclosed on `/accessibility`, so this is a commitment.

## Desktop renderer — remaining `best-practice` rules (home view)

Outside the WCAG A/AA level the statement targets, but known: `landmark-unique` (1 node — the
two `<nav>` elements in `Sidebar` need distinct `aria-label`s) and `region` (5 nodes — some
Titlebar/StatusBar content sits outside a landmark). Fixing means restructuring landmarks.

## No permanent a11y gate for `apps/landing`

Every scan in PR #924 ran from throwaway scripts under `apps/desktop/` (where
`@axe-core/playwright` and `@playwright/test` already resolve). `apps/landing` has no Playwright
dependency, so nothing re-checks these pages on future changes — the fixes can silently
regress. Adding a gate is a self-contained follow-up.

## Browser extension

- **Never scanned.** The extension injects into third-party pages; no accessibility audit has
  been run and `/accessibility` states it as _not assessed_. Deferred pending a CI mechanism —
  the extension is not currently in an automated browser test suite.

## Exported PDFs

- **Not fully PDF/UA-1 tagged.** The Typst engine bakes document metadata (title, author,
  language) but does not emit tagged/accessible PDF structure. Stated future goal in
  `.claude/skills/resume-export-standards`; currently export carries document metadata only.

## Coverage limits worth remembering

- Automated tooling catches roughly a third of WCAG issues. There has been **no third-party
  audit** and **no testing with assistive-technology users** — both disclosed on the page.
- Single-state scans miss state-dependent failures. `/world`'s accent cycles per section; axe
  only ever measured the red state (3.93:1) while the **blue** state was 1.88:1 — a worse
  failure found by reading the code, not by scanning. Assume other cyclic/stateful UI hides
  similar cases.
