---
status: accepted
---

# Live Model Listing — Providers Are the Source, No Hardcoded Defaults

## Context

Every cloud provider (`openai`, `anthropic`, `gemini`, `ollama-cloud`) had hand-curated model lists in TypeScript (`provider-meta.ts`), consumed by onboarding's `AISelectionStep` and `CloudProviderPanel`. The app defaulted to the first entry in each provider's list — no sort, no ranking.

In July 2026, **four stale-model defects shipped in a single session**:

1. **`text-embedding-004`** — retired by Google while still the default Gemini embedding model (discovered via embeddings)
2. **OpenAI reasoning-effort gate** — keyed on legacy o-series, missed newer reasoning models
3. **`gemini-3-pro-preview`** — shut down while in the curated list
4. **Gemini list neighbors** — re-checking that list found its three neighbours also dead, including `gemini-2.0-flash`, the array's first entry and therefore the onboarding default

All four bypassed testing because the CI build doesn't carry API keys (so live catalogues are never fetched), and the curated arrays were never re-audited for retirement after shipping.

The root cause was structural: there is no source of truth _except_ each provider's own `/models` or equivalent endpoint — when an array is the cache + fallback, it rots silently.

## Decision

**Remove all hard-coded model arrays.** The provider's live catalogue (`listProviderModels`) is the only source.

### Specific Changes

1. **`AiProvider::list_models()` now returns `AppResult<Vec<Value>>`** (was `Vec<Value>`).
   - Errors from failed fetches, missing auth, transport problems, unparseable responses, or empty catalogues are propagated to the caller.
   - The TypeScript layer (`ai_list_provider_models` IPC command) rejects on any error, classified through `friendly_api_error()` (401/403 → config error; 429/5xx → network error; others → descriptive).
   - **Before**: all failure modes were silently collapsed to `vec![]`, making an offline app look identical to an empty catalogue.

2. **Model metadata is now carried by the live response**, not guessed locally:
   - `ProviderModelInfo` has `{ name, displayName?, createdAt?, contextLength? }` — every field optional because no provider returns all.
   - **Gemini returns no `createdAt` at all** — this is the evidence for why there is no auto-ranked default (a "newest first" heuristic would silently fail for the one provider whose default actually rotted).
   - Both cloud list endpoints paginate; I/O is bounded by a cumulative deadline across every request.

3. **Hand-curated model arrays are deleted:**
   - TypeScript: the `models: [...]` array on each cloud entry in `provider-meta.ts`, plus the `provider-meta.test.ts` contract test that asserted each default pointed to a member of that list (deleting both guard and guarded entity together).

4. **Onboarding defers model selection** until after a key is entered and the provider answers:
   - The key-entry step is followed by a "pick a model" step that fetches the live list from the provider.
   - **No model is pre-selected.** A hard-coded onboarding default is exactly the defect class this change avoids.
   - "Skip" remains available for Ollama when no model is ready (via `canContinue` gating on installed + not too heavy).

