---
status: accepted
---

# Assisted autofill + answers capture — user-initiated, transparent, no-persistence

## Context

Every major 2026 job-hunt competitor (Simplify, Teal, LazyApply) ships browser form-fill, and the free tier of the market leader does it well. AI Job Hunter already holds the user's contact details in the authoritative **Contact Profile** and already has a paired, authenticated desktop⇄extension bridge (loopback WebSocket, per-frame pairing token) built for **Extension import** (reading a job _into_ the app). The symmetric capability — writing the user's own contact details _out_ onto an employer's application form — was the single most-requested "action" feature the app lacked, and the one place a competitor's free tier beat us on capability rather than on privacy or cost.

The hard question is not whether to build it but how to build it without breaking the two things that are actually our moat: the **local-first privacy boundary** ([ADR 0005](0005-network-egress-privacy-boundary.md)) and the **human-in-the-loop, never-auto-apply** brand posture. Autofill moves PII (name, email, phone, socials) onto a third-party page, which is exactly the kind of egress ADR 0005 governs, and it sits one small step away from the auto-apply line we have deliberately refused to cross.

The design was stress-tested in a grill-with-docs session against the bridge protocol, the pairing/token auth model, Chrome Web Store + Firefox AMO policy, the Contact Profile authoritative model, path-privacy/PII rules, and the app's no-silent-behavior honesty posture. Six decisions were resolved and accepted; this ADR records them.

## Decision

Ship **assisted autofill + answers capture**: a user-initiated, click-to-fill action in the published MV3 extension that (1) fills empty form fields on the current page from the Contact Profile, reviews-and-submits by the human, and (2) captures any application-form text answers the user entered, replays them as suggestions on future applications, and never persists PII in the browser.

1. **User-initiated, per-invocation, no broad host access.** Fill runs only when the user clicks "Fill this form", via `activeTab` + `chrome.scripting.executeScript` on the current tab. There are **no broad `host_permissions`** — the extension gains no standing access to any site; each fill is a one-shot, user-gestured injection. This is what lets it work on _any_ site the user is on while keeping the AMO `data_collection_permissions: ["none"]` posture and a minimal Chrome permission set.

2. **Generic field matcher, not a per-ATS scraper.** Fields are identified by a tiered heuristic — `autocomplete` attribute → label/`aria-label` text → `name`/`id` → `placeholder` — against a fixed key set (`fullName`, `email`, `phone`, `location`, `linkedin`, `github`, `website`). Only **empty** fields are filled; a field whose key is **ambiguous** (see the denylist) is skipped, never guessed. This generalizes across boards instead of coupling us to Workday/Greenhouse/Lever DOM shapes that churn.

3. **Never auto-submit.** The extension fills; the human reviews and clicks Submit. There is no code path that submits a form. This keeps autofill on the safe side of the auto-apply line the product does not cross.

4. **PII travels over the existing authenticated bridge, fetch-fresh.** The content script requests the profile from the desktop app over the existing loopback WebSocket using one new message pair (`profile.get` → `profile.result`), authenticated by the mutual HMAC handshake established per connection (see [ADR 0010](0010-bridge-hmac-handshake.md)). The profile is **fetched fresh at fill time and never written to `chrome.storage`** — nothing PII-bearing persists in the browser, so an extension compromise or an uninstall leaks nothing at rest.

5. **Opt-in, default OFF, enforced desktop-side.** Autofill is gated by a desktop setting (default OFF, reset-to-OFF on data reset). The **desktop refuses `profile.get` when the toggle is off** — the gate lives on the data owner, not in the extension, so disabling it actually stops PII from leaving the device (the ADR 0005 rule for egress carrying user data). With the toggle off, the profile never crosses the bridge. Answers capture (reading form inputs back into the app) operates within the same opt-in gate; the toggle is in Settings under a broadened label reflecting both autofill (fill empty fields only in _bulk_ operations) and answers capture scopes.

6. **Transparent about what it did and honest about limits.** After a fill the extension shows an in-page summary of which fields it set. The disclosed limit: **complex custom ATS** (Workday shadow DOM, multi-step wizards) fill **partially at best**. _(Amended 2026-09-15 — this decision originally also disclosed that a résumé FILE "cannot be uploaded" because "browsers forbid programmatic file-input population"; that claim was wrong. See the amendment below.)_ These are documented in the extension README and the privacy page, not papered over.

