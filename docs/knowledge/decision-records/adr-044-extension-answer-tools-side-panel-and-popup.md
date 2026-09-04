# ADR-044 — Extension Answer tools in a side panel and the popup: two views of one per-tab, per-origin state, page access still user-gestured

**Status:** Accepted

**Date:** 2026-09-03

**Deciders:** owner (kept both surfaces, chose the single delivery PR), main session (recommended the draft-time character limit, which the owner accepted)

## Context

The extension's Answer tools — draft an answer to an application question, then iterate on it — live in the action popup. The popup closes on blur, and blur is exactly what happens when the user clicks into the page to paste the answer they just drafted: the tool disappears at the moment it is used. That is the problem this record answers.

It answers it without reopening anything the doctrine already settled:

- [ADR-0009](0009-assisted-autofill.md) fixed the page-access shape — user-gestured, `activeTab` only, no broad host permissions, never auto-submit, consent gates enforced desktop-side.
- [ADR-0011](0011-extension-ai-assist-optin.md) made AI drafting its own billable opt-in, separate from the free autofill/capture gate. The provider is resolved desktop-side and never disclosed to the extension (`apps/desktop/src-tauri/src/extension_bridge/answer_assist.rs`, Provider resolution).
- [ADR-015](adr-015-extension-bridge-websocket-save-origin.md) fixed the transport, and `docs/knowledge/extension-domain.md` the reserved-verb pattern, the TS↔Rust lockstep rule and the fixed-sentinel wire-error discipline.
- `apps/extension/src/manifest.test.ts` pins the permission surface: a hand-written allowlist, a denylist that survives a careless allowlist edit, loopback host access checked as a property, no content scripts, both browser targets compared to each other, and the extension README's published justification table checked against that allowlist.

So the open questions were the _surface_, the _state behind it_, and what the UI is allowed to claim. The design was grilled against those records first; the decisions below are the outcome.

## Decision

**1. Two surfaces, one state.** The Answer tools gain a persistent side panel — `chrome.sidePanel` on Chrome, `sidebar_action` on Firefox — and the action popup **keeps** the full Answer tools section. Neither is the home; they are two views over ONE state, keyed by tab **and** origin, kept in session-scoped extension storage owned by the background worker, so it survives the MV3 worker idling out and dies with the browser session. Question rows, versions and the in-flight stream all live there, so a draft started in the popup is already on screen in the panel, survives the popup closing, and neither view owns anything the other cannot see. The origin half of the key is captured from the gesture-granted tab at gesture time and never through a `tabs`-permission lookup, so the denylist entry that forbids `tabs` (decision 7) holds by construction rather than by discipline. The Chrome panel is per window, so it follows tab activation and renders the active tab's state.

**2. Page access stays user-gestured, and the gesture model is the same on both browsers.** ADR-0009's shape is untouched: no `host_permissions` widening, no content scripts, no auto-submit. The toolbar click grants `activeTab` for that tab and opens the popup: decision 1 keeps the popup, a declared `default_popup` takes priority over any open-the-panel-on-action-click behaviour, so the action click cannot be the thing that opens the panel on either browser. The panel is opened from the popup's own control, and a click in the popup is itself a user gesture: on Chrome `chrome.sidePanel.open({ tabId })` called directly in that click handler, with the panel's options set beforehand because a set-options-then-open sequence loses the gesture; on Firefox `sidebarAction.open()`. A context-menu entry ships in the first version as the second gesture path, and it opens the panel directly, being one of the gestures the side-panel open call documents.

**3. What the grant does after a navigation is stated in the UI.** Per Chrome's `activeTab` documentation (read 2026-09-03) the grant is per **tab** and lasts for the same-origin session: same-origin navigation keeps it, a cross-origin navigation revokes it. So after a cross-origin navigation the panel **keeps its rows** — they are state the extension holds on its own — and every control that would read or write the page is replaced by one line asking the user to click the toolbar icon, whose click re-grants `activeTab` for that tab and opens the popup, after which the panel's controls return once that grant is live because the panel never re-arms itself. Nothing silently fails and nothing silently re-grants.

**4. The application question is the primary object.** One row per detected question with its own composer, a free-text entry for a question the scan missed, and a rescan control for multi-step forms. Saved answers from other applications stay offered with their origin shown, as today. Both consent gates stay desktop-side (ADR-0009, ADR-0011); a gated-off row says where to turn the feature on instead of erroring.

