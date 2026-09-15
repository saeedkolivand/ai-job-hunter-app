# ADR-050 — Extension read tier and live settings verbs

**Status:** Accepted

**Date:** 2026-09-11

**Deciders:** repo owner, main session

## Context

[ADR-038](adr-038-agent-cli-full-parity-two-tier.md) built the generic `agent.query`/`agent.call`
tier for the CLI/MCP caller only — the extension had no path into it. The extension-round design
grill (`.claude/scratch/extension-round-design.md`, decisions 1 and 2) asked for a second,
narrower caller class: let the paired browser extension read the same curated/generic surface,
but only the rows the AI CLI proof-ceremony exists to protect against are the ones that stay
closed to it. Separately, the redesigned Settings page (decision R7) needed a way to flip the
extension's own opt-in switches from the extension itself, which raised a "who is the source of
truth for consent" question the owner resolved explicitly.

## Decision

1. **Caller class, resolved once per connection.** The WS handshake's `Origin` now resolves into
   `CallerClass` (`extension_bridge::caller_gate::CallerClass`, re-exported as `super::CallerClass`)
   alongside the existing CLI label — `Extension` when the Origin matches the extension allowlist
   (`auth::is_extension_origin`: the pinned Chrome id, the `moz-extension://<uuid>` shape, or a dev
   origin, or the native-messaging relay's sentinel — that relay forwards the same paired
   extension's frames 1:1), never the CLI sentinel. `AGENT_CLI_ORIGIN` is never widened.
2. **Read-only projection, gated by the Autofill opt-in.** For `CallerClass::Extension`,
   `agent.query` and `agent.call` dispatch only when `Effect::Read` (and only once
   `autofill_enabled()` is true); any other Effect (Reversible, Irreversible, NotExposed) refuses
   with a fixed sentinel (`effect_not_allowed_for_extension`) that never enters the CLI's
   confirm-proof ceremony — there is no proof hint, no ceremony, nothing for this caller to chase.
   With Autofill off, the whole tier refuses (`extension_read_gate`), no partial data. CLI
   dispatch through the same code paths is unchanged — see `msg.rs`'s doc comments on
   `AGENT_QUERY`/`AGENT_CALL` and the gate matrix tests beside `CallerClass`.
3. **A dedicated reply cap for this caller** (`EXTENSION_RESULT_MAX_BYTES`,
   `extension_bridge::mod`) — refuse over cap, never truncate, same discipline as ADR-038's
   `result_too_large`. Per-pairing throttle reuses the existing `AgentQueryThrottle` bucket.
4. **No confirm ceremony, ever, for this caller.** The extension cannot reach a Reversible or
   Irreversible row through either tier; there is no analogue of ADR-038 §4 for
   `CallerClass::Extension`, by construction rather than by convention.
5. **Settings verbs, deliberately outside the generic tier.** `settings.get`/`settings.set`
   (`extension_bridge::settings`) are a dedicated Reversible verb pair scoped to the extension's
   own opt-in switches (`SettingsKey`, `apps/desktop/src-tauri/src/extension_bridge/settings.rs`),
   reachable by the extension caller **regardless** of the Autofill gate (they are how the user
   turns it on) and refused for the CLI and for `Other`. They apply through the same setters the
   Tauri Settings commands use, so the desktop UI stays coherent, and — the guard rail the owner
   asked for after the desktop-side-consent concern was raised — every successful `settings.set`
   raises a Notification Center entry via `push_and_notify` before replying, so a flip made from
   the extension is never silent to the person sitting at the desktop. `settings.set` carries its
   own per-pairing throttle (guard rail #4); `settings.get` is unthrottled (a pure read of the
   user's own device-local settings). A malformed key/value refuses with `invalid_settings_request`.
6. **`document.export`/`document.result` (PR2 — documents into ATS) is likewise a dedicated verb,
   not a Resource in this tier.** It rides the same `CallerClass::Extension` + Autofill-opt-in gate
   this ADR establishes, but sits outside the generic `agent.query`/`agent.call` dispatch and
   outside `EXTENSION_RESULT_MAX_BYTES` (a rendered document does not fit that cap) — see
   `extension_bridge::document_export`.

## The desktop-side-consent concern (raised and resolved)

Letting the extension write consent state at all cuts against the posture [ADR-0009](0009-assisted-autofill.md)
established — that autofill's gate lives on the data owner, not in the extension, specifically so
disabling it is trustworthy. The concern raised in the grill: a compromised or careless extension
could now turn its own gates back on. The owner's resolution, over the alternative of read-only
mirrors (below): the desktop remains the enforcement point for every gate at USE time, unchanged —
`settings.set` only moves the stored flag, exactly as the Settings UI's own toggle would, through
the identical setters; the WRITE from the extension is not privileged, it is the same shape the
owner's own click already performs, and it is never silent (guard rail above). This is the
narrowest possible expansion of the extension's authority: the `SettingsKey` switches only, never the
generic Reversible/Irreversible tier.

## Consequences

### Positive

- The extension can show live job/document context without duplicating desktop query logic or
  widening the CLI's own tier.
- A compromised/patched extension gains no new destructive reach: Reversible and Irreversible rows
  stay behind the generic tier's own confirm ceremony, reachable only from `CallerClass::Cli`.
- The Settings page's live toggles (R7) work without a new IPC command and without moving the
  enforcement point off the desktop.

### Tradeoffs

- Two refusal sentinels (`extension_read_gate`, `effect_not_allowed_for_extension`) are
  extension-only vocabulary, distinct from the CLI's `Refusal` variants — a caller must know its
  own class's vocabulary rather than one shared list.
- The reply cap is smaller than the CLI's frame budget (`EXTENSION_RESULT_MAX_BYTES` vs.
  `MAX_FRAME_BYTES`), so a `agent.call`/`agent.query` result that fits for the CLI can refuse for
  the extension — accepted, since the extension's consumers (job/document projections) are
  designed to be small.
- Settings state can now change without the owner touching the desktop UI at all; mitigated by the
  mandatory notification tail, never by a confirm ceremony (would defeat the point of a quick
  toggle from the extension).

## Alternatives considered

1. **Read-only settings mirrors** ("Change in app →" links, no write verb). Keeps ADR-0009's
   desktop-only consent line perfectly intact. Rejected by the owner after the concern above was
   put to them explicitly — the UX cost (leaving the extension to switch apps for every toggle)
   outweighed the marginal trust cost once the notification guard rail was added.
2. **Read + Reversible for the extension's generic tier** (let `agent.call` dispatch Reversible
   rows too, not just Read). Rejected: would reopen the exact class of write the dedicated
   settings verbs were built to avoid needing, and blur the caller-class boundary decision 2
   depends on.