7. **Amendment (Task #22, auto-track Layer A) — a distinct sanctioned "auto-action" consent class.** Where decisions 1-6 above cover a **user-clicked** fill/capture, auto-track adds the first **auto-act on a detected event** surface: after the user invokes the extension on an application page (the same per-invocation gesture from decision 1), the injected `submit-watch` observes — capture-phase, never `preventDefault`/`stopPropagation` — for a real form submit or an apply-style click, and posts the URL back once. On a detected submit the background re-checks a new **auto-track opt-in (default OFF, desktop-enforced)** and, for a tracked `saved` job, auto-issues `status.update { to: 'applied', auto: true }`; already-`applied` is a silent no-op, and an untracked submit only nudges (extension action badge) toward the existing import flow — it **never auto-creates**. This is sanctioned as its own consent class, not folded into the decision-5 autofill opt-in, because the trigger is a detected **event** (a submit) rather than a direct click on the feature itself.
   - **The enforced boundary is server-side, not client-side.** `handle_status_update` (`apps/desktop/src-tauri/src/extension_bridge/status_update.rs`) refuses any `auto:true` write unless the desktop's own `BridgeState::autotrack_enabled()` is true — checked against desktop-owned state (the opt-in flag + the pre-existing `saved → applied` transition allowlist + the exact-url compare-and-set), never against anything the extension asserts about itself. The extension-side arming gate (`autotrack.check` before injecting `submit-watch`) and the `auto` flag on the wire are **defense-in-depth only**. The security reviewer's conclusion driving this split: for any extension→desktop auto-action, a client-attested gesture or opt-in is forgeable (a compromised or patched extension can claim anything), so the write MUST be authorized against state the desktop itself owns.
   - **Honest residual risk (disclosed, not hidden):** with the opt-in ON, a page can flip its OWN already-tracked `saved` job to `applied` purely from a post-gesture submit/apply-click it observes — there is no way to prove intent-behind-the-click beyond the existing gesture-then-detect design. This is inherent to observe-a-submit (not a bug), bounded (only the one job the user already gestured on, only `saved → applied`, never a new Application), and reversible (the stage picker can move it back), with **no egress** — the whole exchange stays on the loopback/native-messaging bridge. Layer C (#23, local email-confirmation parsing) is the planned complement that corroborates from a second, independent signal.
   - New wire surface: `autotrack.check` / `autotrack.result` (`{ enabled }`, read-only, no gate) and the optional `auto` flag on `status.update` (see decisions above's reference file for the schema).

## Amendment (2026-09-11, ADR-050) — consent may also be granted/withdrawn from the extension

Decision 5's "opt-in, default OFF, enforced desktop-side" still holds at USE time: the desktop
refuses `profile.get`/`agent.query`/`agent.call` whenever its own stored flag is off, and that
refusal is never based on anything the extension asserts about itself. What changes: the STORED
flag can now also be flipped from the paired extension's own Settings page, through a dedicated
`settings.get`/`settings.set` verb pair scoped to exactly the extension's own opt-in switches
(`SettingsKey` in `extension_bridge/settings.rs`; never the
generic tier), applying through the same setters the desktop Settings UI uses. Every successful
change raises a Notification Center entry so a flip made from the extension is never silent to the
person at the desktop — see [ADR-050](adr-050-extension-read-tier-and-settings-verbs.md) for the
concern that was raised and how it was resolved.

## Amendment (2026-09-15, PR2 — documents into ATS) — résumé file attach corrects the "cannot be uploaded" disclosure

Decision 6's original claim — a résumé FILE "cannot be uploaded" from a content script because
"browsers forbid programmatic file-input population" — was wrong: assigning a `File` through a
`DataTransfer` to a `type=file` input (`input.files = dt.files`) works. The honest limit is
narrower than what was disclosed: a custom drag-and-drop upload widget built on its own
`drop`/`dragover` handlers, rather than the native `change` event, may ignore the assignment — so
the extension **verifies** by re-reading the field after assigning it and **reports** whether it
actually took, rather than assuming success
(`apps/extension/src/lib/attach-file.ts`, injected via `apps/extension/src/attach-file.ts`). The
file is rendered/exported on the user's own device — through the same `document.export` →
`document.result` bridge verb pair the panel's Documents tab drives (see
[ADR-050](adr-050-extension-read-tier-and-settings-verbs.md)) — and handed only to the page the
user is currently on. Nothing changes about the consent boundary: this still rides decision 5's
Autofill opt-in. Cover-letter text is pasted the same way the existing answer-replace flow works:
into a textarea the user picks, never auto-selected; a Copy action is offered alongside it.

## Amendment (2026-09-15, PR3 — Check-fit on the page) — a read-only badge and read-only results-page stamps join the page-touching surface

Two new page-touching renders join the ones decisions 1-6 already cover, following the same shape:
user-initiated (an explicit Check-fit gesture for the badge, a dedicated "stamp this results page"
gesture for the stamps), opt-in and **default OFF**
(`getShowFitBadge`/`getStampResultsPages`, `apps/extension/src/lib/appearance.ts`), and never a
form action — neither can submit anything, and neither offers a control beyond its own dismiss and
(badge only) "Open the panel".

- **On-page fit badge** (`apps/extension/src/lib/fit-badge.ts`): a fixed pill showing the Check-fit
  score and a saved/applied chip, expanding on click to a mini card (missing keywords, the salary
  facts line when present, one "Open the panel" action).
- **Results-page stamps** (`apps/extension/src/lib/results-stamp.ts`): a small saved/applied marker
  placed next to each matched job-card link on a results-listing page, resolved in one round trip
  through the new `applied.check.batch` verb
  (`apps/desktop/src-tauri/src/extension_bridge/applied_check_batch.rs`) — the batch form of the
  existing ungated, device-local `applied.check` lookup, with its own trust class and throttle, and
  never a score.

**Both render inside a `mode: 'closed'` shadow root** on the node they attach to `document` — so the
page's own scripts (an ad, a tracker, a compromised board) cannot read the score, the missing
keywords, or the salary text back off the shared DOM even though the host element itself lives in
`doc.body`.

**Salary is two facts, never a verdict** — matching design decision 5
(`.claude/scratch/extension-round-design.md`): when `match.live`'s reply carries a `salary` object,
the badge's mini card and the panel's own `why?` details show "Posting says …" (a verbatim RANGE
substring extracted from the posting text by
`apps/desktop/src-tauri/src/extension_bridge/salary_facts.rs`, never inferred, never a single number
treated as a range) and, only when the user has one stored, "You want …"
(`JobPreferences.salary_expectation`, shown verbatim) — side by side, with no comparison or computed
verdict between them.

## Considered options

1. **Assisted, generic, transparent, no-persistence, opt-in (chosen).** Matches the market's most-used capability while preserving both the privacy boundary and the human-in-the-loop brand. Cost: partial fills on complex ATS — accepted and disclosed. (File upload was believed impossible when this option was weighed; the 2026-09-15 amendment below corrects that.)
2. **Per-ATS deep integrations (Workday/Greenhouse/Lever adapters).** Rejected: higher fill quality on a handful of boards, but couples us to churning private DOM, multiplies maintenance, and still can't upload a file. The generic matcher degrades gracefully everywhere instead of excelling in four places and breaking silently elsewhere.
3. **Persist the profile in `chrome.storage` for offline/instant fill.** Rejected: puts PII at rest in the browser, widening the blast radius of an extension compromise and contradicting ADR 0005. Fetch-fresh over the authenticated bridge costs one round-trip and leaks nothing at rest.
4. **Broad `host_permissions` so fill is always available without a click.** Rejected: standing access to all sites is a heavier store-review and privacy posture than the feature needs, and breaks the `["none"]` AMO data-collection stance. `activeTab` on a user gesture gives the same reach with none of the standing access.
5. **Auto-submit after fill (one-click apply).** Rejected: crosses the auto-apply line the product refuses to cross; the human-in-the-loop review is the brand and legal moat, not a limitation to optimize away.

## Consequences

- **Two new bridge message pairs enter the protocol**: `profile.get`/`profile.result` (read contact), and `answers.save`/`answers.suggest` (write/read application answers). Both must stay in TS↔Rust lockstep like every other bridge message; the profile projection is flat (seven contact keys only), never the full Contact Profile record; answers are deduplicated by question text and stored per-Application (never globally).
- **The pairing token's blast radius grows.** A harvested token could previously import jobs; with autofill enabled it can also **read the Contact Profile** via `profile.get` and **read/write stored answers** via `answers.*`. This is disclosed in the extension threat-model note and README, and bounded by the opt-in gate (token reads/writes nothing when autofill is OFF).
- **The desktop toggle is the enforcement point**, not extension UI — reviewers and future authors must keep the refusal on the desktop handler; moving the gate into the extension would silently break the guarantee.
- **Autofill + answers capture is now the sanctioned "read/write user data" template**, the mirror of Extension import's "read job in": user-gestured, authenticated, fetch-fresh, opt-in, never-submit. Future outbound-to-page features follow this shape.
- **The honest-limits disclosure (partial complex-ATS, no rewrite-mode replace — file upload is now supported, see the 2026-09-15 amendment)** is a documentation obligation, not optional polish — hiding it would violate the no-silent-behavior posture. Rewrite-mode (one-click replace of a field value) is deferred to a follow-up; the current shipped version captures and suggests answers but never programmatically replaces filled text.
- **Per-Application answer storage** decouples suggestions across jobs — a "Why this role?" answer saved for Company A does not pollute the suggestion pool for Company B, respecting the human's intent to tailor each application.
- **(Task #22) Auto-action consent gates live desktop-side, not extension-side, by rule now, not just by convention** — the same enforcement-point discipline as decision 5's `profile.get` refusal, extended to a WRITE triggered by a detected event rather than a direct click. Any future "observe X, auto-act Y" surface must follow the same shape: a client-side arming/opt-in check is defense-in-depth only; the desktop must independently re-verify its own opt-in state before honoring the write.
- **`status.update`'s `auto` flag is additive/optional** — an old extension omits it and gets the pre-existing ungated deliberate-click behavior unchanged; only an explicit `auto:true` is subject to the new opt-in gate.

## References

- Protocol: `packages/shared/src/ipc/extension-protocol.ts` + `extension-protocol-constants.ts` (`profile.get`/`profile.result`, `answers.save`/`answers.suggest`, `ExtensionProfileResult`, `ExtensionAnswersSaveRequest`, `ExtensionAnswersSuggestResult`).
- Desktop handlers + gate: `apps/desktop/src-tauri/src/extension_bridge/mod.rs` (`handle_profile`, `resolve_profile`, `autofill_enabled`); `answers_save.rs` (dedup, merge, storage); `answers_suggest.rs` (Jaccard-based replay).
- Matcher + fill: `apps/extension/src/lib/autofill.ts` (tiered matcher, ambiguous denylist, `isHidden`), `apps/extension/src/fill.ts` (import-free injected script), `apps/extension/src/background.ts`.
- Opt-in setting: `apps/desktop/src/renderer/features/settings/components/accounts/ExtensionBridgeSection`.
- Disclosure: `apps/extension/README.md`, `landing/privacy.html`.
- Related: [ADR 0005](0005-network-egress-privacy-boundary.md) (egress boundary), [ADR 0010](0010-bridge-hmac-handshake.md) (hardened auth), [ADR 0011](0011-extension-ai-assist-optin.md) (separate billable AI assist tier), Extension import + Pairing token in `docs/CONTEXT.md`.
- **Résumé file attach (PR2 amendment):** `extension_bridge/document_export.rs` (`document.export`/`document.result` handler + gate/throttle), `apps/extension/src/lib/attach-file.ts` (`DataTransfer` assignment + re-read verification, fail-closed), `apps/extension/src/documents/` (the panel's Documents tab).
- **Check-fit on the page (PR3 amendment):** `apps/desktop/src-tauri/src/extension_bridge/applied_check_batch.rs` (`applied.check.batch`/`applied.batch.result` handler, its own throttle), `apps/desktop/src-tauri/src/extension_bridge/salary_facts.rs` (the posting-side salary-range extractor), `apps/extension/src/lib/fit-badge.ts` (the on-page badge, closed shadow root), `apps/extension/src/lib/results-stamp.ts` (the results-page collector/stamper, closed shadow root), `apps/extension/src/lib/notebook-palette.ts` (the portable palette an injected script inlines), `apps/extension/src/lib/appearance.ts` (`getShowFitBadge`/`getStampResultsPages`, both default OFF).
- **Auto-track (Task #22 amendment):** protocol — `autotrack.check`/`autotrack.result`, `ExtensionStatusUpdateRequest.auto` (`packages/shared/src/ipc/extension-protocol-constants.ts`); server-side enforcement — `handle_status_update`/`auto_write_refused`/`is_auto_status_update` (`apps/desktop/src-tauri/src/extension_bridge/status_update.rs`); opt-in state — `BridgeState::autotrack_enabled`/`set_autotrack_enabled` (`apps/desktop/src-tauri/src/extension_bridge/mod.rs`) + `extension_bridge_auto_track_enabled`/`extension_bridge_set_auto_track_enabled` (`apps/desktop/src-tauri/src/commands/extension_bridge.rs`); client-side arming/decision — `armSubmitWatch` (`apps/extension/src/lib/submit-watch.ts`), `decideSubmitAction`/`handleSubmitDetected`/`maybeArmSubmitWatch` (`apps/extension/src/lib/auto-track.ts`); background wiring — `apps/extension/src/background.ts`. Deferred complement: Layer C (#23, local email-confirmation parsing).