**5. Iteration is honest about what each control does.** The preset chips are **rewrites of the latest version**: they reuse the existing wire verb's rewrite mode with the previous draft as the existing answer (the `answer.assist` request schema in `packages/shared/src/ipc/extension-protocol.ts` — no protocol change), and that mode never sees the résumé or the posting. **Regenerate** is a fresh grounded draft. The UI says which is which rather than presenting them as one dial. Versions are **session state only**, restorable, and Accept writes the version on screen into the field — nothing the model wrote is persisted, so [ADR-033](adr-033-no-model-written-agent-memory.md) is untouched.

**6. The character limit is a draft-time optional field, backward compatible.** The scan reads the field's own length limit from the DOM (extension-internal, no wire change), and the draft request gains **one optional field** carrying it, in TS↔Rust lockstep like every other bridge field. A desktop older than that field drops the unknown key; the panel falls back to a fit-the-limit rewrite chip carrying the number, so the feature degrades instead of breaking. Fields with no limit show no counter. Rejected: doing this with a rewrite only — it costs a second billable call and the shrink never sees the grounding.

**7. The permission pins gain ONE declared per-target delta.** Chrome needs the side-panel permission; Firefox has no equivalent permission and declares a manifest key instead, while the context-menu permission is on both targets. The assertions in `apps/extension/src/manifest.test.ts` move together before the suite is green. The both-targets assertion goes from pure equality to _equality modulo one declared delta_, plus a new assertion that the Firefox manifest carries its sidebar key whenever the Chrome manifest carries the side-panel permission. The hand-written allowlist, today one literal compared against the permissions of every target in turn, has to take a per-target shape, because only one of the two new permissions is on both builds. And the README parity assertion reads that same allowlist, so each new permission also needs a justification row in the extension README table. The intent the original assertions protect, no permission smuggled into one build only and none held without a published justification, survives because the exception is written down rather than inferred. The denylist is unchanged and neither new capability is on it.

**8. Wire errors keep fixed sentinel text, and gain no `code` field.** A gated-off row has to tell the ai-assist-opt-in-off refusal apart from the no-usable-provider one. Both are already fixed sentinels declared beside the handler (`AI_ASSIST_OFF_MESSAGE` and `NO_PROVIDER_MESSAGE` in `apps/desktop/src-tauri/src/extension_bridge/answer_assist.rs`); they move into `packages/shared/src/ipc/extension-protocol-constants.ts`, so the panel matches the shared constant instead of a copied string. The existing TS↔Rust parity test in `apps/desktop/src-tauri/src/extension_bridge/test.rs` is a hand-enumerated list of the wire-type constants, so it pins nothing it does not name: the delivering PR extends it to the two sentinels in the same shape. A machine-readable `code` was rejected: the sentinel **is** the code, and a second discriminator is a second thing to keep in lockstep for no new information. The wire shape does not change.

**9. The in-page floating card is DEFERRED.** It would mean `chrome.scripting` injecting our own UI into pages we do not control: style and event collisions on hostile DOM, a store-review question about injected UI that ADR-0009's fill-and-report shape does not already answer, and a surface where the honesty lines in decisions 3 and 5 are hard to keep visible. It would also not remove the need for decision 1's shared state, because a card injected into the page is not where that state lives. If it is ever built it gets its own ADR.

## Alternatives considered

1. **A single home in the side panel** (remove the Answer tools from the popup) — the smallest surface, and one place to keep honest. This was the main session's recommendation and was **rejected by the owner**: the popup is where the tools are found today, and moving them makes the feature harder to reach for no user-visible gain.

2. **Popup-only, with a richer in-popup card** — the cheapest option: no manifest change, no parity delta, no second view to keep in sync. Rejected because it does not fix the actual defect: the popup closes on blur, which is precisely the moment a copy-only tool is used.

3. **An in-page floating card next to the field** — the best proximity to the field being filled. Deferred, for the reasons in decision 9.

4. **Fitting the character limit with a rewrite pass only** — no wire change at all. Rejected, for the reasons in decision 6.

## Consequences

### Positive

- **A copy-only tool survives the click that uses it** — the whole point, and it is achieved without touching page access, the wire protocol or the consent gates.
- **One state means the two views cannot disagree.** A stream started in either is rendered by both; closing the popup loses nothing.
- **ADR-0009 holds unchanged.** No standing access, no content scripts, no auto-submit; the new gesture path (context menu) is one Chrome already counts as an `activeTab` grant.
- **The UI stops overstating iteration.** A user who expects Regenerate's grounding from a chip is told, rather than left to infer it from disappointing output.
- **The limit is applied where the grounding is**, so the common case is one billable call, not two.

### Tradeoffs