3. **No generic tier at all for the extension** (curated Resources only, hand-projected per need,
   as `profile.get`/`applied.check`/etc. already are). Rejected: duplicates the CLI's own
   `agent.query`/`agent.call` machinery for a caller with the identical read need, for no
   additional safety — the Read/Autofill gate already does the work a bespoke projection would.

## References

- Caller class + gate: `extension_bridge::caller_gate::CallerClass`,
  `extension_bridge::auth::is_extension_origin`.
- Dispatch + sentinels + cap: `extension_bridge::mod::EXTENSION_RESULT_MAX_BYTES`,
  the `agent.query`/`agent.call` dispatch arms in `extension_bridge/mod.rs`.
- Settings verbs: `extension_bridge::settings` (`SettingsKey`, `handle_settings_get`,
  `handle_settings_set`).
- Wire constants: `extension_bridge::msg::{SETTINGS_GET,SETTINGS_RESULT,SETTINGS_SET}`,
  mirrored in `packages/shared/src/ipc/extension-protocol-constants.ts` +
  `extension-protocol.ts`.
- Design record: `.claude/scratch/extension-round-design.md` decisions 1, 2, R7.
- Extends [ADR-038](adr-038-agent-cli-full-parity-two-tier.md); amends
  [ADR-0009](0009-assisted-autofill.md) (consent may now be granted/withdrawn from the extension
  as well as the app).
