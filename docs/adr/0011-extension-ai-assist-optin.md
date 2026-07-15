---
status: accepted
---

# Extension AI-assist opt-in — billable egress, distinct from free autofill

## Context

The published MV3 extension already ships two free, local-only features: autofill (fill empty form fields from the Contact Profile) and answers capture/replay (record and suggest prior answers). Both gate on the same `autofill_enabled` toggle and carry PII over the bridge, but stay device-local — no billable AI compute.

A new feature, `answer.assist`, lets the extension ask the desktop app for AI-drafted answers to application questions. Unlike autofill's one-time per-frame token cost (per-connection `AssistStreamRegistry`), this verb is **billable**: every call spends provider budget (API dollars, daily limits), and the model/provider snapshot must be locked at enable time so the user knows their budget is being spent on a known AI cost, not an invisible config change.

The feature request is to gate this separately from the free autofill/capture toggle: a **separate opt-in** (persisted via `BridgeState`), independent consent, and a **provider/model snapshot** stored at enable time — so if the user later swaps AI providers (e.g. Claude→GPT-4) the extension still uses the provider that was active when they opt in (until they re-enable and re-snapshot).

This decision separates the **free local-data tier** (autofill, answers capture, answers suggest) from the **billable AI tier** (answer assist) — distinct consent classes per ADR 0005 rule 6 (egress carrying billable charges is a separate consent gate from PII transit).

## Decision

Ship `answer.assist` with a **separate, independent opt-in gate**, default OFF, enforced desktop-side. At enable time, **snapshot the active provider/model** into a persistent `AiAssistConfig` and **use that snapshot** for every answer-assist call, regardless of later user AI-provider changes.

**Enable-time snapshot:**

1. User toggles "AI answer assist" ON in Settings.
2. Desktop immediately captures the currently-selected AI provider + model + base_url (from the active config).
3. Desktop stores the snapshot as `AiAssistConfig { enabled: true, provider, model, base_url }` in a persistent JSON file, immutable across later provider switches.
4. Desktop refuses `answer.assist` calls when the snapshot's `enabled == false` or when the snapshot is missing (fresh install, or reset).

**Separation from autofill gate:**

- The `autofill_enabled` toggle in Settings remains (gate for `profile.get`, `answers.save`, `answers.suggest`).
- The new AI-assist toggle is adjacent, gated by `BridgeState::ai_assist_enabled()` (queried via `extension_bridge_ai_assist_enabled` IPC command), with a clear cost/budget notice (e.g. "Requires AI provider credits — only charged when you request an AI-drafted answer").
- A user can have `autofill_enabled=true` and AI-assist `disabled=false` to get free suggestions without paying for AI drafting.
- The toggle is reset-to-OFF on factory reset (user data wipe), same as autofill.

**Snapshot invariant:**

