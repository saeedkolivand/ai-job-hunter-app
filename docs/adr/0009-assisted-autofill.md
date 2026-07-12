---
status: accepted
---

# Assisted autofill — user-initiated, generic-matcher, transparent, no-persistence

## Context

Every major 2026 job-hunt competitor (Simplify, Teal, LazyApply) ships browser form-fill, and the free tier of the market leader does it well. AI Job Hunter already holds the user's contact details in the authoritative **Contact Profile** and already has a paired, authenticated desktop⇄extension bridge (loopback WebSocket, per-frame pairing token) built for **Extension import** (reading a job _into_ the app). The symmetric capability — writing the user's own contact details _out_ onto an employer's application form — was the single most-requested "action" feature the app lacked, and the one place a competitor's free tier beat us on capability rather than on privacy or cost.

The hard question is not whether to build it but how to build it without breaking the two things that are actually our moat: the **local-first privacy boundary** ([ADR 0005](0005-network-egress-privacy-boundary.md)) and the **human-in-the-loop, never-auto-apply** brand posture. Autofill moves PII (name, email, phone, socials) onto a third-party page, which is exactly the kind of egress ADR 0005 governs, and it sits one small step away from the auto-apply line we have deliberately refused to cross.

The design was stress-tested in a grill-with-docs session against the bridge protocol, the pairing/token auth model, Chrome Web Store + Firefox AMO policy, the Contact Profile authoritative model, path-privacy/PII rules, and the app's no-silent-behavior honesty posture. Six decisions were resolved and accepted; this ADR records them.

## Decision

Ship **assisted autofill**: a user-initiated, click-to-fill action in the published MV3 extension that fills empty form fields on the current page from the Contact Profile, reviews-and-submits by the human, and never persists PII in the browser.

1. **User-initiated, per-invocation, no broad host access.** Fill runs only when the user clicks "Fill this form", via `activeTab` + `chrome.scripting.executeScript` on the current tab. There are **no broad `host_permissions`** — the extension gains no standing access to any site; each fill is a one-shot, user-gestured injection. This is what lets it work on _any_ site the user is on while keeping the AMO `data_collection_permissions: ["none"]` posture and a minimal Chrome permission set.

2. **Generic field matcher, not a per-ATS scraper.** Fields are identified by a tiered heuristic — `autocomplete` attribute → label/`aria-label` text → `name`/`id` → `placeholder` — against a fixed key set (`fullName`, `email`, `phone`, `location`, `linkedin`, `github`, `website`). Only **empty** fields are filled; a field whose key is **ambiguous** (see the denylist) is skipped, never guessed. This generalizes across boards instead of coupling us to Workday/Greenhouse/Lever DOM shapes that churn.

3. **Never auto-submit.** The extension fills; the human reviews and clicks Submit. There is no code path that submits a form. This keeps autofill on the safe side of the auto-apply line the product does not cross.

4. **PII travels over the existing authenticated bridge, fetch-fresh.** The content script requests the profile from the desktop app over the existing loopback WebSocket using one new message pair (`profile.get` → `profile.result`), authenticated by the same per-frame pairing token as import. The profile is **fetched fresh at fill time and never written to `chrome.storage`** — nothing PII-bearing persists in the browser, so an extension compromise or an uninstall leaks nothing at rest.

5. **Opt-in, default OFF, enforced desktop-side.** Autofill is gated by a desktop setting (default OFF, reset-to-OFF on data reset). The **desktop refuses `profile.get` when the toggle is off** — the gate lives on the data owner, not in the extension, so disabling it actually stops PII from leaving the device (the ADR 0005 rule for egress carrying user data). With the toggle off, the profile never crosses the bridge.

6. **Transparent about what it did and honest about limits.** After a fill the extension shows an in-page summary of which fields it set. The disclosed, non-negotiable limits: a **résumé FILE cannot be uploaded** from a content script (browsers forbid programmatic file-input population), and **complex custom ATS** (Workday shadow DOM, multi-step wizards) fill **partially at best**. These are documented in the extension README and the privacy page, not papered over.

## Considered options

1. **Assisted, generic, transparent, no-persistence, opt-in (chosen).** Matches the market's most-used capability while preserving both the privacy boundary and the human-in-the-loop brand. Cost: partial fills on complex ATS, and no file upload — accepted and disclosed.
2. **Per-ATS deep integrations (Workday/Greenhouse/Lever adapters).** Rejected: higher fill quality on a handful of boards, but couples us to churning private DOM, multiplies maintenance, and still can't upload a file. The generic matcher degrades gracefully everywhere instead of excelling in four places and breaking silently elsewhere.
3. **Persist the profile in `chrome.storage` for offline/instant fill.** Rejected: puts PII at rest in the browser, widening the blast radius of an extension compromise and contradicting ADR 0005. Fetch-fresh over the authenticated bridge costs one round-trip and leaks nothing at rest.
4. **Broad `host_permissions` so fill is always available without a click.** Rejected: standing access to all sites is a heavier store-review and privacy posture than the feature needs, and breaks the `["none"]` AMO data-collection stance. `activeTab` on a user gesture gives the same reach with none of the standing access.
5. **Auto-submit after fill (one-click apply).** Rejected: crosses the auto-apply line the product refuses to cross; the human-in-the-loop review is the brand and legal moat, not a limitation to optimize away.

## Consequences

- **A new bridge message pair (`profile.get`/`profile.result`) enters the protocol** and must stay in TS↔Rust lockstep like every other bridge message; the profile projection is flat (the seven contact keys only), never the full Contact Profile record.
- **The pairing token's blast radius grows.** A harvested token could previously import jobs; with autofill enabled it can also **read the Contact Profile** via `profile.get`. This is disclosed in the extension threat-model note and README, and bounded by the opt-in gate (token reads nothing when autofill is OFF).
- **The desktop toggle is the enforcement point**, not extension UI — reviewers and future authors must keep the refusal on the desktop handler; moving the gate into the extension would silently break the guarantee.
- **Autofill is now the sanctioned "write user data out" template**, the mirror of Extension import's "read job in": user-gestured, authenticated, fetch-fresh, opt-in, never-submit. Future outbound-to-page features follow this shape.
- **The honest-limits disclosure (no file upload, partial complex-ATS)** is a documentation obligation, not optional polish — hiding it would violate the no-silent-behavior posture.

## References

- Protocol: `packages/shared/src/ipc/extension-protocol.ts` + `extension-protocol-constants.ts` (`profile.get`/`profile.result`, `ExtensionProfileResult`).
- Desktop handler + gate: `apps/desktop/src-tauri/src/extension_bridge/mod.rs` (`handle_profile`, `resolve_profile`, `autofill_enabled`).
- Matcher + fill: `apps/extension/src/lib/autofill.ts` (tiered matcher, ambiguous denylist, `isHidden`), `apps/extension/src/fill.ts` (import-free injected script), `apps/extension/src/background.ts`.
- Opt-in setting: `apps/desktop/src/renderer/features/settings/components/accounts/ExtensionBridgeSection`.
- Disclosure: `apps/extension/README.md`, `landing/privacy.html`.
- Related: [ADR 0005](0005-network-egress-privacy-boundary.md) (egress boundary), Extension import + Pairing token in `docs/CONTEXT.md`.