- **The manifest surface is no longer literally identical across targets.** The parity test now guards a declared exception rather than pure equality, which is strictly weaker; the exception has to be argued every time it grows.
- **After a cross-origin navigation the panel shows rows it cannot act on** until the next gesture. Deliberate and stated, but it is a state a user can sit in and be confused by.
- **Session-scoped state dies with the browser session**, so versions do not survive a restart. ADR-033 intends that, and it is still a real loss the UI must not paper over.
- **Opening the panel costs a second click on both browsers.** The toolbar click is spent on the popup that decision 1 keeps, so the panel is always one control further away than a single-surface design would make it.
- **A newer extension against an older desktop pays for two calls** in the limit case: the first draft ignores the limit, and the fallback rewrite chip is billable.
- **Two surfaces are two places for a UI regression to hide**, and the Chrome panel being per window means one tab's state can be on screen in more than one place.
- **It ships as one PR** (owner's call over the main session's three-way split), so a red CI cannot say which part broke it. The same PR also carries the desktop app's inline-rewrite fix, whose root cause is still being measured, so the branch grows by a change whose shape is not yet known.

### What is verified, and what is not

- **Verified against the source, as this branch actually shipped it**: `apps/extension/src/manifest.test.ts` pins the per-target permission delta (`DECLARED_PERMISSION_DELTA`), the both-targets-modulo-that-delta equality, that the Firefox manifest carries `sidebar_action` whenever the Chrome manifest carries `sidePanel`, that `openPanelOnActionClick` appears nowhere in either manifest, and the README-justification assertions covering both `sidePanel` and `contextMenus` — the denylist and the no-content-scripts pin are unchanged. `apps/desktop/src-tauri/src/extension_bridge/test.rs`'s `message_type_constants_match_ts` was extended, in the same hand-enumerated shape, to the two refusal sentinels now exported from `packages/shared/src/ipc/extension-protocol-constants.ts`; a companion numeric test, `answer_assist_max_chars_matches_ts`, pins the shared `EXTENSION_ANSWER_ASSIST_MAX_CHARS` ceiling to the Rust `DRAFT_CAP`. The `maxChars` wire field's shape (an optional positive integer, deliberately no upper bound on the wire) and the desktop-side parsing/clamping that reduces an over-large value to `DRAFT_CAP` (`parse_max_chars` in `answer_assist.rs`) are each covered by their own mutation-checked unit tests.
- **Verified against Chrome's documentation** (developer.chrome.com, read 2026-09-03), on the two rules decisions 2 and 3 rest on: the `activeTab` grant is per tab, is granted by the action, a context-menu item or a keyboard shortcut, and lasts for the same-origin session; and a declared default popup takes priority over opening the panel on the action click, which is why decision 2 opens the panel from inside the popup instead.
- **Not verified**: `parse_max_chars` has no caller in this branch — nothing puts `maxChars` into the draft prompt and nothing re-asks on an overshoot, so decision 6's code-enforced half is deferred until a sibling PR (#1103) merges and the extension path degrades to the panel counting the returned text itself, exactly as decision 6 says a pre-field desktop should. No visual verification was possible without running the app: not of the panel or the popup's "Open answer panel" control, and not of the desktop-side inline-rewrite popover fix this same PR carries (the clipping fix folded in alongside the panel) — each is verified only at the DOM-assertion / unit-test level, never rendered and looked at.

## References

- Manifest + permission pins: `apps/extension/src/manifest.ts`, `apps/extension/src/manifest.test.ts`
- Extension surfaces + background worker: `apps/extension/src/`
- Wire protocol: `packages/shared/src/ipc/extension-protocol.ts`, `packages/shared/src/ipc/extension-protocol-constants.ts`; Rust mirror `apps/desktop/src-tauri/src/extension_bridge/msg.rs`
- Handler + refusal sentinels: `apps/desktop/src-tauri/src/extension_bridge/answer_assist.rs`
- Domain doc: `docs/knowledge/extension-domain.md` (reserved-verb pattern, lockstep rule, wire-error discipline)
- Prior decisions: [ADR-0009](0009-assisted-autofill.md) (assisted autofill: user-gestured, `activeTab`, never auto-submit), [ADR-0011](0011-extension-ai-assist-optin.md) (AI assist is a separate billable opt-in), [ADR-015](adr-015-extension-bridge-websocket-save-origin.md) (bridge transport), [ADR-033](adr-033-no-model-written-agent-memory.md) (no model-written memory), [ADR-0005](0005-network-egress-privacy-boundary.md) (egress boundary), [ADR-0010](0010-bridge-hmac-handshake.md) (bridge auth)