5. **Cache fallback behavior is explicit and split by purpose:**
   - **`'display'` mode** (Settings UI, model picker):
     - A fresh fetch is attempted.
     - If it fails but a cached list exists for that provider + base URL, the cached list is served with a `{ cached: true }` flag for visual cue.
     - **If there is no cache, the error is raised** — UI shows an error state, not an empty dropdown. A missing key will say so.
   - **`'verify'` mode** (onboarding's Continue button, verifying a newly-entered key):
     - A fresh fetch is attempted.
     - **Cache fallback is never used**, even if available. A successful fetch proves the key works; serving a cached list from a _prior_ key could let a newly-entered wrong/revoked key pass verification.

6. **CLI-agent aliases remain.** Local binaries (Claude Code, Codex, etc.) expose no catalogue endpoint, so aliases stay in the source:
   - `cli_agent/claude_code.rs` line 21: `const MODELS: &[&str] = &["sonnet", "opus", "haiku", "fable"];`
   - The same risk (stale names) exists, but there is no catalogue to fetch from, and missing/wrong names are caught at invocation time.

### What Is NOT in Scope

- Scheduled CI staleness checks were approved in the design but **dropped at implementation**: they would false-positive on nearly everything because:
  - `RATES` deliberately retains retired model rows (e.g. `gemini-3-pro-preview`, `text-embedding-004`) so historical spend still prices correctly.
  - CLI aliases appear in no catalogue.
  - The false-positive rate means maintainers would ignore the signal.

## Rationale

### Why No Auto-Ranked Default?

**Gemini's `/models` endpoint returns no `createdAt` field.** Anthropic, OpenAI, and Ollama all provide timestamps; Gemini does not. A "sort by creation date, pick the newest" rule works for three providers and silently fails for the one whose default actually rotted. Because:

- If `createdAt` is absent in every Gemini entry, sorting by it returns either the list unchanged or an error.
- Defaulting to the first entry (`createdAt` missing → sort unchanged → first) is fragile and invisible: the API response can change order, and there is no validation that the first entry is actually suitable.
- Trying to infer recency from a release page or API docs would duplicate the provider's own data source — now maintained in two places.

**Centralized, versioned model fetches are more robust than local heuristics.**

### Why Is Verify Mode Different From Display Mode?

The cache is keyed by `(provider, baseUrl)` with **no credential identity**. When a user enters a new API key:

1. They go through onboarding's "enter key" → "pick model" flow.
2. The model picker calls `useListProviderModels(provider, 'verify')`.
3. If the key is wrong or revoked, the fetch fails.
4. If **we served a cached list anyway** (from a _prior_, valid key for the same provider + URL), the UI would show "key is valid, here are your models" — but the _actual_ key is broken.
5. The user would continue, hit a generation error, and see an unhelpful "invalid key" much later.

**`'verify'` mode must not accept cache fallback**, because cache cannot encode which credential it came from.

`'display'` mode (Settings AI tab) _can_ use cache fallback because the user already has a stored key — they're just re-rendering an existing list, not testing a new one. If the network is down, showing "last known models" with a `cached: true` flag is helpful.

## Consequences

- **Onboarding now has a network round-trip on the model-selection step.** This is intentional. A dead default is worse than a slower first run.
- The provider's supported-models list is **refreshed per React Query staleTime**: 5 minutes for Settings' model-list fetch (`staleTime: 300_000` in `ModelSelector`), and 5 minutes (`QUERY_TIMES.VERY_LONG = 300_000`) for the service hook. Cache is considered stale after that interval, not invalid — a stale cached list is served if the live fetch fails (display mode only).
- Both OpenAI and Gemini's `/models` endpoints support pagination; the Rust layer implements bounded iteration (cumulative deadline across all pages).
- Settings → AI shows "loading…" briefly while the model list is fetched, then an error state if the list fails.
- Export spend and capability checks still use `RATES` and `ModelCapabilities` matrices (defined in the Rust backend), which are static and must be manually maintained — but now only for _capability_ info, not model discovery. A model not in either matrix is still supported (the validator is permissive).
- TypeScript caches the last-known list per provider + base URL for display purposes (via `readModelListCache` / `writeModelListCache` in `model-list-cache.ts`). **This cache is never used for verification.**

## Alternatives Considered

1. **Keep the array, add a CI job to refresh it weekly**
   - Rejected: Would false-positive on retired models in `RATES` and CLI aliases, and maintainers would ignore it (see "What Is NOT in Scope").

2. **Use model creation date for onboarding default, fall back to the first entry if none exists**
   - Rejected: Gemini's lack of `createdAt` makes this silently unreliable.

3. **Embed the provider's `/models` response at build time**
   - Rejected: Doesn't solve the stale-after-release problem; just delays it.

4. **Display a warning when a model is no longer in the provider's list**
   - Deferred: `ModelCapabilities` checks still happen at generation time; a missing model is caught there, not at onboarding. A "model no longer available" warning on Settings would be a natural follow-up.

## Residual Risks & Accepted Costs

1. **A provider removes an effort level a model previously accepted.**
   - `ModelCapabilities` is static; the backend doesn't re-query per model at generation time.
   - If a model suddenly stops accepting `reasoning_effort="high"`, the app won't know until a generation fails.
   - Mitigation: Rare (providers carefully maintain backward compatibility for reasoning effort). Edge case caught at generation, not silently-bad output.

2. **`RATES` row for a retired model still exists and over-estimates cost.**
   - Example: `gemini-3-pro-preview` is gone from the API, but `RATES` still has `{ model: "gemini-3-pro-preview", rate: $X }` for historical spend to price correctly.
   - A new user could theoretically select it if they found the name in old docs or a cache. They would get a generation error, not a false cost estimate.
   - Mitigation: `ProviderId::validate_model` rejects models that look like they belong to a different provider (by checking known prefix patterns like `claude*` for Anthropic, `gemini*` for Gemini, `gpt*`/`o1*`/`o3*` for OpenAI), but allows any unknown names because providers ship new models weekly.

3. **Onboarding is slower on first run for cloud providers** (extra network round-trip).
   - Acceptable: A fresh install that defers to "live data, no magic defaults" is more maintainable long-term.

## Why This Shipped as Two PRs, Not Three

The initial plan was three: (1) expose errors from `list_models`, (2) remove curated arrays, (3) teach onboarding to defer model selection. Steps 1 and 2 merged together (**#935 + #936**) because the curated arrays and their consumers (`CLOUD_DEFAULT_MODELS` maps in two onboarding surfaces, the contract test guarding them) are a single defect. Shipping step 2 alone would leave an intermediate state worse than either endpoint: arrays gone, contract guard gone, but the hardcoded defaults still there reaching for nothing.

## Follow-Up Regression Fixed in #937

#936 added a keyless carve-out for `openai-compatible` (LM Studio and vLLM users can list models without storing a key). But it only checked `if provider == 'openai-compatible'`, not whether the provider was actually configured. For unconfigured users:

- `baseUrlFor('openai-compatible')` returns `undefined`
- The backend falls back to `https://api.openai.com/v1` with no auth header
- `ModelSelector` mounts on six surfaces
- Every user (including fully local, offline Ollama users who never touched openai-compatible) made unauthenticated requests to `api.openai.com` on every relevant page load — breaking the local-first promise

#937 fixed it by requiring `openai-compatible` to be configured (either a stored base URL or a stored key) before it can fetch. The same guard now applies uniformly across all surfaces (`isProviderConfigured` predicate rather than per-site special cases).

## Important Notes

- **A "model is gone" scenario is now caught at generation time**, not onboarding. The user picks a model from the provider's live list → starts a generation → the model now raises a provider-level error (404, deprecated, etc.). The error message is specific.

## References

- `packages/shared/src/ipc/contracts/ai.ts` — `ProviderModelInfo` interface, `listProviderModels` contract
- `apps/desktop/src-tauri/src/commands/ai.rs` — `ai_list_provider_models` command (now returns `AppResult<Value>`)
- `apps/desktop/src-tauri/src/commands/ai_provider/mod.rs` — `AiProvider::list_models()` trait method (now returns `AppResult`)
- `apps/desktop/src-tauri/src/commands/ai_provider/{anthropic,gemini,openai,ollama_cloud}.rs` — per-provider implementations with pagination + deadline bounding
- `apps/desktop/src/renderer/services/use-ai-provider/use-ai-provider.ts` — `fetchProviderModelsWithCache()`, `useListProviderModels()`, purpose-split cache logic
- `apps/desktop/src/renderer/lib/ai-providers/model-list-cache.ts` — local cache implementation (display only, never used for verification)
- `apps/desktop/src/renderer/features/onboarding/steps/ollama/CloudProviderPanel/index.tsx` — onboarding's model picker, uses `'verify'` mode
- `apps/desktop/src/renderer/features/settings/components/ai-settings/CloudProviderConfig/index.tsx` — Settings' model picker, uses `'display'` mode