- The snapshot is **immutable once captured** — it does not change when the user later updates their AI provider/model in Settings (those changes affect new agent runs, not the extension's cached provider).
- When the user manually re-toggles the AI-assist toggle, the snapshot is re-captured (capturing the _new_ current provider/model/base_url), so they have a way to track forward if they explicitly want to switch the extension's provider.
- The snapshot persists as one JSON blob (`AiAssistConfig`) to the file named by the constant `AI_ASSIST_OPTIN_FILE`.

**Desktop enforcement:**

- Desktop `handle_answer_assist` checks `BridgeState::ai_assist_enabled()` BEFORE checking the snapshot (fast refusal path).
- If the snapshot is missing or the `enabled` field is false, desktop refusal text: "AI answer assist is not configured — re-enable it in Settings to use the current provider."
- The extension **never sees the provider/model/base_url**; it only receives the drafted answer or an error.

## Considered options

1. **Separate AI-assist opt-in + provider/model snapshot (chosen).** Clear consent boundary between free (autofill) and billable (AI); user knows the cost and can revert a provider switch by re-enabling. Snapshot decouples the extension's AI cost from invisible config drifts.
2. **Single unified toggle, no snapshot (autofill and AI under one gate).** Rejects: mixes free and billable under one toggle, violating ADR 0005 rule 6; user cannot control AI cost separately; a provider switch changes the extension's behavior without user re-consent.
3. **Snapshot but always use the latest provider (no immutability).** Rejects: defeats the purpose of a snapshot; a user who swaps to an expensive model later is surprised by budgets disappearing; snapshot is bookkeeping clutter without meaning.
4. **UI toggle in the extension popup (not desktop Settings).** Rejects: the gate must live on the data owner (desktop) to enforce; extension UI cannot be trusted; also, PII/billing settings belong in the desktop app's authoritative Settings, not buried in an extension popup.

## Consequences

- **A new persistent `AiAssistConfig` structure** `{ enabled, provider, model, base_url }` is stored by the desktop (via `BridgeState::set_ai_assist`) and checked on every `answer.assist` call. The persisted filename is the constant `AI_ASSIST_OPTIN_FILE`.
- **The snapshot persists across desktop app updates** and across bridge reconnects; it is reset to `{ enabled: false }` (clears provider/model/base_url) only on factory reset or explicit user toggle-off+re-enable.
- **The extension never learns the provider/model** — it only sends a `answer.assist` request and receives drafted text or an error. Provider identity is invisible to the extension; the bridge validates that the requesting `reqId` has a registered job (via per-connection `AssistStreamRegistry`). The `base_url` field is consumed server-side only when resolving a custom-endpoint provider.
- **Costs are billable per-extension-call**, not per-provider-session: the daily/budget limit counting lives in the shared provider limiter (`charge_provider_daily` in `limits/mod.rs`, via `charge_compose_budget` in `answer_assist.rs`), which gates EVERY compose call (agent runs or extension AI assists) off the same `PROVIDER_DAILY_MAX` budget.
- **The toggle is documented in Settings** with a cost notice (e.g., "Powered by [provider name]. Only charged when you request an AI-drafted answer."). The snapshot provider/model are shown when enabled, so the user knows which provider their budget spend applies to.
- **Provider/model snapshots are a known tech debt pattern** (documented in memory as incurred twice before) — a future, higher-priority goal is to shift billing decisions to a backend-owned active-provider store so the frontend never needs to snapshot. For now, the snapshot is the pragmatic pattern.

## References

- Toggle + snapshot: `apps/desktop/src-tauri/src/extension_bridge/mod.rs` (`BridgeState::ai_assist_enabled()`, `BridgeState::ai_assist_snapshot()`, `BridgeState::set_ai_assist()`, `AiAssistConfig`, `AI_ASSIST_OPTIN_FILE`).
- IPC commands: `apps/desktop/src-tauri/src/commands/extension_bridge.rs` (`extension_bridge_ai_assist_enabled`, `extension_bridge_set_ai_assist_enabled`).
- Renderer UI: `apps/desktop/src/renderer/features/settings/components/accounts/ExtensionBridgeSection` (Settings toggle).
- Desktop gate + billing check: `apps/desktop/src-tauri/src/extension_bridge/answer_assist.rs` (`handle_answer_assist`, `resolve_answer_assist`, `charge_compose_budget`).
- Per-connection stream registry (pre-compose cancellation guard): `apps/desktop/src-tauri/src/extension_bridge/stream.rs` (`AssistStreamRegistry`, `CancelledEarly` invariant).
- Protocol: `packages/shared/src/ipc/extension-protocol-constants.ts` (`ANSWER_ASSIST_REQUEST`, `answer.assist`, `assist.chunk`, `assist.done`).
- Shared daily budget: `apps/desktop/src-tauri/src/limits/mod.rs` (`PROVIDER_DAILY_MAX`, `charge_provider_daily`).
- Related: [ADR 0005](0005-network-egress-privacy-boundary.md) (egress consent class rule 6), [ADR 0009](0009-assisted-autofill.md) (free autofill/capture tier), [ADR 0010](0010-bridge-hmac-handshake.md) (bridge auth).
