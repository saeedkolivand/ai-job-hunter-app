<!-- GENERATED FILE — DO NOT EDIT.
     Source: packages/shared/src/ipc/contracts/*.ts · Generator: scripts/gen-api-docs.mjs
     Change the contracts (signatures live in the code, prose lives in TSDoc on the
     contract members) then run `pnpm gen:api`. CI fails on a stale copy. -->

# IPC API Reference

Every renderer ↔ Rust call is a typed contract in `packages/shared/src/ipc/contracts`, and this page is
generated from those contracts. A method that is documented here exists; one that is not,
does not. Methods with no description have no TSDoc on the contract yet — add it next to
the signature, not here.

The renderer reaches the shell exclusively through `AppClient`
(`createTauriInvokeClient()` in `apps/desktop/src/tauri-client/index.ts`), consumed via the
React Query service hooks in `apps/desktop/src/renderer/services/`.

> **Never call `window.__TAURI_INVOKE__` directly.** Use the service hooks.

## Transport

| Direction       | Mechanism                      | Description                |
| --------------- | ------------------------------ | -------------------------- |
| Renderer → Rust | `tauri.invoke(cmd, payload)`   | Request/response (promise) |
| Rust → Renderer | `tauri.listen(event, handler)` | Push events (subscription) |

A subscription method (`onX(handler)`) returns its own unsubscribe function.

Renderer protocol version: `1.1.0` (`PROTOCOL_VERSION`, `packages/shared/src/ipc/contracts/index.ts`).
It must match `system.getProtocolVersion()` from the shell; a mismatch means a
partially-updated install.

Adding a capability is a five-step change — contract, Rust command, `tauri-client`,
service hook, query key — see `AGENTS.md` rule 14.

## Namespaces

| Namespace                             | Methods | Summary                                                                                                                                |
| ------------------------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| [`ai`](#ai)                           | 29      |                                                                                                                                        |
| [`aiGenerations`](#aigenerations)     | 5       |                                                                                                                                        |
| [`applications`](#applications)       | 10      | Application-tracking capability (ADR 0001).                                                                                            |
| [`autopilot`](#autopilot)             | 11      | Job-discovery agent:                                                                                                                   |
| [`boards`](#boards)                   | 6       |                                                                                                                                        |
| [`cliAgents`](#cliagents)             | 3       |                                                                                                                                        |
| [`contactProfile`](#contactprofile)   | 3       | The candidate's stored contact fields (name, email, phone, location, LinkedIn, GitHub, website, custom links), localized per language. |
| [`credentials`](#credentials)         | 1       |                                                                                                                                        |
| [`data`](#data)                       | 2       | Full app-data backup & restore (all persistent stores).                                                                                |
| [`dedup`](#dedup)                     | 1       | Cross-board dedup (ADR-029):                                                                                                           |
| [`dialog`](#dialog)                   | 1       |                                                                                                                                        |
| [`discovery`](#discovery)             | 3       | Discovery namespace (ADR-030 §f):                                                                                                      |
| [`documents`](#documents)             | 9       |                                                                                                                                        |
| [`emailWatch`](#emailwatch)           | 6       |                                                                                                                                        |
| [`extensionBridge`](#extensionbridge) | 9       |                                                                                                                                        |
| [`geocode`](#geocode)                 | 1       |                                                                                                                                        |
| [`github`](#github)                   | 1       |                                                                                                                                        |
| [`jobPreferences`](#jobpreferences)   | 5       |                                                                                                                                        |
| [`jobs`](#jobs)                       | 5       |                                                                                                                                        |
| [`linkedin`](#linkedin)               | 5       |                                                                                                                                        |
| [`match`](#match)                     | 3       |                                                                                                                                        |
| [`menu`](#menu)                       | 3       |                                                                                                                                        |
| [`notifications`](#notifications)     | 9       | Notification Center capability (Phase 2).                                                                                              |
| [`privacy`](#privacy)                 | 5       |                                                                                                                                        |
| [`referrals`](#referrals)             | 3       |                                                                                                                                        |
| [`resume`](#resume)                   | 2       |                                                                                                                                        |
| [`resumePipeline`](#resumepipeline)   | 6       | The staged résumé pipeline — one fixed stage sequence; there is no depth choice.                                                       |
| [`scrape`](#scrape)                   | 9       |                                                                                                                                        |
| [`support`](#support)                 | 1       |                                                                                                                                        |
| [`system`](#system)                   | 16      |                                                                                                                                        |
| [`updater`](#updater)                 | 5       |                                                                                                                                        |

## `ai`

Contract: `AiContract` in `packages/shared/src/ipc/contracts/ai.ts`

### Methods — `ai`

- [`ai.generate`](#aigenerate)
- [`ai.activeConfig`](#aiactiveconfig)
- [`ai.setActiveProvider`](#aisetactiveprovider)
- [`ai.setProviderSettings`](#aisetprovidersettings)
- [`ai.seedActiveConfig`](#aiseedactiveconfig)
- [`ai.stageOverrides`](#aistageoverrides)
- [`ai.setStageOverride`](#aisetstageoverride)
- [`ai.clearStageOverride`](#aiclearstageoverride)
- [`ai.generatePipeline`](#aigeneratepipeline)
- [`ai.onStream`](#aionstream)
- [`ai.listModels`](#ailistmodels)
- [`ai.inspectModel`](#aiinspectmodel)
- [`ai.researchCompany`](#airesearchcompany)
- [`ai.lookupSalary`](#ailookupsalary)
- [`ai.researchAnswer`](#airesearchanswer)
- [`ai.pullModel`](#aipullmodel)
- [`ai.unloadModel`](#aiunloadmodel)
- [`ai.embed`](#aiembed)
- [`ai.setProviderKey`](#aisetproviderkey)
- [`ai.removeProviderKey`](#airemoveproviderkey)
- [`ai.hasProviderKey`](#aihasproviderkey)
- [`ai.testProviderKey`](#aitestproviderkey)
- [`ai.listProviderModels`](#ailistprovidermodels)
- [`ai.modelCapabilities`](#aimodelcapabilities)
- [`ai.embeddingStatus`](#aiembeddingstatus)
- [`ai.setEmbeddingConfig`](#aisetembeddingconfig)
- [`ai.reembedAll`](#aireembedall)
- [`ai.indexStaleDocuments`](#aiindexstaledocuments)
- [`ai.spendSummary`](#aispendsummary)

#### `ai.generate`

```ts
generate(req: AiGenerateRequest): Promise<{ jobId: string }>;
```

Start a generation. Returns as soon as the job is queued; the content
arrives on `onStream`, keyed by the returned `jobId`.

The request shape is `AiGenerateRequestSchema` in
`packages/shared/src/schemas/index.ts` (code-generated into
`apps/desktop/src-tauri/src/ipc_contracts/ai.rs` by `pnpm gen:ipc`). Read
the schema for the field list — it is the source of truth and this doc
deliberately does not restate it.

Two fields whose behaviour does not follow from their types:

- **`intent`** — the caller declares what the generation is _for_; each
  provider adapter then picks its own sampling numbers
  (`AiProvider::sampling_profile` in
  `apps/desktop/src-tauri/src/commands/ai_provider/mod.rs`). The accepted
  values are generated from the schema into `ipc_contracts/ai_intents.rs`,
  so they cannot drift from a list nobody wrote down here.
- **`temperature`** (and the other numeric sampling fields) — an explicit
  **override** that beats `intent` on every adapter. In practice it is only
  ever set by the per-model "Custom temperature" control in Settings; it is
  not a default the app applies.

#### `ai.activeConfig`

```ts
activeConfig(): Promise<ActiveAiConfig>;
```

The backend-owned active AI _generation_ provider config — the single source
of truth for which provider/model/baseUrl generation routes to (task #16).
The renderer reads this (never the request) so it can no longer point
generation at an arbitrary endpoint. `providers` is always present (maybe
empty); `activeProvider`/`model`/`baseUrl` are absent when unseeded.

#### `ai.setActiveProvider`

```ts
setActiveProvider(req: { provider: string }): Promise<ActiveAiConfig | { error: string }>;
```

Switch the active provider (the "switch" half of the switch-vs-edit split).
Returns the fresh active config, or `{ error }` on an invalid id.

#### `ai.setProviderSettings`

```ts
setProviderSettings(req: {
    provider: string;
    model?: string | null;
    baseUrl?: string | null;
    /** 512–131072, validated server-side. Only Ollama acts on it (`num_ctx`). */
    contextWindow?: number | null;
  }): Promise<ActiveAiConfig | { error: string }>;
```

Edit a (possibly non-active) provider's model/base_url/context window
without flipping the active provider (the "edit" half). Returns the fresh
active config, or `{ error }` when server-side validation rejects any of
them — including against fields it did NOT send (the merged row is what is
validated, so patching in a cross-family model still fails).

**PATCH semantics, per field:**

- **omitted** → keep whatever is stored. Send only what changed.
- **`null`** → clear the stored value.
- **a value** → set it.

Replace-everything was the first design and it erased fields at three call
sites before the day was out; absence is what a caller produces by
accident, so absence is the harmless case.

`contextWindow` is the window for the MODEL in this entry, so send it
alongside a model change and let it be `null` when the new model has none.

#### `ai.seedActiveConfig`

```ts
seedActiveConfig(req: {
    config: AiConfigSnapshot;
  }): Promise<{ seeded: boolean } | { error: string }>;
```

One-time first-run seed from the renderer's migrated Zustand config.
Row-presence gated server-side, so re-calls are safe no-ops.

#### `ai.stageOverrides`

```ts
stageOverrides(): Promise<Record<string, AiStageOverride>>;
```

Every explicitly-set per-stage model override, keyed by pipeline stage
name (the `PIPELINE_STAGES` vocabulary from `@ajh/shared`).

**Absent means the active provider.** A stage with no entry is not
"overridden to the default" — it was never configured, and the backend
resolves it through the normal active-config path. Render the difference:
a suggested value must be shown as a suggestion until the user applies it,
because nothing here is applied on the user's behalf.

#### `ai.setStageOverride`

```ts
setStageOverride(req: {
    stage: string;
    provider: string;
    model?: string;
    contextWindow?: number;
  }): Promise<Record<string, AiStageOverride> | { error: string }>;
```

Point ONE stage at a provider + model. Returns the fresh override map, or
`{ error }` when server-side validation rejects the stage name, the
provider, the model (cross-family check) or the context window
(512–131072).

`model` may be empty ONLY for a CLI-agent provider, which runs on its own
configured default.

No base URL: the stage uses the one stored for `provider` — see
`AiStageOverride`. Change it in that provider's settings and every
override on it follows.

#### `ai.clearStageOverride`

```ts
clearStageOverride(req: {
    stage: string;
  }): Promise<Record<string, AiStageOverride> | { error: string }>;
```

Return ONE stage to the active provider. A no-op for a stage that has no
override. Returns the fresh override map.

#### `ai.generatePipeline`

```ts
generatePipeline(req: AiGenerateRequest): Promise<{ jobId: string }>;
```

Stream a generation through the backend orchestration pipeline. Same wire
shape as `generate`, but the work runs as a composable `Pipeline` (so feature
generators share one lifecycle). Used by resume/cover-letter generation.

#### `ai.onStream`

```ts
onStream(handler: (chunk: AiStreamChunk) => void): () => void;
```

#### `ai.listModels`

```ts
listModels(): Promise<Array<{ name: string }>>;
```

#### `ai.inspectModel`

```ts
inspectModel(req: { model: string }): Promise<ModelInspectResult | null>;
```

Inspect a local (Ollama) model's real context window + size via `/api/show`,
to suggest safe generation limits. Returns `null` for non-local providers or
an unreachable Ollama server.

#### `ai.researchCompany`

```ts
researchCompany(req: {
    jobAd: string;
    /** Accurate AI-extracted company name; preferred over heuristic job-ad extraction. */
    company?: string;
    /** Accurate AI-extracted job title; preferred over heuristic job-ad
     *  extraction, whose last resort is the ad's first short line — on a scraped
     *  page routinely an apply button ("Jetzt bewerben") or a nav link. */
    role?: string;
    /** The SAME reasoning-effort value a generation request carries. Sizes the
     *  backend's deadline around search + synthesis — synthesis is a model call,
     *  so its cost scales with the model's reasoning budget. Omit for the
     *  unscaled baseline. */
    effort?: string;
  }): Promise<{ company: string; brief: string }>;
```

Research the company named in a job ad and return a short factual brief —
used by the cover-letter "fit" paragraph and company-specific application
answers. Reuses the shared enricher: the active provider's own web search
(native tool, or the Ollama Web Search API for Ollama), cached. Degrades
gracefully — an empty brief, never an error, when the provider can't search
or the search fails. The brief is reference context only; every prompt that
consumes it fences it as untrusted input (ADR-010).

#### `ai.lookupSalary`

```ts
lookupSalary(req: {
    role: string;
    company?: string;
    location?: string;
    /** ISO-3166 alpha-2 job country, when known — grounds `currency` below. */
    country?: string;
    /** Authoritative ISO-4217 currency for `country` (resolve client-side via
     *  `countryToCurrency`). Pins the researched/reported currency server-side
     *  so a blank/weak `location` can't let the model default to USD or
     *  hallucinate one; omitted when the country is unknown, which preserves
     *  today's unconstrained "local currency for that location" behavior. */
    currency?: string;
    /** See `researchCompany.effort` — the same deadline scaling applies here. */
    effort?: string;
  }): Promise<SalaryRange | null>;
```

Web-grounded market salary-range lookup for the salary application
question. Reuses the active provider's own web search (same channel as
`researchCompany`), parsed and strictly validated server-side, cached.
Degrades gracefully — `null`, never an error, when the provider can't
search, the search yields nothing reliable, or times out. Only validated
integers + a sanitized currency code are ever returned; no raw web text
crosses this boundary.

#### `ai.researchAnswer`

```ts
researchAnswer(req: { question: string; role?: string; company?: string }): Promise<string>;
```

Best-effort, per-question web-search reference notes for an application
answer — opt-in sibling of `researchCompany`, scoped to a single
question's topic (combines it with the role + company for relevance)
rather than a general company brief. Reuses the same backend enricher
channel: the active provider's own web search (native tool, or the Ollama
Web Search API), gated on the provider's actual search support. Degrades
gracefully — an empty string, never an error, when the provider can't
search or the search fails, so answer generation always proceeds exactly
as without web search. The notes are reference context only; the prompt
layer fences them as untrusted and never lets them write the answer.

#### `ai.pullModel`

```ts
pullModel(model: string): Promise<{ jobId: string }>;
```

#### `ai.unloadModel`

```ts
unloadModel(model: string): Promise<void>;
```

#### `ai.embed`

```ts
embed(req: {
    text: string;
    model?: string;
  }): Promise<
    { vector: number[]; dim: number; provider: string; model: string } | { error: string }
  >;
```

Synchronous embedding — returns the vector, or `{ error }` on any
provider/config failure (a context-length overflow, a missing key, an
unreachable host, …).

`model` is part of the wire shape but the handler does not read it: the
active embedding config persisted in the document store always wins
(`ai_embed` → `documents::embed`).

Input longer than the provider's per-chunk limit is split, embedded per
chunk, mean-pooled and L2-normalized into one vector. A context-length
overflow is retried adaptively INSIDE this same call (the chunk is halved
down to a floor, and the working cap is learned once per document) — never
a follow-up request the caller has to issue.

#### `ai.setProviderKey`

```ts
setProviderKey(req: { provider: string; apiKey: string }): Promise<{ success: boolean }>;
```

Store an API key for a cloud AI provider in the OS keychain.

#### `ai.removeProviderKey`

```ts
removeProviderKey(req: { provider: string }): Promise<{ success: boolean }>;
```

Remove a stored provider API key from the OS keychain.

#### `ai.hasProviderKey`

```ts
hasProviderKey(req: { provider: string }): Promise<{ has: boolean }>;
```

Check whether a provider API key is stored (does not return the key).

#### `ai.testProviderKey`

```ts
testProviderKey(req: {
    provider: string;
    baseUrl?: string;
  }): Promise<{ success: boolean; error?: string }>;
```

Test whether a stored provider API key is valid by making a lightweight API call.
`baseUrl` is forwarded for OpenAI-compatible servers (LM Studio, vLLM, etc.).

#### `ai.listProviderModels`

```ts
listProviderModels(req: { provider: string; baseUrl?: string }): Promise<ProviderModelInfo[]>;
```

Fetch available models from a cloud provider using its stored API key.
`baseUrl` is forwarded for OpenAI-compatible servers.

Rejects — never resolves with a partial list — when the stored key is
missing or refused, the provider is unreachable, or the catalogue response
cannot be parsed. There is no cached-list fallback, so verifying a
freshly-entered key in onboarding cannot pass on stale data.

#### `ai.modelCapabilities`

```ts
modelCapabilities(req: {
    provider: string;
    model?: string;
    baseUrl?: string;
  }): Promise<{ supportsWebSearch: boolean; supportsReasoning: boolean; effortLevels: string[] }>;
```

Capability probe for a provider/model. Network-free, but NOT static: it
reads stored credentials to answer `supportsWebSearch` — whether it can
attempt a web-grounded company/role search, whether it accepts a
reasoning-effort value, and (when it does) exactly which levels this MODEL
accepts. Reads the Rust `ModelCapabilities` matrix + `AiProvider::effort_levels`
(the same values the backend gates `research*` and each adapter's own
effort field on), so the renderer never mirrors the per-provider/per-model
vocabulary and a new provider or model needs zero TS change — some
providers' accepted level SET genuinely varies by model tier (Gemini), not
just by provider, which is why this is a per-model lookup rather than a
static per-provider list. Drives the capability-driven default of the
tailoring "search company" toggle, and the Settings → AI effort picker
(which renders exactly the `effortLevels` this returns). Unknown/
unresolvable providers degrade to `supportsWebSearch: false`,
`supportsReasoning: false`, `effortLevels: []`. `baseUrl` is forwarded for
OpenAI-compatible servers.

`supportsWebSearch` is a CONFIGURATION answer, not a capability one: true
when a search backend can actually serve research — the provider's own, or
the configured fallback. A provider that advertises search but has no key
for it reads false, because the brief it would produce is empty.

#### `ai.embeddingStatus`

```ts
embeddingStatus(): Promise<EmbeddingStatus>;
```

Active embedding space, per-space vector counts, and document index coverage.

#### `ai.setEmbeddingConfig`

```ts
setEmbeddingConfig(req: {
    provider: string;
    model?: string;
    baseUrl?: string;
  }): Promise<{ success: boolean; error?: string; config?: EmbeddingConfig }>;
```

Set the active embedding provider/model. An empty model resolves to the
provider's default. Changing it changes the embedding space — call
`reembedAll` afterwards to rebuild the index.

#### `ai.reembedAll`

```ts
reembedAll(): Promise<{ jobId: string }>;
```

Re-embed every document with the active embedding config. Returns a job id.

#### `ai.indexStaleDocuments`

```ts
indexStaleDocuments(): Promise<{ jobId: string | null }>;
```

Index ONLY the documents with no usable vector in the active embedding
space — a newly imported résumé, or everything after the provider/model
changed. Backs the `autoIndexOnUpload` preference.

Not `reembedAll`: that re-embeds every document unconditionally, which would
re-bill a cloud embedding provider for already-indexed documents each time
one new file is added.

`jobId` is `null` when nothing was stale, so the caller can stay silent
rather than show progress for a no-op.

#### `ai.spendSummary`

```ts
spendSummary(): Promise<AiSpendSummary>;
```

Read-only AI-spend summary: today's REAL per-provider token totals — as
reported by each provider's own response, never estimated — plus an
ESTIMATED USD cost from a static list-price rate table. The dollar
figure is a best-effort ballpark (BYO-key users have no billing API to
query), not a billing-accurate source. Local (Ollama) and CLI-agent
calls always cost $0.

### Channels — `ai`

`AI_CHANNELS` in `packages/shared/src/ipc/contracts/ai.ts`:

| Key           | Channel          |
| ------------- | ---------------- |
| `generate`    | `ai:generate`    |
| `listModels`  | `ai:listModels`  |
| `pullModel`   | `ai:pullModel`   |
| `unloadModel` | `ai:unloadModel` |
| `embed`       | `ai:embed`       |

`AI_CHANNELS` registers 5 of this namespace's 29 methods; the rest have no entry in it.

### Types — `ai`

Declared in `packages/shared/src/ipc/contracts/ai.ts`.

```ts
/**
 * One entry in a provider's model catalogue (`listProviderModels`). `name` is
 * the canonical id everything selects on (a stored model preference matches
 * against it) and is always present — every other field is optional because
 * no single provider's models endpoint returns all of them: see
 * `AiProvider::list_models`/`model_entry` in the Rust backend
 * (`apps/desktop/src-tauri/src/commands/ai_provider/mod.rs`) for exactly
 * which fields each provider supplies. A provider that doesn't return a
 * field omits it here entirely — never a fabricated zero/empty-string/
 * "unknown" sentinel; treat absent as absent.
 */
export interface ProviderModelInfo {
  name: string;
  /** Human-readable label, when the provider returns one distinct from `name`. */
  displayName?: string;
  /**
   * Unix epoch MILLISECONDS. Normalized on the Rust side across providers'
   * native wire formats (an RFC3339 string, a unix-epoch-SECONDS integer) —
   * never a provider's raw representation — so this never needs per-provider
   * branching to sort.
   */
  createdAt?: number;
  /** Max input tokens, when the provider's catalogue endpoint reports one. */
  contextLength?: number;
}

export interface EmbeddingConfig {
  provider: string;
  model: string;
  baseUrl?: string | null;
}

/** One provider's backend-persisted generation routing (`base_url` is only
 *  meaningful for `openai-compatible`). Mirrors the Rust `ProviderConfig`. */
export interface AiProviderRouting {
  model?: string;
  baseUrl?: string;
  /**
   * The context window (`num_ctx`) `model` is configured to run with, when the
   * user set one. Belongs to the model in this entry, not to the provider.
   *
   * This is what a STAGED run reads: the backend builds its own requests, so
   * the renderer's per-model limits map cannot reach it. Absent means the
   * provider's own default — never a guessed size.
   */
  contextWindow?: number;
}

/**
 * One pipeline stage's explicitly-chosen routing. Mirrors the Rust
 * `StageOverride`.
 *
 * Provider AND model, not a bare model id: moving the judge to a cloud model
 * while drafting locally is a change of provider, and a model-only shape would
 * have to guess which provider it belonged to. `contextWindow` belongs to the
 * model this entry names.
 *
 * There is deliberately NO `baseUrl`. The endpoint belongs to the PROVIDER, and
 * the backend reads it from that provider's own settings row when the stage
 * resolves — so an override always uses the URL Settings displays, and one
 * cannot be pointed at an endpoint no screen shows.
 */
export interface AiStageOverride {
  provider: string;
  /** Empty only for a CLI-agent provider. */
  model: string;
  contextWindow?: number;
}

/** The persisted snapshot the renderer seeds the backend store from — 1:1 with
 *  the old Zustand `aiProviderConfig` (`{ activeProvider, providers }`). */
export interface AiConfigSnapshot {
  activeProvider?: string;
  providers: Record<string, AiProviderRouting>;
  /** Per-stage overrides. Optional so a pre-override bundle still validates. */
  stageOverrides?: Record<string, AiStageOverride>;
}

/** Backend-owned active generation config (task #16). The active provider's own
 *  resolved `model`/`baseUrl` plus the full `providers` map (for the Settings AI
 *  tab). `activeProvider`/`model`/`baseUrl` are absent when the store is unseeded;
 *  `providers` is always present (maybe empty). Mirrors the Rust `ActiveAiConfig`. */
export interface ActiveAiConfig {
  activeProvider?: string;
  model?: string;
  baseUrl?: string;
  /** The active provider entry's own {@link AiProviderRouting.contextWindow}. */
  contextWindow?: number;
  providers: Record<string, AiProviderRouting>;
}

/** A validated web-researched market salary range (mirrors the Rust
 *  `salary_research::SalaryRange` — min/max/currency only, already validated
 *  server-side before it crosses the IPC boundary). */
export interface SalaryRange {
  min: number;
  max: number;
  currency: string;
}

export interface EmbeddingSpaceInfo {
  provider: string;
  model: string;
  dim: number;
  count: number;
  active: boolean;
}

export interface EmbeddingStatus {
  active: EmbeddingConfig;
  spaces: EmbeddingSpaceInfo[];
  documents: { total: number; indexedInActiveSpace: number; stale: number };
  /**
   * An embedding job (auto-index or manual re-index) is running right now.
   *
   * The backend's own answer, so the UI never has to infer it — deducing
   * "indexing now" from the auto-index preference claims work is happening even
   * when the run already failed or was never started. Only one embedding job
   * runs at a time; a second trigger joins the running one.
   */
  indexing: boolean;
}

/** One provider's real token totals + estimated cost, since the start of the
 *  current UTC day. */
export interface AiSpendProviderTotals {
  provider: string;
  inputTokens: number;
  outputTokens: number;
  estCostUsd: number;
}

/**
 * One model's OBSERVED reasoning overhead, over ALL history (not just today —
 * "how much does this model think" does not reset at midnight).
 *
 * Only calls whose provider reported a DISTINCT reasoning-token count take
 * part, so `outputTokens` is the denominator over exactly those calls and
 * `thinkingTokens / outputTokens` is a like-for-like ratio. `calls` is the
 * sample size: treat a ratio from one or two calls as noise.
 *
 * A model that never appears is not a model that does no reasoning — it is one
 * whose provider does not report the split. Anthropic counts thinking inside
 * its output tokens and Ollama's `eval_count` includes the thinking channel, so
 * neither ever appears here. OpenAI and Gemini do report it — note that a
 * current OpenAI model that did no reasoning reports a measured `0` rather than
 * omitting the field, so it CAN appear here with a zero ratio, which is a real
 * observation ("this model does not think") and not the same as absence.
 * Render absence as "not measured", never as zero.
 */
export interface AiSpendModelThinking {
  provider: string;
  model: string;
  calls: number;
  thinkingTokens: number;
  outputTokens: number;
}

/** Today's real AI-spend totals, overall and per provider, plus the all-history
 *  per-model reasoning overhead where it was actually measured. */
export interface AiSpendSummary {
  today: { inputTokens: number; outputTokens: number; estCostUsd: number };
  perProvider: AiSpendProviderTotals[];
  /** Empty until a provider that reports the split has been used — see
   *  {@link AiSpendModelThinking}. */
  thinkingByModel: AiSpendModelThinking[];
}
```

### Referenced types — `ai`

- `packages/shared/src/schemas/index.ts` — `AiGenerateRequest`, `ModelInspectResult`
- `packages/shared/src/types/index.ts` — `AiStreamChunk`

---

## `aiGenerations`

Contract: `AiGenerationsContract` in `packages/shared/src/ipc/contracts/aiGenerations.ts`

### Methods — `aiGenerations`

- [`aiGenerations.list`](#aigenerationslist)
- [`aiGenerations.save`](#aigenerationssave)
- [`aiGenerations.update`](#aigenerationsupdate)
- [`aiGenerations.remove`](#aigenerationsremove)
- [`aiGenerations.removeBulk`](#aigenerationsremovebulk)

#### `aiGenerations.list`

```ts
list(): Promise<AiGenerationRecord[]>;
```

#### `aiGenerations.save`

```ts
save(req: AiGenerationSaveRequest): Promise<AiGenerationSaveResult>;
```

Per-job merge-upsert keyed on `jobUrl` (`merge_application` in
`apps/desktop/src-tauri/src/ai_generations/mod.rs`): a résumé, a cover
letter, application answers and a company brief produced by separate
generation actions all land on ONE row when they share a `jobUrl`. A save
with no `jobUrl` (a manual generation) inserts its own row instead.

#### `aiGenerations.update`

```ts
update(req: AiGenerationUpdateRequest): Promise<void>;
```

#### `aiGenerations.remove`

```ts
remove(id: string): Promise<void>;
```

#### `aiGenerations.removeBulk`

```ts
removeBulk(ids: string[]): Promise<void>;
```

### Channels — `aiGenerations`

`AI_GENERATIONS_CHANNELS` in `packages/shared/src/ipc/contracts/aiGenerations.ts`:

| Key          | Channel                    |
| ------------ | -------------------------- |
| `list`       | `aiGenerations:list`       |
| `save`       | `aiGenerations:save`       |
| `update`     | `aiGenerations:update`     |
| `remove`     | `aiGenerations:remove`     |
| `removeBulk` | `aiGenerations:removeBulk` |

### Types — `aiGenerations`

Declared in `packages/shared/src/ipc/contracts/aiGenerations.ts`.

```ts
export interface AiGenerationRecord {
  id: string;
  createdAt: number;
  candidateName: string;
  jobTitle: string;
  companyName: string;
  resumeLanguage: string;
  jobAdLanguage: string;
  targetLanguage: string;
  mismatch: boolean;
  topRequirements: string[];
  mode: string;
  resumeText: string;
  coverLetterText: string;
  jobAd: string;
  /** The job this generation targets — links the record to an autopilot found job. */
  jobUrl: string;
  /** The board the job came from (e.g. "linkedin"). */
  board: string;
  /** Answered application questions (the questions assistant), if any. */
  applicationAnswers: ApplicationAnswer[];
  /** The company-research brief used for this application, if any. */
  companyBrief: string;
  /** AI-suggested questions the candidate can ASK the interviewer, if any. */
  interviewQuestions: InterviewQuestion[];
  /**
   * The persisted apply-by-email draft (subject line + body). Optional because
   * records serialised before these columns existed — e.g. an older exported
   * backup replayed through a test fixture — carry neither field.
   */
  emailSubject?: string;
  emailBody?: string;
  /**
   * Parent Application FK — set at save time (and backfilled at boot for legacy
   * rows). The Application detail page joins this generation's docs by this id, not
   * by url, because the Application stores the NORMALIZED url and the generation the
   * RAW one (they never match for query-id boards like Indeed). Absent when unlinked.
   */
  applicationId?: string;
  /**
   * Serialized JSON wrapper `{schemaVersion, pipeline, generatedAt, resume?,
   * coverLetter?}` (this shape is renderer-owned) holding the deterministic
   * content-quality report(s). Each per-document key holds a SLOT —
   * `{report, sourceTextHash}`: `validate::content::ContentReport` plus the
   * hash of the exact text it validated, so the renderer can flag that
   * document stale against the current résumé/letter text. The hash lives
   * inside the slot precisely because the merge below is per TOP-LEVEL key: a
   * sibling hash map would be replaced wholesale by a single-document save,
   * orphaning the other document's anchor. The Rust store never clears a
   * report on a text edit, so staleness display is entirely a renderer-side,
   * read-time decision.
   *
   * Always present on a record returned from `list`/`save` (possibly `''` = no
   * report yet, or the row predates this field) — unlike on
   * {@link AiGenerationSaveRequest.qualityReport}, where it is genuinely
   * optional (omit to leave whatever report is already on the aggregate). A
   * save MERGES its incoming wrapper onto the existing one per TOP-LEVEL key:
   * a letter-only save overlays only `coverLetter` (plus the envelope fields)
   * and leaves a stored `resume` sub-report untouched, and vice versa. See
   * ADR-007 addendum — a manual text edit via {@link AiGenerationUpdateRequest}
   * deliberately never clears this.
   */
  qualityReport?: string;
}

export interface AiGenerationSaveRequest {
  candidateName: string;
  jobTitle: string;
  companyName: string;
  resumeLanguage: string;
  jobAdLanguage: string;
  targetLanguage: string;
  mismatch: boolean;
  topRequirements: string[];
  mode: string;
  resumeText: string;
  coverLetterText: string;
  jobAd: string;
  /** The job this generation targets (marks the autopilot found job "applied"). */
  jobUrl?: string;
  /** The board the job came from. */
  board?: string;
  /** Answered application questions to persist on the (per-job) record. */
  applicationAnswers?: ApplicationAnswer[];
  /** The company-research brief used, persisted for audit. */
  companyBrief?: string;
  /** AI-suggested interview questions to persist on the (per-job) record. */
  interviewQuestions?: InterviewQuestion[];
  /**
   * The apply-by-email draft to persist on the (per-job) record. Merged like
   * `coverLetterText`: a non-blank value overwrites the stored draft, a blank
   * one leaves it untouched — so a résumé/answers save can't wipe the email.
   */
  emailSubject?: string;
  emailBody?: string;
  /**
   * Deterministic content-quality report wrapper to merge onto the aggregate
   * (see {@link AiGenerationRecord.qualityReport} for the shape and the
   * per-key merge rule). Renderer's job to compute it (typically right after a
   * resume/cover regeneration) and pass it here; an absent/empty value merges
   * nothing, leaving whatever report is already on the aggregate untouched.
   */
  qualityReport?: string;
}

/**
 * Edit the résumé/cover-letter text of an existing saved generation, selected by
 * `id`. Unlike {@link AiGenerationSaveRequest} (a per-job merge-upsert that keeps
 * existing non-empty text), this is a direct overwrite — so a user editing a
 * saved generation can blank out or fully replace the text. Each text field is
 * optional; an absent field is left unchanged.
 */
export interface AiGenerationUpdateRequest {
  id: string;
  resumeText?: string;
  coverLetterText?: string;
}

/**
 * Result of `save` — the Rust command reports failure IN-BAND (it resolves with
 * `{ error }` instead of rejecting), so a caller's `onError` never fires for a
 * store failure. Modelled as a union of the two disjoint arms the command
 * actually returns, which makes the compiler refuse a bare `result.id` until
 * the failure arm has been narrowed out (`'error' in result`) — the check is
 * otherwise trivially forgotten, which is exactly how it was missed here.
 */
export type AiGenerationSaveResult = { id: string; success: true } | { error: string };
```

### Referenced types — `aiGenerations`

- `packages/shared/src/types/index.ts` — `ApplicationAnswer`, `InterviewQuestion`

---

## `applications`

Application-tracking capability (ADR 0001). The Generate trigger lives in the
`aiGenerations.save` flow (it upserts the Application as a side-effect); the two
creation triggers here are the doc-less ones: `track` (manual, → `applied`) and
`saveFromPosting` (Jobs-page Save, → `saved`).

## One contact per application

`contactName`/`contactEmail` are **canonical** — the single primary contact
(recruiter / hiring manager / apply-by-email recipient). `recipientName`/
`recipientEmail` are **deprecated aliases** of them:

| Direction                                | Behaviour                                               |
| ---------------------------------------- | ------------------------------------------------------- |
| `update({ contactName })`                | writes the canonical field                              |
| `update({ recipientName })`              | writes the SAME canonical field                         |
| `update({ contactName, recipientName })` | canonical wins; the alias is ignored                    |
| response (`list`/`get`)                  | `recipientName === contactName` always (same for email) |

Both email names go through the same server-side address validation, and an
invalid one rejects the whole patch with `{ error }`. New UI should read and
write `contactName`/`contactEmail` only.

## Follow-up reminders

`nextActionAt` (epoch ms, nullable) is the reminder. A backend sweep raises a
notification (`kind: 'application.follow_up'`, route `/applications` with
`search.highlight = <id>`) once per due date for non-terminal applications;
moving or clearing `nextActionAt` re-arms it. `nextActionNotifiedAt` is the
read-only dedupe marker behind that "once" — it exists on the wire only so a
backup round trip does not re-announce delivered reminders, and `update()`
deliberately has no field for it. There is **no** counts command:
overdue/upcoming badges are derived client-side from the `nextActionAt` values
already carried by `list()` (see `features/applications/lib/pipeline.ts`).

## Email-derived status adjudication (v2)

`get()`'s `events` now also carry `StatusEvent.source`/
`StatusEvent.confirmed`. A `source: 'email'` row with `confirmed: false`
is a provisional, auto-written transition (see the `email.match` notification)
that the timeline must render distinctly, with Accept/Reject affordances:

- `acceptStatusEvent` clears `confirmed` in place — the status itself is
  untouched (the auto-write already applied it).
- `rejectStatusEvent` reverts the status BY COMPARE-AND-SET (a status the user
  changed by hand in the meantime is never clobbered — the row is simply
  marked reviewed instead) and APPENDS a reversal event
  (`source: 'email_reject'`); `status_events` stays append-only, so the
  original row is never edited or deleted.

**Both require `StatusEvent.eventId` — the id of the SPECIFIC row
being actioned, not just the application id.** Two provisional rows can
coexist (a confirmation email, then a later rejection email, both still
unreviewed); always pass the `eventId` of the exact row the Accept/Reject
affordance was rendered on. Both are idempotent no-ops (`{ success: true
}`, nothing changed) when `eventId` does not resolve to a pending
unconfirmed row for `id` — never an error a UI needs to branch on.
**Nothing in this app ever writes `confirmed: true` except these two calls
clearing it on review** — adjudication is the entire safety model for a
classifier with a recorded precision limit (see `docs/knowledge/
decision-records/0013-email-confirmation-watching.md`).

Contract: `ApplicationsContract` in `packages/shared/src/ipc/contracts/applications.ts`

### Methods — `applications`

- [`applications.list`](#applicationslist)
- [`applications.get`](#applicationsget)
- [`applications.setStatus`](#applicationssetstatus)
- [`applications.acceptStatusEvent`](#applicationsacceptstatusevent)
- [`applications.rejectStatusEvent`](#applicationsrejectstatusevent)
- [`applications.update`](#applicationsupdate)
- [`applications.remove`](#applicationsremove)
- [`applications.track`](#applicationstrack)
- [`applications.saveFromPosting`](#applicationssavefromposting)
- [`applications.onChanged`](#applicationsonchanged)

#### `applications.list`

```ts
list(): Promise<Application[]>;
```

#### `applications.get`

```ts
get(id: string): Promise<ApplicationDetail>;
```

#### `applications.setStatus`

```ts
setStatus(args: {
    id: string;
    status: string;
    note?: string;
  }): Promise<ApplicationMutationResult>;
```

Transition the status, optionally recording a free-text `note` — persisted
on the appended `status_events` row and returned as `StatusEvent.note` by
`get()` (the interaction log).

#### `applications.acceptStatusEvent`

```ts
acceptStatusEvent(args: { id: string; eventId: number }): Promise<ApplicationMutationResult>;
```

Accept the SPECIFIC email-derived, unconfirmed status-event row
`eventId` names — clears its `StatusEvent.confirmed` flag; the
status itself is untouched. `eventId` must be the
`StatusEvent.eventId` of the exact row the Accept affordance was
rendered on — see `StatusEvent.eventId`'s doc for why "the most
recent pending row" is not a safe substitute. A no-op when `eventId`
does not resolve to a pending row for `id` (still `{ success: true }`,
not an error).

#### `applications.rejectStatusEvent`

```ts
rejectStatusEvent(args: { id: string; eventId: number }): Promise<ApplicationMutationResult>;
```

Reject the SPECIFIC email-derived, unconfirmed status-event row
`eventId` names — reverts the status by compare-and-set (never clobbers
a status that moved on, whether by the user's own hand or a later
email, in the meantime) and appends a reversal event. Same
`eventId`-targeting requirement as `acceptStatusEvent`. A no-op
when `eventId` does not resolve to a pending row.

#### `applications.update`

```ts
update(req: ApplicationUpdateRequest): Promise<ApplicationMutationResult>;
```

#### `applications.remove`

```ts
remove(args: { id: string; keepDocuments: boolean }): Promise<ApplicationMutationResult>;
```

#### `applications.track`

```ts
track(req: ApplicationTrackRequest): Promise<ApplicationCreateResult>;
```

#### `applications.saveFromPosting`

```ts
saveFromPosting(req: ApplicationTrackRequest): Promise<ApplicationCreateResult>;
```

#### `applications.onChanged`

```ts
onChanged(handler: (event: ApplicationChangedEvent) => void): () => void;
```

Subscribe to out-of-band application changes (e.g. browser-extension imports).
Returns a sync unsubscribe handle.

### Channels — `applications`

`APPLICATIONS_CHANNELS` in `packages/shared/src/ipc/contracts/applications.ts`:

| Key                 | Channel                          |
| ------------------- | -------------------------------- |
| `list`              | `applications:list`              |
| `get`               | `applications:get`               |
| `setStatus`         | `applications:setStatus`         |
| `acceptStatusEvent` | `applications:acceptStatusEvent` |
| `rejectStatusEvent` | `applications:rejectStatusEvent` |
| `update`            | `applications:update`            |
| `remove`            | `applications:remove`            |
| `track`             | `applications:track`             |
| `saveFromPosting`   | `applications:saveFromPosting`   |

`APPLICATIONS_CHANNELS` registers 9 of this namespace's 10 methods; the rest have no entry in it.

### Types — `applications`

Declared in `packages/shared/src/ipc/contracts/applications.ts`.

```ts
/** The detail payload for one Application: the aggregate plus its status history. */
export interface ApplicationDetail {
  application: Application | null;
  events: StatusEvent[];
}

/** Result of a mutating command (matches the Rust `{ success } | { error }` shape). */
export interface ApplicationMutationResult {
  success?: boolean;
  error?: string;
}

/** Result of a create command. */
export interface ApplicationCreateResult {
  id?: string;
  success?: boolean;
  error?: string;
}

/**
 * Event payload emitted when an Application is created/changed out-of-band — e.g.
 * a job imported via the browser-extension bridge. Carries the affected id so
 * consumers can refresh the applications (and postings) lists live, plus a
 * best-effort title/company/status so a live toast can name the job without a
 * refetch race. The descriptive fields are OPTIONAL — an older emitter (or a
 * non-import change) may send only `applicationId`.
 */
export interface ApplicationChangedEvent {
  applicationId: string;
  /** Parsed job title, for a live notification ("Imported '<title>'"). */
  title?: string;
  /** Parsed company name, shown alongside the title. */
  company?: string;
  /** Resulting status id (e.g. `saved`, `applied`). */
  status?: string;
}
```

### Referenced types — `applications`

- `packages/shared/src/schemas/index.ts` — `ApplicationTrackRequest`, `ApplicationUpdateRequest`
- `packages/shared/src/types/index.ts` — `Application`, `StatusEvent`

---

## `autopilot`

Job-discovery agent: saved searches that run on a schedule, then rank and
surface the matching jobs. It never submits anything — auto-apply was
removed, so a stored cover letter is a reusable starting point for the apply
assistant, and the opt-in `assistant` notes are read-only enrichment. The
user tailors and applies by hand.

Contract: `AutopilotContract` in `packages/shared/src/ipc/contracts/autopilot.ts`

### Methods — `autopilot`

- [`autopilot.list`](#autopilotlist)
- [`autopilot.get`](#autopilotget)
- [`autopilot.create`](#autopilotcreate)
- [`autopilot.update`](#autopilotupdate)
- [`autopilot.remove`](#autopilotremove)
- [`autopilot.run`](#autopilotrun)
- [`autopilot.pause`](#autopilotpause)
- [`autopilot.resume`](#autopilotresume)
- [`autopilot.onStep`](#autopilotonstep)
- [`autopilot.onFocus`](#autopilotonfocus)
- [`autopilot.takePendingFocus`](#autopilottakependingfocus)

#### `autopilot.list`

```ts
list(): Promise<Autopilot[]>;
```

#### `autopilot.get`

```ts
get(req: { autopilotId: string }): Promise<Autopilot | null>;
```

#### `autopilot.create`

```ts
create(req: AutopilotCreate): Promise<Autopilot>;
```

#### `autopilot.update`

```ts
update(req: { autopilotId: string } & AutopilotUpdate): Promise<Autopilot>;
```

#### `autopilot.remove`

```ts
remove(req: { autopilotId: string }): Promise<void>;
```

#### `autopilot.run`

```ts
run(req: { autopilotId: string }): Promise<{
    jobId?: string;
    error?: string;
    status?: AutopilotRunStatus;
    skipped?: 'already-running';
  }>;
```

Run an autopilot now. The backend command _resolves_ (does not reject) with
an `{ error }` payload on a scrape failure or unknown id, so callers MUST
inspect `error` — a resolved value is not proof of success. `jobId` is
present on every non-error outcome (success / cancel).

`status` mirrors the outcome persisted on the record (`completed` /
`completedWithErrors` / `failed`) on a run that reached the record site, so
a caller can tell a run that found real jobs from one where every board
failed WITHOUT re-fetching the record. Absent on the early `{ error }` and
`{ cancelled }` outcomes.

`skipped: 'already-running'` is the concurrent-run guard's early return: a
double-invoke of the SAME autopilot (a scheduler retry racing a fresh
occurrence, or two manual triggers) is de-duplicated rather than run twice.
No `jobId`/`error`/`status` accompanies it — no run happened for this call.

#### `autopilot.pause`

```ts
pause(req: { autopilotId: string }): Promise<void>;
```

#### `autopilot.resume`

```ts
resume(req: { autopilotId: string }): Promise<void>;
```

#### `autopilot.onStep`

```ts
onStep(handler: (event: AutopilotStepEvent) => void): () => void;
```

#### `autopilot.onFocus`

```ts
onFocus(handler: (event: AutopilotFocusEvent) => void): () => void;
```

Fired by the shell (tray "New jobs" click or a validated deep link) to
focus an autopilot's found-jobs panel. An empty `autopilotId` is a pure
"refresh the list" signal (e.g. after a tray Pause-All) with no navigation.

#### `autopilot.takePendingFocus`

```ts
takePendingFocus(): Promise<string | null>;
```

Atomically take + clear the autopilot-focus intent buffered by the shell.
A cold-start `ajh://autopilot/<id>` deep link fires the `autopilot:focus`
emit during Rust setup, before the renderer's `useAutopilotFocusNavigation`
listener attaches, so the event is lost; the shell buffers the id and the
renderer pulls it once its JS loop is live (on mount + on the emitted
event). The IPC response is reliable where the event was not. Resolves to
the buffered `autopilotId`, or `null` when nothing is buffered (the common
case — only set by a cold-start deep link). Mirrors `menu.takePending`.

### Channels — `autopilot`

`AUTOPILOT_CHANNELS` in `packages/shared/src/ipc/contracts/autopilot.ts`:

| Key      | Channel            |
| -------- | ------------------ |
| `list`   | `autopilot:list`   |
| `get`    | `autopilot:get`    |
| `create` | `autopilot:create` |
| `update` | `autopilot:update` |
| `remove` | `autopilot:remove` |
| `run`    | `autopilot:run`    |
| `pause`  | `autopilot:pause`  |
| `resume` | `autopilot:resume` |

`AUTOPILOT_CHANNELS` registers 8 of this namespace's 11 methods; the rest have no entry in it.

### Types — `autopilot`

Declared in `packages/shared/src/ipc/contracts/autopilot.ts`.

```ts
export interface AutopilotStepEvent {
  jobId: string;
  autopilotId: string;
  step: string;
  detail: string;
}

export interface AutopilotFocusEvent {
  autopilotId: string;
}
```

### Referenced types — `autopilot`

- `packages/shared/src/schemas/index.ts` — `AutopilotCreate`, `AutopilotUpdate`
- `packages/shared/src/types/index.ts` — `Autopilot`, `AutopilotRunStatus`

---

## `boards`

Contract: `BoardsContract` in `packages/shared/src/ipc/contracts/boards.ts`

### Methods — `boards`

- [`boards.catalog`](#boardscatalog)
- [`boards.health`](#boardshealth)
- [`boards.connect`](#boardsconnect)
- [`boards.disconnect`](#boardsdisconnect)
- [`boards.getStatus`](#boardsgetstatus)
- [`boards.importCookies`](#boardsimportcookies)

#### `boards.catalog`

```ts
catalog(): Promise<BoardCatalogEntry[]>;
```

Full scraper catalog (id, label, mode, auth tier, listed) from the
registry — `SCRAPERS` in
`apps/desktop/src-tauri/src/scraping/boards/mod.rs`, whose per-scraper
`Scraper` impl owns the auth tier and the listed flag.

The canonical id list is `BOARD_IDS` in
`packages/shared/src/schemas/index.ts`; it is not restated here, because
every copy of it has gone stale. `indeed`, `stepstone`, `xing`, `workday`
and `glassdoor` are no longer direct scrapers (ADR-026) — their postings
now arrive through the `aggregator` board.

#### `boards.health`

```ts
health(): Promise<BoardHealthEntry[]>;
```

Live per-board reliability across runs (Track B1). Boards with no recorded
history are simply absent.

#### `boards.connect`

```ts
connect(req: { boardId: string }): Promise<{ connected: boolean; accountEmail?: string }>;
```

Connect to a board by launching a browser for manual login.

#### `boards.disconnect`

```ts
disconnect(req: { boardId: string }): Promise<void>;
```

Disconnect a board (closes context only; does not delete profile).

#### `boards.getStatus`

```ts
getStatus(req: {
    boardId: string;
  }): Promise<{ connected: boolean; accountEmail?: string; lastConnected?: number }>;
```

Get current connection status for a board.

#### `boards.importCookies`

```ts
importCookies(req: { boardId: string }): Promise<CookieImportResult>;
```

Try to import session cookies from the user's installed Chromium browsers
(Chrome, Edge, Brave), so the user can skip the in-app re-login.

Writes the SAME artifacts the in-app browser login produces, so nothing
downstream changes. Best-effort by design and never a regression: a missing
browser, a locked profile or a store this cannot decrypt all resolve as
non-error outcomes (see `CookieImportOutcome`), not as failures. Which
cookie encryption versions are covered, and why, is documented at the
implementation: `apps/desktop/src-tauri/src/scraping/board_login/import.rs`.

### Channels — `boards`

`BOARDS_CHANNELS` in `packages/shared/src/ipc/contracts/boards.ts`:

| Key             | Channel                |
| --------------- | ---------------------- |
| `catalog`       | `boards:catalog`       |
| `health`        | `boards:health`        |
| `connect`       | `boards:connect`       |
| `disconnect`    | `boards:disconnect`    |
| `getStatus`     | `boards:getStatus`     |
| `importCookies` | `boards:importCookies` |

### Types — `boards`

Declared in `packages/shared/src/ipc/contracts/boards.ts`.

```ts
export type CookieImportOutcome =
  'Imported' | 'NoSession' | 'Undecryptable' | 'BrowserNotFound' | 'Error';

export interface CookieImportResult {
  outcome: CookieImportOutcome;
  imported: number;
}

/** Login requirement for a board, sourced from the Rust scraper registry. */
export type BoardAuthRequirement = 'guest' | 'optional' | 'required';

/** One board in the scraper catalog — the source of truth for the jobs picker. */
export interface BoardCatalogEntry {
  id: string;
  displayName: string;
  mode: string;
  auth: BoardAuthRequirement;
  /** Whether the board appears in the manual jobs picker. */
  listed: boolean;
  /**
   * Whether this board requires a company slug to return any results.
   * ATS platforms (Greenhouse, Lever, Ashby, Recruitee, Personio,
   * SmartRecruiters) set this to true. When true, the UI should show a company
   * input field and the engine will skip the board with `skipped: "needs-company"`
   * if no companies are supplied.
   */
  requiresCompany: boolean;
  /**
   * Whether the board narrows results by the requested location server-side.
   * When `false`, the engine conservatively post-filters this board's results
   * against the requested location (dropping only clear city mismatches; never
   * remote/unknown-location rows), so the picker can indicate which boards will
   * genuinely honor a location. Optional so older/absent payloads read as `false`.
   */
  supportsLocation?: boolean;
  /**
   * Curated companies this company-scoped ATS board will query when the user
   * supplies none; empty/absent for boards without a seed.
   */
  seededCompanies?: string[];
}

/**
 * Per-board outcome from a completed scrape job.
 * - `skipped: "needs-login"` — board bypassed because no session exists.
 * - `skipped: "needs-company"` — ATS board bypassed because no company slug was supplied.
 * - `skipped: "needs-keys"` — key-backed board bypassed because its API keys
 *   aren't configured; prompt the user to add them in Settings. No board emits
 *   this today: the aggregator was the only one, and its keyless freehire tier
 *   can answer without any key, so it never asks to be skipped.
 * - `truncated` — a paginated board kept a partial harvest after a mid-run page
 *   failure (e.g. `"page 3 of 5 failed: HTTP 429"`); `count` is a partial tally,
 *   not the full result set. Absent when the harvest ran to completion.
 * - `note` — an INFORMATIONAL location policy the board applied that the user did
 *   not explicitly request (not a failure; `count` is still authoritative). One of:
 *   - `"guessed-market:<cc>"` — no country was supplied, so the `<cc>` market was
 *     guessed and returned an authoritative result set; set a country for
 *     deterministic results.
 *   - `"broadened:<cc>"` — a sparse city search was widened country-wide within
 *     the `<cc>` market.
 *   - `"location-filtered:<n>"` — this board doesn't honor location server-side
 *     (`supportsLocation: false`), so the engine conservatively dropped `<n>`
 *     of its results whose own location clearly mismatched the request; never
 *     drops remote/unknown-location rows.
 *   - `"slugs-invalid:<n>"` — a company-slug ATS board rejected `<n>` of the
 *     supplied slugs pre-fetch (malformed company names) but still returned
 *     results from the valid ones. If EVERY slug was rejected it's an `error`,
 *     not a note.
 *   - `"rows-dropped:<n>"` — a company-slug ATS board dropped `<n>` individual
 *     response rows that failed per-row parsing (schema drift on those rows)
 *     while the rest parsed. If EVERY row of a company dropped it's counted as
 *     a fetch failure, not a note. At most one of `slugs-invalid`/`rows-dropped`
 *     is emitted per board per run (`slugs-invalid` wins when both apply).
 *   - `"companies-failed:<n>"` — a company-slug ATS board (Lever, Ashby) could
 *     not fetch `<n>` of the requested companies (404 on a rotted slug, 403,
 *     429, or a payload over the byte cap) while at least one other company
 *     succeeded, so the run still returned results. If EVERY fetch failed it's
 *     an `error`, not a note. Emitted independently of the
 *     `slugs-invalid`/`rows-dropped` pair above — those come from boards that
 *     validate slugs pre-fetch, this one from the boards that don't — but a
 *     board still reports at most ONE note per run overall.
 *   `<cc>` is an ISO country code; the field never carries the raw location text.
 */
/**
 * Verdict of a board's cross-run history (Track B1). Mirrors the Rust
 * `scraping::board_health::BoardHealthStatus`.
 *   - `unknown` — never actually contacted (only ever skipped).
 *   - `healthy` — the last run that contacted it succeeded, recently.
 *   - `failing` — it is in a failure streak.
 *   - `stale`   — not failing, but its last confirmed success is over a
 *     fortnight old (in practice: skipped ever since).
 *   - `flaky`   — working right now, but failing a meaningful share of its
 *     recent verified runs (`failedRuns` / `verifiedRuns`) — the alternating
 *     ok/fail pattern a consecutive-failure streak can't see.
 */
export type BoardHealthStatus = 'unknown' | 'healthy' | 'failing' | 'stale' | 'flaky';

/**
 * One board's reliability across runs, derived in Rust from every previous
 * run's `BoardScrapeSummary` and attached to the current one — what lets the UI
 * tell "this board found nothing today" apart from "this board has been broken
 * since Tuesday".
 *
 * Only an UNHEALTHY board carries this (see `BoardHealth::is_noteworthy`): a
 * healthy board's chip is unchanged. Timestamps are epoch-ms.
 */
export interface BoardHealth {
  status: BoardHealthStatus;
  /** Length of the current failure streak. Skipped runs neither extend nor
   *  break it — a skip is not a failure. */
  consecutiveFailures: number;
  /** Last run that returned results (or an empty-but-successful answer).
   *  Absent = the board has never succeeded. */
  lastSuccessAt?: number;
  /** Last run that contacted the board at all (success OR error). Absent =
   *  only ever skipped, so nothing has been verified. */
  lastVerifiedAt?: number;
  /** Start of the CURRENT failure streak — the "broken since" timestamp. */
  failingSince?: number;
  /** Why the streak is failing, capped in Rust. Still sanitized at display
   *  time like every other persisted reason. */
  lastError?: string;
  /** The scrape `jobId` of the run that produced this state — the per-board
   *  correlation id for the logs. Only ever set by a run that actually
   *  contacted the board, so a skipped run never claims authorship. */
  lastRunId?: string;
  /** Runs that actually contacted the board (ok + error) within a decayed
   *  rolling window (Rust bounds it, not the board's entire history); skips
   *  are excluded because they verify nothing. */
  verifiedRuns: number;
  /** Failures among those windowed runs. `failedRuns / verifiedRuns` is the
   *  flapping signal a consecutive-failure counter structurally cannot express. */
  failedRuns: number;
}

/** One board's live verdict, as returned by `boards_health`. */
export interface BoardHealthEntry {
  board: string;
  health: BoardHealth;
}

export interface BoardScrapeSummary {
  board: string;
  count: number;
  error?: string;
  skipped?: 'needs-login' | 'needs-company' | 'needs-keys';
  truncated?: string;
  note?: string;
  /** Cross-run reliability, present only when the board is unhealthy AND this
   *  is a LIVE scrape response. Never persisted (it is cross-run state, not part
   *  of this run) — a stored run record carries none, and the Autopilot card
   *  reads the current verdict from `boards_health` instead. */
  health?: BoardHealth;
}
```

---

## `cliAgents`

Contract: `CliAgentsContract` in `packages/shared/src/ipc/contracts/cliAgents.ts`

### Methods — `cliAgents`

- [`cliAgents.status`](#cliagentsstatus)
- [`cliAgents.redetect`](#cliagentsredetect)
- [`cliAgents.install`](#cliagentsinstall)

#### `cliAgents.status`

```ts
status(): Promise<CliAgentsStatus>;
```

Cached install status for every CLI agent (+ npm availability).

#### `cliAgents.redetect`

```ts
redetect(): Promise<CliAgentsStatus>;
```

Clear the detection cache and re-probe (call after an install).

#### `cliAgents.install`

```ts
install(opts: {
    commandName: string;
    args: string[];
    onOutput?: (line: string) => void;
    signal?: AbortSignal;
  }): Promise<CliAgentInstallResult>;
```

One-click install: spawn the capability-allowlisted command (fixed args) and
stream its output. Implemented over the shell plugin in the adapter — the
caller can't tell it isn't a plain IPC command. `commandName`/`args` come
verbatim from `CliAgentStatus`; the shell capability rejects anything
not in the static allowlist.

### Channels — `cliAgents`

`CLI_AGENTS_CHANNELS` in `packages/shared/src/ipc/contracts/cliAgents.ts`:

| Key        | Channel              |
| ---------- | -------------------- |
| `status`   | `cliAgents:status`   |
| `redetect` | `cliAgents:redetect` |

`CLI_AGENTS_CHANNELS` registers 2 of this namespace's 3 methods; the rest have no entry in it.

### Types — `cliAgents`

Declared in `packages/shared/src/ipc/contracts/cliAgents.ts`.

```ts
/** Per-agent install status for the Settings → AI "CLI agents" panel (#22). */
export interface CliAgentStatus {
  /** Provider id (`claude-code` | `codex` | `gemini-cli`). */
  id: string;
  /** Binary looked up on PATH (e.g. `claude`). */
  binary: string;
  installed: boolean;
  version: string | null;
  /** npm package that provides the binary (shown in the guide). */
  package: string;
  /** Official install/setup docs, opened by the guide path. */
  docsUrl: string;
  /** Shell-capability command name for the one-click install (`install-<id>`). */
  installCommandName: string;
  /** Exact args to pass `install` — must match the capability allowlist entry. */
  installArgs: string[];
}

export interface CliAgentsStatus {
  agents: CliAgentStatus[];
  /** `npm` on PATH — gates the one-click install (the guide always shows). */
  npmAvailable: boolean;
}

/** Result of a one-click install spawn. */
export interface CliAgentInstallResult {
  /** Process exit code (`null` if killed). `0` = success. */
  code: number | null;
  success: boolean;
}
```

---

## `contactProfile`

The candidate's stored contact fields (name, email, phone, location, LinkedIn,
GitHub, website, custom links), localized per language.

It seeds the header of every generated document. It does not police it: at
export time the document text owns the résumé header, and the profile is the
fallback for an empty one (ADR-0021).

Contract: `ContactProfileContract` in `packages/shared/src/ipc/contracts/contactProfile.ts`

### Methods — `contactProfile`

- [`contactProfile.get`](#contactprofileget)
- [`contactProfile.set`](#contactprofileset)
- [`contactProfile.headerLine`](#contactprofileheaderline)

#### `contactProfile.get`

```ts
get(): Promise<ContactProfile>;
```

#### `contactProfile.set`

```ts
set(profile: ContactProfile): Promise<{ success: true }>;
```

Rejects on failure (unmanaged store, invalid payload, storage error) —
there is no in-band `{ error }` shape to inspect. A Tauri command
returning `Result` rejects the invoke promise on `Err`, which is what
lets `useSaveContactProfile`'s `onError` fire; an in-band error field
that no caller reads is a silent, permanent save failure.

#### `contactProfile.headerLine`

```ts
headerLine(lang: string): Promise<string>;
```

The stored profile's header contact line as markdown, localized for `lang`
— built by the single shared `ContactProfile::header_markdown` (Rust), never
re-implemented here, so the ordering rules can't drift between the two
languages. `''` when the profile has nothing to contribute.

### Channels — `contactProfile`

`CONTACT_PROFILE_CHANNELS` in `packages/shared/src/ipc/contracts/contactProfile.ts`:

| Key          | Channel                       |
| ------------ | ----------------------------- |
| `get`        | `contact_profile_get`         |
| `set`        | `contact_profile_set`         |
| `headerLine` | `contact_profile_header_line` |

### Types — `contactProfile`

Declared in `packages/shared/src/ipc/contracts/contactProfile.ts`.

```ts
/**
 * Contact profile — the single source of truth for the document header contact
 * line (name fields → clickable links), localized per language. Built from the
 * named fields here, never scavenged from the résumé's link pool, so a company
 * link can't displace the candidate's own profile / site.
 */

/** A free-text value with optional per-language (ISO-639-1) overrides. */
export interface LocalizedText {
  /** Value used when no language override matches. */
  default: string;
  /** ISO-639-1 (`de`, `en`, …) → localized value. */
  byLang?: Record<string, string>;
}

/** One additional labelled link beyond the named platform fields. */
export interface ContactLink {
  label: string;
  url: string;
}

/** Header contact fields, by name. Every field is optional. */
export interface ContactProfile {
  fullName?: string;
  email?: string;
  phone?: string;
  location?: LocalizedText;
  linkedin?: string;
  github?: string;
  website?: string;
  extraLinks?: ContactLink[];
  /**
   * Optional candidate photo as a `data:image/…;base64,…` URL produced by the
   * photo-upload control (decoded, square-cropped, downscaled, EXIF-stripped).
   * File paths are never accepted — local-only, never sent over the network.
   */
  photo?: string;
}
```

---

## `credentials`

Contract: `CredentialsContract` in `packages/shared/src/ipc/contracts/credentials.ts`

### Methods — `credentials`

- [`credentials.available`](#credentialsavailable)

#### `credentials.available`

```ts
available(): Promise<boolean>;
```

Whether the OS supports encrypted secret storage. Board logins use browser
sessions (`boards.*`), so there is no password CRUD here; this only gates
the encryption-availability warning.

### Channels — `credentials`

`CREDENTIALS_CHANNELS` in `packages/shared/src/ipc/contracts/credentials.ts`:

| Key         | Channel                 |
| ----------- | ----------------------- |
| `available` | `credentials:available` |

---

## `data`

Full app-data backup & restore (all persistent stores).

Contract: `DataContract` in `packages/shared/src/ipc/contracts/data.ts`

### Methods — `data`

- [`data.export`](#dataexport)
- [`data.import`](#dataimport)

#### `data.export`

```ts
export(): Promise<{ success: boolean; filePath?: string; error?: string }>;
```

Export all user data to a user-chosen JSON file.

#### `data.import`

```ts
import(): Promise<{
    success: boolean;
    /** True when one or more stores failed to restore (others may have succeeded). */
    partial?: boolean;
    imported?: Record<string, number | { error: string }>;
    error?: string;
  }>;
```

Restore all user data from a user-chosen backup file (replace semantics).

### Channels — `data`

`DATA_CHANNELS` in `packages/shared/src/ipc/contracts/data.ts`:

| Key      | Channel       |
| -------- | ------------- |
| `export` | `data:export` |
| `import` | `data:import` |

---

## `dedup`

Cross-board dedup (ADR-029): the renderer's only write into the clustering
feature. Clustering itself is recomputed in Rust at every ingest and is never
driven from here — the UI groups rows by the opaque `clusterId`/member keys
the backend attaches and, on a user "not a duplicate" action, echoes those
keys straight back through `DedupContract.markNotDuplicate`.

Contract: `DedupContract` in `packages/shared/src/ipc/contracts/dedup.ts`

### Methods — `dedup`

- [`dedup.markNotDuplicate`](#dedupmarknotduplicate)

#### `dedup.markNotDuplicate`

```ts
markNotDuplicate(
    req: DedupMarkNotDuplicateRequest
  ): Promise<{ success: true } | { error: string }>;
```

Record a "not a duplicate" verdict: `memberKey` is split from each of
`otherKeys` (opaque canonical job keys taken from a cluster's members). The
pair tombstones persist, so the split survives every re-scrape. Pass
`autopilotId` when splitting within an autopilot found-jobs view so that
record's annotations are recomputed too.

The command RESOLVES (never rejects) an `{ error }` union on failure — Tauri
turns the backend's `json!({"error": ...})` into a resolved value — so the
renderer must narrow this union and throw (see `useMarkNotDuplicate`),
mirroring `AiContract.setActiveProvider`.

### Channels — `dedup`

`DEDUP_CHANNELS` in `packages/shared/src/ipc/contracts/dedup.ts`:

| Key                | Channel                  |
| ------------------ | ------------------------ |
| `markNotDuplicate` | `dedup:markNotDuplicate` |

### Referenced types — `dedup`

- `packages/shared/src/schemas/index.ts` — `DedupMarkNotDuplicateRequest`

---

## `dialog`

Contract: `DialogContract` in `packages/shared/src/ipc/contracts/dialog.ts`

### Methods — `dialog`

- [`dialog.openFiles`](#dialogopenfiles)

#### `dialog.openFiles`

```ts
openFiles(opts?: {
    multiple?: boolean;
    filters?: Array<{ name: string; extensions: string[] }>;
  }): Promise<string[]>;
```

### Channels — `dialog`

`DIALOG_CHANNELS` in `packages/shared/src/ipc/contracts/dialog.ts`:

| Key         | Channel             |
| ----------- | ------------------- |
| `openFiles` | `dialog:open-files` |

---

## `discovery`

Discovery namespace (ADR-030 §f): reads over the discovered-companies store
that backs the ScrapeForm slug typeahead and the watched-company autopilot
target.

Contract: `DiscoveryContract` in `packages/shared/src/ipc/contracts/discovery.ts`

### Methods — `discovery`

- [`discovery.searchCompanies`](#discoverysearchcompanies)
- [`discovery.setStarred`](#discoverysetstarred)
- [`discovery.watched`](#discoverywatched)

#### `discovery.searchCompanies`

```ts
searchCompanies(req: DiscoverySearchRequest): Promise<DiscoveredCompany[]>;
```

Typeahead search over slug + display name (case-insensitive), starred first
then by most-seen. Debouncing is the UI's job.

#### `discovery.setStarred`

```ts
setStarred(req: DiscoveryStarRequest): Promise<{ success: true } | { error: string }>;
```

Star / unstar a company (materializing a curated-seed row if it was never
organically seen). RESOLVES an `{ error }` union on failure — Tauri turns the
backend's `json!({"error": ...})` into a resolved value — so the hook must
narrow it and throw (mirrors `DedupContract.markNotDuplicate`; #756 lesson).

#### `discovery.watched`

```ts
watched(): Promise<DiscoveredCompany[]>;
```

Every watched (starred) company.

### Channels — `discovery`

`DISCOVERY_CHANNELS` in `packages/shared/src/ipc/contracts/discovery.ts`:

| Key               | Channel                     |
| ----------------- | --------------------------- |
| `searchCompanies` | `discovery:searchCompanies` |
| `setStarred`      | `discovery:setStarred`      |
| `watched`         | `discovery:watched`         |

### Types — `discovery`

Declared in `packages/shared/src/ipc/contracts/discovery.ts`.

```ts
/**
 * A passively-harvested (or curated-seed) ATS company (ADR-030). `extract_ats_ref`
 * pulls `(atsKind, slug)` out of every scraped/imported posting URL; starred rows
 * are the user's "watched companies" that a `watchedCompaniesOnly` autopilot
 * resolves at run time.
 */
export interface DiscoveredCompany {
  /** Registry board id (`greenhouse`, `lever`, `ashby`, …). */
  atsKind: string;
  /** Company slug — casing is preserved (Ashby tokens are case-sensitive). */
  slug: string;
  /** Display name backfilled from the posting's company, when known. */
  displayName?: string;
  /** How many postings this slug has been seen in. */
  seenCount: number;
  /** Whether the user has starred it (a "watched company"). */
  starred: boolean;
  /** Provenance: `scrape | extension | seed` (free-text for future feeders). */
  source: string;
}
```

### Referenced types — `discovery`

- `packages/shared/src/schemas/index.ts` — `DiscoverySearchRequest`, `DiscoveryStarRequest`

---

## `documents`

Contract: `DocumentsContract` in `packages/shared/src/ipc/contracts/documents.ts`

### Methods — `documents`

- [`documents.list`](#documentslist)
- [`documents.getText`](#documentsgettext)
- [`documents.import`](#documentsimport)
- [`documents.recommendTemplate`](#documentsrecommendtemplate)
- [`documents.remove`](#documentsremove)
- [`documents.setDefault`](#documentssetdefault)
- [`documents.exportDocument`](#documentsexportdocument)
- [`documents.exportAndSave`](#documentsexportandsave)
- [`documents.renderPreviewImages`](#documentsrenderpreviewimages)

#### `documents.list`

```ts
list(): Promise<DocumentRecord[]>;
```

#### `documents.getText`

```ts
getText(id: string): Promise<string>;
```

Fetch the stored extracted text for one document by id. Returns the empty
string when the document is missing or has no text (never rejects), so a
caller can safely seed a generator without a missing-doc guard.

#### `documents.import`

```ts
import(req: DocumentImportRequest): Promise<{
    id: string;
    success: boolean;
    review?: StructuredResume;
    contactConflicts?: ContactFieldConflict[];
    suggestedContact?: ContactProfile;
  }>;
```

#### `documents.recommendTemplate`

```ts
recommendTemplate(req: TemplateRecommendSignals): Promise<TemplateRecommendation>;
```

Suggest a template + locale from the generation metadata signals.

#### `documents.remove`

```ts
remove(id: string): Promise<void>;
```

#### `documents.setDefault`

```ts
setDefault(id: string): Promise<void>;
```

#### `documents.exportDocument`

```ts
exportDocument(
    req: BaseExportRequest
  ): Promise<{ data: number[]; mimeType: string; filename: string; report?: ExportReport }>;
```

#### `documents.exportAndSave`

```ts
exportAndSave(req: BaseExportRequest): Promise<string>;
```

#### `documents.renderPreviewImages`

```ts
renderPreviewImages(req: BaseExportRequest): Promise<{ pages: string[]; mimeType: string }>;
```

Render the same document to per-page images for the live preview, shown via
`<img>` (CSP `img-src 'self' data: blob:`) instead of the PDF→iframe path.
Takes the same request fields as `exportDocument` (`format` is ignored
— the preview always emits SVG) and renders the identical model + Typst
world, so preview fidelity matches export. `pages` is one SVG document string
per page; `mimeType` is always `image/svg+xml`. Called imperatively (no React
Query key) — the preview is requested on demand, like an export.

### Channels — `documents`

`DOCUMENTS_CHANNELS` in `packages/shared/src/ipc/contracts/documents.ts`:

| Key                   | Channel                           |
| --------------------- | --------------------------------- |
| `list`                | `documents:list`                  |
| `getText`             | `documents:get_text`              |
| `import`              | `documents:import`                |
| `recommendTemplate`   | `documents:recommend_template`    |
| `remove`              | `documents:remove`                |
| `exportDocument`      | `documents:export_document`       |
| `exportAndSave`       | `documents:export_and_save`       |
| `renderPreviewImages` | `documents:render_preview_images` |

`DOCUMENTS_CHANNELS` registers 8 of this namespace's 9 methods; the rest have no entry in it.

### Types — `documents`

Declared in `packages/shared/src/ipc/contracts/documents.ts`.

```ts
export type TemplateId =
  | 'classic'
  | 'swiss-minimal'
  | 'academic'
  | 'atelier'
  | 'meridian'
  | 'throughline'
  | 'portrait'
  | 'lebenslauf'
  | 'cadence'
  | 'regent'
  | 'cologne-navy'
  | 'aria'
  | 'saffron'
  | 'jake'
  | 'awesome'
  | 'deedy';

/**
 * Cover-letter **layout** (arrangement only) — MUST match the Rust `LetterLayout`
 * enum (export/types.rs, kebab-case serde). A layout owns the letter's
 * composition; the palette and fonts always inherit from the chosen résumé
 * {@link TemplateId}. `classic` (the default) is the original single-`letter.typ`
 * arrangement, so an omitted value renders the pre-layout-picker output. Ignored
 * for résumé exports.
 */
export type LetterLayoutId = 'classic' | 'refined' | 'banded' | 'navy' | 'sidebar' | 'monogram';

export interface BaseExportRequest {
  text: string;
  format: 'docx' | 'pdf' | 'txt';
  documentType: 'resume' | 'cover-letter';
  templateId: TemplateId;
  meta?: ExportMeta;
  /**
   * ATS-safe rendering for **this** document — one flag, read per request, so a
   * résumé and a cover letter exported from the same session can carry different
   * values (each export request holds exactly one document).
   *
   * - Résumé: design-tier templates collapse to a single column and drop the
   *   photo. A documented **no-op** for ATS-tier templates (`single_column.typ`:
   *   "data.opts.ats — ATS flag (no-op for single column)").
   * - Cover letter: reaches the letter renderer as `data.opts.ats`; a decorated
   *   layout drops its decoration — Banded's band, Sidebar's contact rail,
   *   Monogram's initials tile (whose two characters otherwise land in the text
   *   layer in front of the candidate's name). `classic` / `refined` / `navy`
   *   have no decoration to drop, so the flag does nothing for them.
   *
   * The renderer surfaces the toggle wherever the flag can still act on the
   * document on screen (see `shouldClearAtsMode` / `isDecoratedLetterLayout`).
   */
  atsMode?: boolean;
  /** Target market id (`us`, `de`, …); drives the page size (US → Letter, else A4). */
  locale?: string;
  /**
   * Header contact source of truth — named fields rendered as clickable links,
   * localized per language. When present it overrides whatever links the
   * generated text carried, so a company link can't displace a personal profile.
   */
  contact?: ContactProfile;
  /**
   * Per-export **document accent** (ADR 0004): an optional 6-digit hex
   * (`#RRGGBB` or bare `RRGGBB`) recoloring the chosen template's accent.
   * Distinct from the app-UI accent — the backend never reads theme prefs.
   * Omitted (the default) leaves the template palette untouched; a malformed
   * value is ignored by the backend.
   */
  accent?: string;
  /**
   * Cover-letter **layout** — the arrangement of the letter, independent of the
   * résumé {@link TemplateId} (which still supplies the palette + fonts). Wire
   * name is `letterLayoutId` (matches the Rust `#[serde(rename = "letterLayoutId")]`).
   * Omitted (the default) → the backend renders `classic`. Ignored for résumé
   * exports.
   */
  letterLayoutId?: LetterLayoutId;
}

export type ExportIssueSeverity = 'critical' | 'warning';

/** A single problem found while re-reading an exported document. */
export interface ExportIssue {
  severity: ExportIssueSeverity;
  /** Stable machine code (e.g. `section_order`, `missing_section`). */
  code: string;
  /** Plain-language explanation for the user. */
  message: string;
}

/**
 * Pre-export validation report. Present for PDF/DOCX, absent for TXT. The
 * backend auto-fixes a two-column layout that doesn't survive extraction and
 * blocks the export only when a critical issue survives, so `ok` is `false`
 * only on a hard failure the user must address.
 */
export interface ExportReport {
  ok: boolean;
  /** Whether the returned bytes were rendered in ATS (single-column) mode. */
  atsMode: boolean;
  issues: ExportIssue[];
  /** Human-readable description of each auto-fix that was applied. */
  fixed: string[];
}

export interface CoverLetterExportRequest {
  templateId: TemplateId;
  /** Recipient first/last name — used for salutation. */
  recipientName?: string;
  /** Honorific: "Dr.", "Prof.", "Ms.", etc. — prepended to salutation. */
  recipientTitle?: string;
  recipientCompany?: string;
  /** Multi-line OK — rendered as recipient block. */
  recipientAddress?: string;
  /** Overrides the template's default closing phrase. */
  closingPhrase?: string;
  /** User's professional title — shown in NameAndTitle / ScriptStyle signatures. */
  signatureTitle?: string;
  /** Overrides the app locale for salutation and closing phrase resolution. */
  locale?: 'en' | 'de';
}

export type ConfidenceLevel = 'high' | 'medium' | 'low';

/** Byte span `[start, end)` into the extracted source text. */
export interface SourceSpan {
  start: number;
  end: number;
}

/** One structured-extraction field: value, confidence, and where it was found. */
export interface ResumeField<T> {
  value: T;
  confidence: ConfidenceLevel;
  sourceSpan?: SourceSpan;
}

/** A detected section in the review inventory. */
export interface SectionSummary {
  heading: string;
  /** Canonical kind: `experience`, `skills`, `custom`, … */
  kind: string;
  confidence: ConfidenceLevel;
}

/**
 * Typed view of an imported resume with per-field confidence. Returned by
 * `import` so the renderer can surface low-confidence / missing fields for
 * review before generation. `reviewRequired` flags (never blocks).
 */
export interface StructuredResume {
  name: ResumeField<string>;
  email?: ResumeField<string>;
  phone?: ResumeField<string>;
  location?: ResumeField<string>;
  links: ResumeField<string>[];
  sections: SectionSummary[];
  /** Whole-document confidence (the fast gate). */
  overall: ConfidenceLevel;
  reviewRequired: boolean;
  warnings: string[];
}

/**
 * A single identity field where the imported résumé's contact value conflicts
 * with the saved contact profile (both non-empty, normalized values differ).
 * The import never blocks on these — it still silently fills empty fields — but
 * they are returned so the renderer can let the user resolve each one per-field.
 * `field` is a stable key: `email`, `phone`, `fullName`, `linkedin`, `github`,
 * `website`, or `location`. `current`/`suggested` are the original (un-normalized)
 * values for faithful display.
 */
export interface ContactFieldConflict {
  field: string;
  current: string;
  suggested: string;
}

/** Signals the recommender reads — a subset of the generation metadata. */
export interface TemplateRecommendSignals {
  jobTitle?: string;
  /** `junior | mid | senior | lead | executive` */
  candidateSeniority?: string;
  topRequirements?: string[];
  resumeLanguage?: string;
  jobAdLanguage?: string;
  /** Job ad's target country/market (`us`, `de`, `gb`, …); wins over language. */
  targetCountry?: string;
}

/** A template + locale suggestion with a printed reason. Always overridable. */
export interface TemplateRecommendation {
  templateId: TemplateId;
  /** Market id (`us`, `dach`, `en`, …). */
  locale: string;
  atsSuggested: boolean;
  rationale: string;
}
```

### Referenced types — `documents`

- `packages/shared/src/ipc/contracts/contactProfile.ts` — `ContactProfile`
- `packages/shared/src/schemas/index.ts` — `DocumentImportRequest`
- `packages/shared/src/types/index.ts` — `DocumentRecord`

---

## `emailWatch`

Contract: `EmailWatchContract` in `packages/shared/src/ipc/contracts/emailWatch.ts`

### Methods — `emailWatch`

- [`emailWatch.status`](#emailwatchstatus)
- [`emailWatch.connect`](#emailwatchconnect)
- [`emailWatch.disconnect`](#emailwatchdisconnect)
- [`emailWatch.setEnabled`](#emailwatchsetenabled)
- [`emailWatch.setAutoWriteEnabled`](#emailwatchsetautowriteenabled)
- [`emailWatch.checkNow`](#emailwatchchecknow)

#### `emailWatch.status`

```ts
status(): Promise<EmailWatchStatus>;
```

#### `emailWatch.connect`

```ts
connect(req: EmailWatchConnectRequest): Promise<EmailWatchStatus>;
```

Validates by a real IMAP LOGIN + SELECT INBOX before persisting.

#### `emailWatch.disconnect`

```ts
disconnect(): Promise<EmailWatchStatus>;
```

Removes the keychain app password and clears the account row.

#### `emailWatch.setEnabled`

```ts
setEnabled(enabled: boolean): Promise<EmailWatchStatus>;
```

#### `emailWatch.setAutoWriteEnabled`

```ts
setAutoWriteEnabled(enabled: boolean): Promise<EmailWatchStatus>;
```

The v2 auto-write opt-in — see `EmailWatchStatus.autoWriteEnabled`.
Echoes the fresh status back, like every other mutating call here.

#### `emailWatch.checkNow`

```ts
checkNow(): Promise<EmailWatchStatus>;
```

Runs a real fetch+parse+match+notify pass now (the same pass the
background poller runs). Rejects if a check already ran too recently.

### Channels — `emailWatch`

`EMAIL_WATCH_CHANNELS` in `packages/shared/src/ipc/contracts/emailWatch.ts`:

| Key                   | Channel                          |
| --------------------- | -------------------------------- |
| `status`              | `emailWatch:status`              |
| `connect`             | `emailWatch:connect`             |
| `disconnect`          | `emailWatch:disconnect`          |
| `setEnabled`          | `emailWatch:setEnabled`          |
| `setAutoWriteEnabled` | `emailWatch:setAutoWriteEnabled` |
| `checkNow`            | `emailWatch:checkNow`            |

### Types — `emailWatch`

Declared in `packages/shared/src/ipc/contracts/emailWatch.ts`.

```ts
/**
 * Email-confirmation watching (Task #23, auto-track Layer C) — IMAP
 * connect/status/enable control surface, plus the poller it gates.
 *
 * The backend validates the address/app-password by a real IMAP `LOGIN` +
 * `SELECT INBOX` before persisting anything (`connect`). Once `enabled`, a
 * backend-owned background poller periodically fetches new INBOX headers,
 * fingerprints them as plausible application-confirmation emails, and
 * fuzzy-matches company/title against the user's saved applications — a
 * match surfaces as a Notification Center card AND (v2, gated by
 * {@link EmailWatchStatus.autoWriteEnabled}) an UNCONFIRMED status write the
 * application's timeline surfaces with Accept/Reject
 * (`applications.acceptStatusEvent`/`rejectStatusEvent`) — never a
 * `confirmed: true` write; adjudication is the whole safety model. `checkNow`
 * runs that SAME pass on-demand, gated by a short server-side min-interval
 * guard (rejects if a check ran too recently) so it can't be used to spam
 * Gmail logins. `appPassword` is write-only: sent once to `connect`, stored
 * in the OS keychain, and never returned or logged.
 */

/** Current connection status. `connected` means an account has been
 *  configured (a successful `connect`), not that a live socket is open —
 *  there is no persistent IMAP connection; each check connects fresh. */
export interface EmailWatchStatus {
  connected: boolean;
  address?: string;
  /** The poller opt-in — default OFF, independent of `connected`. */
  enabled: boolean;
  lastCheckAt?: number;
  /** Timestamp of the most recent email→application match, if any. */
  lastMatchAt?: number;
  /** The v2 auto-write opt-in — default ON, independent of `enabled`. An
   *  escape hatch, not the primary safeguard: every auto-write always lands
   *  `confirmed: false` regardless of this toggle: turning it off just stops
   *  the write from happening at all, in favour of the notify-only v1
   *  behaviour. */
  autoWriteEnabled: boolean;
}

export interface EmailWatchConnectRequest {
  address: string;
  appPassword: string;
}
```

---

## `extensionBridge`

Contract: `ExtensionBridgeContract` in `packages/shared/src/ipc/contracts/extensionBridge.ts`

### Methods — `extensionBridge`

- [`extensionBridge.status`](#extensionbridgestatus)
- [`extensionBridge.regenerateToken`](#extensionbridgeregeneratetoken)
- [`extensionBridge.autofillEnabled`](#extensionbridgeautofillenabled)
- [`extensionBridge.setAutofillEnabled`](#extensionbridgesetautofillenabled)
- [`extensionBridge.aiAssistEnabled`](#extensionbridgeaiassistenabled)
- [`extensionBridge.setAiAssistEnabled`](#extensionbridgesetaiassistenabled)
- [`extensionBridge.autoTrackEnabled`](#extensionbridgeautotrackenabled)
- [`extensionBridge.setAutoTrackEnabled`](#extensionbridgesetautotrackenabled)
- [`extensionBridge.onChanged`](#extensionbridgeonchanged)

#### `extensionBridge.status`

```ts
status(): Promise<ExtensionBridgeStatus>;
```

#### `extensionBridge.regenerateToken`

```ts
regenerateToken(): Promise<ExtensionBridgeTokenResult>;
```

#### `extensionBridge.autofillEnabled`

```ts
autofillEnabled(): Promise<ExtensionAutofillSetting>;
```

Read the assisted-autofill opt-in (default OFF).

#### `extensionBridge.setAutofillEnabled`

```ts
setAutofillEnabled(enabled: boolean): Promise<ExtensionAutofillSetting>;
```

Set + persist the assisted-autofill opt-in; echoes the stored value.

#### `extensionBridge.aiAssistEnabled`

```ts
aiAssistEnabled(): Promise<ExtensionAiAssistSetting>;
```

Read the AI-answer-assist opt-in (default OFF).

#### `extensionBridge.setAiAssistEnabled`

```ts
setAiAssistEnabled(enabled: boolean): Promise<ExtensionAiAssistSetting>;
```

Set + persist the AI-answer-assist opt-in; echoes the stored value. A bare
boolean — the billable-AI consent gate. It no longer snapshots a provider:
a draft resolves the active provider from the backend active-provider store
at answer-time (task #16), so nothing more needs capturing here.

#### `extensionBridge.autoTrackEnabled`

```ts
autoTrackEnabled(): Promise<ExtensionAutoTrackSetting>;
```

Read the auto-track opt-in (default OFF).

#### `extensionBridge.setAutoTrackEnabled`

```ts
setAutoTrackEnabled(enabled: boolean): Promise<ExtensionAutoTrackSetting>;
```

Set + persist the auto-track opt-in; echoes the stored value.

#### `extensionBridge.onChanged`

```ts
onChanged(handler: (event: ExtensionBridgeChangedEvent) => void): () => void;
```

Subscribe to a live connection-count transition (0→1 / →0). Returns a
sync unsubscribe handle — mirrors `ApplicationsContract.onChanged`.

### Channels — `extensionBridge`

`EXTENSION_BRIDGE_CHANNELS` in `packages/shared/src/ipc/contracts/extensionBridge.ts`:

| Key                   | Channel                               |
| --------------------- | ------------------------------------- |
| `status`              | `extensionBridge:status`              |
| `regenerateToken`     | `extensionBridge:regenerateToken`     |
| `autofillEnabled`     | `extensionBridge:autofillEnabled`     |
| `setAutofillEnabled`  | `extensionBridge:setAutofillEnabled`  |
| `aiAssistEnabled`     | `extensionBridge:aiAssistEnabled`     |
| `setAiAssistEnabled`  | `extensionBridge:setAiAssistEnabled`  |
| `autoTrackEnabled`    | `extensionBridge:autoTrackEnabled`    |
| `setAutoTrackEnabled` | `extensionBridge:setAutoTrackEnabled` |

`EXTENSION_BRIDGE_CHANNELS` registers 8 of this namespace's 9 methods; the rest have no entry in it.

### Types — `extensionBridge`

Declared in `packages/shared/src/ipc/contracts/extensionBridge.ts`.

```ts
/**
 * Extension-bridge control capability (Feature 2).
 *
 * Read the local WebSocket bridge's status (bound port, whether an extension is
 * currently paired/connected, and the current pairing token) and regenerate the
 * pairing token. The bridge itself is a loopback WS server in
 * `apps/desktop/src-tauri/src/extension_bridge`; this namespace is only the
 * renderer's control surface over it (show the token in Settings, rotate it).
 */

/** Current bridge status. `port` is `null` when the server failed to bind. */
export interface ExtensionBridgeStatus {
  port: number | null;
  connected: boolean;
  token: string;
}

/** Result of rotating the pairing token (existing sockets must re-pair). */
export interface ExtensionBridgeTokenResult {
  token: string;
}

/**
 * Assisted-autofill opt-in state. When `enabled`, a `profile.get` from the
 * extension returns the user's contact profile so it can fill matching empty form
 * fields on the current page; when off, the desktop refuses. Default OFF.
 */
export interface ExtensionAutofillSetting {
  enabled: boolean;
}

/**
 * AI-answer-assist opt-in state (extension roadmap PR 9) — a SEPARATE opt-in
 * from {@link ExtensionAutofillSetting}: `answer.assist` is billable provider
 * spend (a materially different consent class from the local/free autofill
 * verbs), so it gets its own desktop-enforced gate, default OFF.
 *
 * A bare boolean flag: no provider/model snapshot. A draft resolves the active
 * provider from the backend-owned active-provider store at answer-time (task
 * #16), so a Settings row reads that store (`aiActiveConfig`) for its
 * "Using: X · Y" label rather than a field echoed back here.
 */
export interface ExtensionAiAssistSetting {
  enabled: boolean;
}

/**
 * Auto-track opt-in state (Task #22, auto-track Layer A) — a SEPARATE opt-in
 * from {@link ExtensionAutofillSetting}/{@link ExtensionAiAssistSetting}. When
 * on, the extension arms a gesture submit-watcher that, on a detected form
 * submit, auto-marks the matched `saved` application `applied` (or nudges you
 * to import an untracked one). Default OFF, desktop-enforced: the desktop also
 * re-checks it before honoring an automated write, so a compromised extension
 * can't auto-mark applied without this consent.
 */
export interface ExtensionAutoTrackSetting {
  enabled: boolean;
}

/**
 * Pushed on a 0→1 or →0 transition in the live paired-connection COUNT (not a
 * per-socket event) — the desktop supports multiple browsers sharing one
 * pairing token, each with its own socket, so this only fires when the last
 * one disconnects or the first one (re)connects, never on an intermediate
 * pairing/close while at least one other socket stays open.
 */
export interface ExtensionBridgeChangedEvent {
  connected: boolean;
}
```

---

## `geocode`

Contract: `GeocodeContract` in `packages/shared/src/ipc/contracts/geocode.ts`

### Methods — `geocode`

- [`geocode.suggest`](#geocodesuggest)

#### `geocode.suggest`

```ts
suggest(query: string): Promise<GeocodeSuggestion[]>;
```

Location autocomplete, filtered to city-level and country-level results
only (`to_city_country` in
`apps/desktop/src-tauri/src/commands/geocoding.rs`) — a street or a venue
is never a job-search location. `display` reads `"City, Country"` for a city
and the bare country name for a country-level match.

### Channels — `geocode`

`GEOCODE_CHANNELS` in `packages/shared/src/ipc/contracts/geocode.ts`:

| Key       | Channel           |
| --------- | ----------------- |
| `suggest` | `geocode:suggest` |

### Types — `geocode`

Declared in `packages/shared/src/ipc/contracts/geocode.ts`.

```ts
export interface GeocodeSuggestion {
  display: string;
  /** WGS84 latitude of the place (for radius search). */
  lat?: number | null;
  /** WGS84 longitude of the place (for radius search). */
  lon?: number | null;
  /** ISO 3166-1 alpha-2 country code (upper-case) — for country-correct filtering (#49). */
  countryCode?: string | null;
}
```

---

## `github`

Contract: `GitHubContract` in `packages/shared/src/ipc/contracts/github.ts`

### Methods — `github`

- [`github.importRepos`](#githubimportrepos)

#### `github.importRepos`

```ts
importRepos(input: string): Promise<GitHubRepo[]>;
```

Fetch a user's public repos. `input` is a bare username or a
`github.com/<user>` URL. Resolves to the repo list (the `{ repos }`
envelope is unwrapped in the client layer); rejects on validation /
rate-limit / not-found errors.

### Channels — `github`

`GITHUB_CHANNELS` in `packages/shared/src/ipc/contracts/github.ts`:

| Key           | Channel               |
| ------------- | --------------------- |
| `importRepos` | `github_import_repos` |

### Types — `github`

Declared in `packages/shared/src/ipc/contracts/github.ts`.

```ts
/**
 * GitHub repos import — fetch a user's public repos for the resume-builder
 * "Import from GitHub" projects step. The backend extracts + validates the
 * username (bare name or `github.com/<user>` URL), drops forks, sorts by stars,
 * and caps to the top 30. Fields are camelCase to match the Rust output struct.
 */

/** A single public GitHub repo offered to the candidate for import. */
export interface GitHubRepo {
  name: string;
  /** Omitted by the backend when absent (serde `skip_serializing_if`). */
  description?: string;
  /** Canonical repo URL — kept verbatim; the AI step never rewrites it. */
  htmlUrl: string;
  language?: string;
  topics: string[];
  /** `stargazers_count` from the GitHub API. */
  stars: number;
  pushedAt?: string;
}
```

---

## `jobPreferences`

Contract: `JobPreferencesContract` in `packages/shared/src/ipc/contracts/jobPreferences.ts`

### Methods — `jobPreferences`

- [`jobPreferences.get`](#jobpreferencesget)
- [`jobPreferences.set`](#jobpreferencesset)
- [`jobPreferences.setSalaryExpectation`](#jobpreferencessetsalaryexpectation)
- [`jobPreferences.setExtraAgencyCompanies`](#jobpreferencessetextraagencycompanies)
- [`jobPreferences.setSemanticScoring`](#jobpreferencessetsemanticscoring)

#### `jobPreferences.get`

```ts
get(): Promise<JobPreferences>;
```

#### `jobPreferences.set`

```ts
set(prefs: JobPreferences): Promise<void>;
```

#### `jobPreferences.setSalaryExpectation`

```ts
setSalaryExpectation(salaryExpectation: string | undefined): Promise<void>;
```

Single-column salary-expectation write (review fix, PR #695) — unlike
`set()`, this NEVER touches `location`/`techStack`/`countryCode`. Callers
that only have the salary value on hand (not a freshly-read copy of the
other fields) MUST use this instead of `set({ ...maybeStaleOrUndefined,
salaryExpectation })`, which would silently NULL every other field when
the spread source is stale or hasn't loaded yet.

#### `jobPreferences.setExtraAgencyCompanies`

```ts
setExtraAgencyCompanies(companies: string[] | undefined): Promise<void>;
```

Single-column extra-agency-companies write (ADR-029 §i) — like
`setSalaryExpectation`, this NEVER touches the other columns, so an
agency-list edit can't NULL the user's saved location/techStack/countryCode/
salaryExpectation via a stale spread (PR #695 pattern). `undefined`/empty
clears the list.

#### `jobPreferences.setSemanticScoring`

```ts
setSemanticScoring(enabled: boolean): Promise<void>;
```

Single-column mirror of the renderer's `semanticScoring` preference
(ADR-020 addendum). The setting itself lives in the webview's
`localStorage`, which no Rust code can read — the headless Autopilot
scheduler needs this copy to decide whether to run its semantic re-rank.
Write-only from the renderer's perspective (the preference store stays the
source of truth); like the two setters above it NEVER touches another
column.

### Channels — `jobPreferences`

`JOB_PREFERENCES_CHANNELS` in `packages/shared/src/ipc/contracts/jobPreferences.ts`:

| Key                       | Channel                                  |
| ------------------------- | ---------------------------------------- |
| `get`                     | `jobPreferences:get`                     |
| `set`                     | `jobPreferences:set`                     |
| `setSalaryExpectation`    | `jobPreferences:setSalaryExpectation`    |
| `setExtraAgencyCompanies` | `jobPreferences:setExtraAgencyCompanies` |
| `setSemanticScoring`      | `jobPreferences:setSemanticScoring`      |

### Referenced types — `jobPreferences`

- `packages/shared/src/schemas/index.ts` — `JobPreferences`

---

## `jobs`

Contract: `JobsContract` in `packages/shared/src/ipc/contracts/jobs.ts`

### Methods — `jobs`

- [`jobs.list`](#jobslist)
- [`jobs.get`](#jobsget)
- [`jobs.cancel`](#jobscancel)
- [`jobs.retry`](#jobsretry)
- [`jobs.onEvent`](#jobsonevent)

#### `jobs.list`

```ts
list(): Promise<JobRecord[]>;
```

#### `jobs.get`

```ts
get(jobId: string): Promise<JobRecord | null>;
```

#### `jobs.cancel`

```ts
cancel(jobId: string): Promise<void>;
```

#### `jobs.retry`

```ts
retry(jobId: string): Promise<void>;
```

#### `jobs.onEvent`

```ts
onEvent(handler: (event: JobEvent) => void): () => void;
```

### Channels — `jobs`

`JOBS_CHANNELS` in `packages/shared/src/ipc/contracts/jobs.ts`:

| Key      | Channel       |
| -------- | ------------- |
| `list`   | `jobs:list`   |
| `get`    | `jobs:get`    |
| `cancel` | `jobs:cancel` |
| `retry`  | `jobs:retry`  |

`JOBS_CHANNELS` registers 4 of this namespace's 5 methods; the rest have no entry in it.

### Referenced types — `jobs`

- `packages/shared/src/types/index.ts` — `JobEvent`, `JobRecord`

---

## `linkedin`

Contract: `LinkedinContract` in `packages/shared/src/ipc/contracts/linkedin.ts`

### Methods — `linkedin`

- [`linkedin.connect`](#linkedinconnect)
- [`linkedin.disconnect`](#linkedindisconnect)
- [`linkedin.getStatus`](#linkedingetstatus)
- [`linkedin.importProfileFromUrl`](#linkedinimportprofilefromurl)
- [`linkedin.importCookies`](#linkedinimportcookies)

#### `linkedin.connect`

```ts
connect(): Promise<{ connected: boolean; accountEmail?: string }>;
```

Connect to LinkedIn by launching a browser for manual login.

#### `linkedin.disconnect`

```ts
disconnect(): Promise<void>;
```

Disconnect and clear LinkedIn session.

#### `linkedin.getStatus`

```ts
getStatus(): Promise<{ connected: boolean; accountEmail?: string; lastConnected?: number }>;
```

Get current LinkedIn session status.

#### `linkedin.importProfileFromUrl`

```ts
importProfileFromUrl(
    url: string
  ): Promise<{ text: string; name?: string; platform: string } | { error: string }>;
```

Fetch a LinkedIn profile URL and return extracted resume text.

#### `linkedin.importCookies`

```ts
importCookies(): Promise<CookieImportResult>;
```

Import an existing LinkedIn session from the installed browser's cookie store.

### Channels — `linkedin`

`LINKEDIN_CHANNELS` in `packages/shared/src/ipc/contracts/linkedin.ts`:

| Key                    | Channel                         |
| ---------------------- | ------------------------------- |
| `connect`              | `linkedin:connect`              |
| `disconnect`           | `linkedin:disconnect`           |
| `getStatus`            | `linkedin:getStatus`            |
| `importProfileFromUrl` | `linkedin:importProfileFromUrl` |
| `importCookies`        | `linkedin:importCookies`        |

### Referenced types — `linkedin`

- `packages/shared/src/ipc/contracts/boards.ts` — `CookieImportResult`

---

## `match`

Contract: `MatchContract` in `packages/shared/src/ipc/contracts/match.ts`

### Methods — `match`

- [`match.resume`](#matchresume)
- [`match.text`](#matchtext)
- [`match.trimSuggestions`](#matchtrimsuggestions)

#### `match.resume`

```ts
resume(req: MatchResumeRequest): Promise<MatchScore>;
```

Score one résumé against one job. The single scoring path: the jobs list
asks for a score per row as that row renders, rather than running one pass
over everything (the one-shot `match_resume_batch` command was removed —
it had no consumers).

Keyword-only by default. Semantic (embedding) scoring is opt-in per
request via `semanticScoringEnabled`; omitting it means keyword-only, not
"provider decides".

#### `match.text`

```ts
text(req: MatchTextRequest): Promise<MatchScore>;
```

Score one résumé against arbitrary job-ad TEXT — for a caller with a
`jobDesc: string` in hand but no `PostingsCache` id (e.g. the Score tab in
`JobAdView`, whose `TailorFlow` parent receives an `Application` /
`AutopilotFoundJob`, neither of which carries one). Routes through the
SAME shared kernel `resume()` does, over the SAME pre-processed text (the
Rust command strips markdown before scoring, exactly as `resume()` does
for a cached posting) — not a second scorer — but two axes still
legitimately diverge from `resume()`: this call never has a title or
requirements to compose in (only the description `JobAdView` holds), and
semantic (embedding) scoring is always OFF here, never
caller-configurable, so its number only matches `resume()`'s when
semantic scoring is off there too. Content-addressed on the pre-processed
job text, so repeated opens of the same posting reuse that cached score.
`scoreSource` is therefore always `'keyword'` on the result.

#### `match.trimSuggestions`

```ts
trimSuggestions(req: ResumeTrimSuggestionsRequest): Promise<TrimSuggestions>;
```

### Channels — `match`

`MATCH_CHANNELS` in `packages/shared/src/ipc/contracts/match.ts`:

| Key               | Channel                 |
| ----------------- | ----------------------- |
| `resume`          | `match:resume`          |
| `text`            | `match:text`            |
| `trimSuggestions` | `match:trimSuggestions` |

### Referenced types — `match`

- `packages/shared/src/schemas/index.ts` — `MatchResumeRequest`, `MatchTextRequest`, `ResumeTrimSuggestionsRequest`
- `packages/shared/src/types/index.ts` — `MatchScore`, `TrimSuggestions`

---

## `menu`

Contract: `MenuContract` in `packages/shared/src/ipc/contracts/menu.ts`

### Methods — `menu`

- [`menu.onNavigate`](#menuonnavigate)
- [`menu.onAction`](#menuonaction)
- [`menu.takePending`](#menutakepending)

#### `menu.onNavigate`

```ts
onNavigate(handler: (event: MenuNavigateEvent) => void): () => void;
```

Fired by the native menu (and other shell chrome) to deep-link into a
route. `section` carries a settings sub-section when `route` is the
settings page (e.g. `{ route: '/settings', section: 'ai' }`); `null`
otherwise.

#### `menu.onAction`

```ts
onAction(handler: (event: MenuActionEvent) => void): () => void;
```

Fired by the native menu for app-level actions that aren't routes:
trigger an update check or open the keyboard-shortcuts cheat-sheet.

#### `menu.takePending`

```ts
takePending(): Promise<PendingMenuIntent | null>;
```

Atomically take + clear the menu intent buffered by the shell while the
window was hidden/minimized (close-to-tray). The shell's `emit` is
fire-and-forget, so a `menu:navigate`/`menu:action` fired right after the
window is un-hidden lands before the resumed webview re-attaches its
listeners and is lost; the renderer pulls the buffered intent once its JS
loop is live (on mount and on window focus/visibility-restore). Returns
`null` when nothing is buffered.

### Channels — `menu`

`menu` has no `*_CHANNELS` constant and is absent from `IPC_CHANNELS`.

### Types — `menu`

Declared in `packages/shared/src/ipc/contracts/menu.ts`.

```ts
/** A menu intent buffered shell-side and pulled by the renderer. Discriminated
 *  by the same event name the shell would otherwise `emit`. */
export type PendingMenuIntent =
  | { event: 'menu:navigate'; payload: MenuNavigateEvent }
  | { event: 'menu:action'; payload: MenuActionEvent };

export interface MenuNavigateEvent {
  route: string;
  section: string | null;
  /** Optional in-page focus signal carried alongside the route. The native menu
   *  and tray omit it; the `ajh://settings/extension` deep link sets it to
   *  `'extension-token'` so the Accounts → Browser-extension section focuses the
   *  pairing token. Optional so omitting consumers (the native menu) still
   *  type-check. */
  focus?: 'extension-token';
}

export interface MenuActionEvent {
  action: 'check-updates' | 'shortcuts';
}
```

---

## `notifications`

Notification Center capability (Phase 2). The read/mutate seam over the
persisted Rust `NotificationStore`. Every mutator resolves once the store has
persisted; the renderer keeps its inbox live via `onChanged`. `clicked` is the
unified OS-banner / tray click target (focuses the window + opens the inbox via
`onOpenInbox`).

Contract: `NotificationsContract` in `packages/shared/src/ipc/contracts/notifications.ts`

### Methods — `notifications`

- [`notifications.list`](#notificationslist)
- [`notifications.markRead`](#notificationsmarkread)
- [`notifications.markAllRead`](#notificationsmarkallread)
- [`notifications.remove`](#notificationsremove)
- [`notifications.clearAll`](#notificationsclearall)
- [`notifications.clicked`](#notificationsclicked)
- [`notifications.onChanged`](#notificationsonchanged)
- [`notifications.onOpenInbox`](#notificationsonopeninbox)
- [`notifications.onToast`](#notificationsontoast)

#### `notifications.list`

```ts
list(): Promise<AppNotification[]>;
```

#### `notifications.markRead`

```ts
markRead(id: string): Promise<void>;
```

#### `notifications.markAllRead`

```ts
markAllRead(): Promise<void>;
```

#### `notifications.remove`

```ts
remove(id: string): Promise<void>;
```

#### `notifications.clearAll`

```ts
clearAll(): Promise<void>;
```

#### `notifications.clicked`

```ts
clicked(): Promise<void>;
```

Invokes `notifications_clicked` — focuses the window and opens the inbox.

#### `notifications.onChanged`

```ts
onChanged(handler: () => void): () => void;
```

Subscribe to list changes (push / read / remove / clear). Sync unsubscribe.

#### `notifications.onOpenInbox`

```ts
onOpenInbox(handler: (payload: NotificationOpen) => void): () => void;
```

Subscribe to the "open inbox" signal. Emitted by a tray click (no `route`)
and by an OS-banner click, which carries the clicked notification's own
`route` so the renderer can navigate straight to it rather than only
opening the inbox. Sync unsubscribe.

#### `notifications.onToast`

```ts
onToast(handler: (toast: NotificationToast) => void): () => void;
```

Subscribe to in-app toasts: a notification was just pushed while the window
was focused, so the renderer shows a transient toast (with a "View" that
follows the carried `route`) instead of relying on the OS banner. Sync
unsubscribe. See the Rust `notifications:toast` emit in `push_and_notify`.

### Channels — `notifications`

`NOTIFICATIONS_CHANNELS` in `packages/shared/src/ipc/contracts/notifications.ts` is declared and empty — no channel name is registered here. Read the constant's own doc comment for where the names come from.

### Referenced types — `notifications`

- `packages/shared/src/types/index.ts` — `AppNotification`, `NotificationOpen`, `NotificationToast`

---

## `privacy`

Contract: `PrivacyContract` in `packages/shared/src/ipc/contracts/privacy.ts`

### Methods — `privacy`

- [`privacy.signOutAll`](#privacysignoutall)
- [`privacy.clearInteractions`](#privacyclearinteractions)
- [`privacy.resetApp`](#privacyresetapp)
- [`privacy.getCrashReporting`](#privacygetcrashreporting)
- [`privacy.setCrashReporting`](#privacysetcrashreporting)

#### `privacy.signOutAll`

```ts
signOutAll(): Promise<void>;
```

Sign out all connected accounts by wiping Chromium profiles.

#### `privacy.clearInteractions`

```ts
clearInteractions(): Promise<void>;
```

Clear all saved job interaction history (applied, viewed, bookmarked).

#### `privacy.resetApp`

```ts
resetApp(): Promise<PrivacyResetResult>;
```

Factory reset: sign out all boards, clear all cached data. Frontend resets preferences separately.

#### `privacy.getCrashReporting`

```ts
getCrashReporting(): Promise<CrashReportingSettings>;
```

Current crash-reporting consent state.

#### `privacy.setCrashReporting`

```ts
setCrashReporting(settings: CrashReportingSettings): Promise<CrashReportingSettings>;
```

Persist crash-reporting consent.

Turning it OFF unbinds the client immediately, so no further error or
breadcrumb events are captured in this session. The native-crash supervisor
is a separate process forked at launch and cannot be recalled mid-session,
so a hard crash before the next restart could still be reported — the UI
says so rather than implying a clean instant stop.

### Channels — `privacy`

`PRIVACY_CHANNELS` in `packages/shared/src/ipc/contracts/privacy.ts`:

| Key                 | Channel                     |
| ------------------- | --------------------------- |
| `signOutAll`        | `privacy:signOutAll`        |
| `clearInteractions` | `privacy:clearInteractions` |
| `resetApp`          | `privacy:resetApp`          |
| `getCrashReporting` | `privacy:getCrashReporting` |
| `setCrashReporting` | `privacy:setCrashReporting` |

### Types — `privacy`

Declared in `packages/shared/src/ipc/contracts/privacy.ts`.

```ts
/**
 * Outcome of a factory reset. The Rust `privacy_reset_app` command wipes every
 * persistent store and then removes the on-disk Chromium board-login profiles.
 * Store-wipe always succeeds; the profile removal is best-effort (a browser may
 * still hold a file lock, common on Windows), so a partial reset is reported
 * honestly instead of over-claiming a clean wipe.
 *
 * - `{ success: true }` — full reset (stores wiped + browser state removed).
 * - `{ success: false, error, browserStateRetained: true }` — partial reset:
 *   stores were wiped but board-login sessions remain on disk.
 */
export interface PrivacyResetResult {
  success: boolean;
  error?: string;
  browserStateRetained?: boolean;
}

/**
 * Crash-reporting consent. Rust-owned rather than a renderer preference: the
 * Sentry client is constructed before `tauri::Builder` runs (the minidump
 * supervisor forks at startup), so there is no WebView — and therefore no
 * `localStorage` — to read at the moment the decision is needed.
 *
 * `enabled` is the user's choice and defaults to **true**. `consentShown`
 * records whether the setup wizard has actually put that choice in front of
 * them; nothing is transmitted until it has, so a default nobody saw never
 * silently reports. Transmission requires BOTH flags (`Settings::transmits`).
 *
 * These two are the whole surface — see ADR-0020. There is no analytics
 * toggle and no retention setting because there is no behavioural analytics to
 * switch off, and retention belongs to the processor, not to the app.
 */
export interface CrashReportingSettings {
  enabled: boolean;
  consentShown: boolean;
}
```

---

## `referrals`

Contract: `ReferralsContract` in `packages/shared/src/ipc/contracts/referrals.ts`

### Methods — `referrals`

- [`referrals.list`](#referralslist)
- [`referrals.upsert`](#referralsupsert)
- [`referrals.remove`](#referralsremove)

#### `referrals.list`

```ts
list(jobUrl?: string): Promise<ReferralContact[]>;
```

All referral contacts, optionally filtered to one job's `jobUrl`.

#### `referrals.upsert`

```ts
upsert(req: ReferralUpsertRequest): Promise<ReferralContact>;
```

Create or update a contact; resolves to the stored record.

#### `referrals.remove`

```ts
remove(id: string): Promise<void>;
```

### Channels — `referrals`

`REFERRALS_CHANNELS` in `packages/shared/src/ipc/contracts/referrals.ts`:

| Key      | Channel            |
| -------- | ------------------ |
| `list`   | `referrals:list`   |
| `upsert` | `referrals:upsert` |
| `remove` | `referrals:remove` |

### Types — `referrals`

Declared in `packages/shared/src/ipc/contracts/referrals.ts`.

```ts
/** The channel the user plans to reach the referral contact through. */
export type ReferralChannel = 'email' | 'linkedin_message' | 'connection_note';

/** Where a referral ask stands in the user's manual outreach flow. */
export type ReferralStatus = 'draft' | 'sent' | 'replied';

/**
 * A locally-stored "referral contact": a person the user wants to ask for a
 * referral at a target company. Every detail is entered MANUALLY by the user —
 * there is no LinkedIn scraping or profile fetch. `linkedinUrl` is just an
 * optional free-text field the user pastes in.
 */
export interface ReferralContact {
  id: string;
  /** The job this referral targets — links the contact to an autopilot found job. */
  jobUrl: string;
  companyName: string;
  personName: string;
  /** The person's role/title, if the user noted it. */
  personRole?: string;
  /** Manual free text — never fetched or scraped. */
  linkedinUrl?: string;
  /** A drafted referral email, if any. */
  emailDraft?: string;
  /** A drafted LinkedIn message, if any. */
  messageDraft?: string;
  /** A drafted connection-request note, if any. */
  inviteNoteDraft?: string;
  channel: ReferralChannel;
  status: ReferralStatus;
  /** Free-form notes about the contact. */
  notes?: string;
  createdAt: number;
  updatedAt: number;
}

/**
 * Create or update a referral contact in one call. An absent `id` inserts a
 * fresh row (the store assigns the id, `createdAt`, and `updatedAt`); a present
 * `id` overwrites that row and bumps `updatedAt`.
 */
export interface ReferralUpsertRequest {
  /** Absent → insert; present → overwrite the row with this id. */
  id?: string;
  jobUrl?: string;
  companyName?: string;
  personName?: string;
  personRole?: string;
  /** Manual free text — never fetched or scraped. */
  linkedinUrl?: string;
  emailDraft?: string;
  messageDraft?: string;
  inviteNoteDraft?: string;
  channel?: ReferralChannel;
  status?: ReferralStatus;
  notes?: string;
}
```

---

## `resume`

Contract: `ResumeContract` in `packages/shared/src/ipc/contracts/resume.ts`

### Methods — `resume`

- [`resume.extractText`](#resumeextracttext)
- [`resume.validateContent`](#resumevalidatecontent)

#### `resume.extractText`

```ts
extractText(req: { name: string; bytes: Uint8Array }): Promise<{ text: string }>;
```

Extract plain text from an uploaded resume/job-ad file (pdf, docx, txt, md).

#### `resume.validateContent`

```ts
validateContent(req: ResumeValidateContentRequest): Promise<ContentReportPayload>;
```

Deterministic content-quality checks (factual accuracy, ATS structure,
AI-voice tells) on an already-generated résumé/letter against its source
résumé and the job ad. Pure and fast — no AI call, safe to call on every
save. See `validate::content::validate_content` (Rust, L1).

`req.docKind` must be exactly `'resume'` or `'coverLetter'` — the Zod
`z.enum` here is renderer-side only; the Rust command rejects any other
value with a Validation error rather than guessing which ruleset to run.

### Channels — `resume`

`RESUME_CHANNELS` in `packages/shared/src/ipc/contracts/resume.ts`:

| Key               | Channel                  |
| ----------------- | ------------------------ |
| `extractText`     | `resume:extractText`     |
| `validateContent` | `resume:validateContent` |

### Types — `resume`

Declared in `packages/shared/src/ipc/contracts/resume.ts`.

```ts
/**
 * Wire shape of Rust's `validate::content::ContentReport` — `ContentReport`
 * derives `Serialize` only (its `code` is a `&'static str`), so this is a hand
 * mirror rather than a generated type. `code` is the stable i18n key from
 * `CONTENT_ISSUE_CODES`; `section`/`evidence` serialize as `null`, not omitted
 * (no `skip_serializing_if` on those fields Rust-side).
 */
export interface ContentReportPayload {
  ok: boolean;
  issues: {
    severity: 'critical' | 'warning';
    code: string;
    section: string | null;
    message: string;
    evidence: string | null;
  }[];
  metrics: {
    keywordCoverage: number | null;
    /**
     * How many of the posting's top requirements the document evidences, or
     * `null` when nothing was measured — an uncomparable posting, an empty
     * requirements list, or a cover letter (which never runs the alignment
     * pass). Render the absent value as "—"; a `0` here would claim a
     * measurement that was never taken.
     *
     * Only meaningful next to `topRequirementsMeasured`: a bare "2" reads the
     * same for 2-of-2 and 2-of-10.
     */
    topRequirementHits: number | null;
    /**
     * The denominator for `topRequirementHits`: how many of the posting's top
     * requirements could be measured at all. `null` exactly when
     * `topRequirementHits` is — Rust produces the pair from one `Option` — so
     * one null check covers both. Lower than the requirements list whenever a
     * requirement has no extractable keywords ("Team player!"), and `0` when
     * none of them had any (the analysis produced requirements this kernel
     * cannot check — distinct from "no requirements", which is `null`).
     *
     * Optional in this mirror ONLY so that payload literals written before the
     * field existed keep type-checking; Rust always serializes it
     * (present-and-null, no `skip_serializing_if`). Read it as
     * `metrics.topRequirementsMeasured ?? null` and treat `undefined` as "not
     * measured".
     */
    topRequirementsMeasured?: number | null;
    duplicateRatio: number;
    rolesSource: number;
    rolesOutput: number;
  };
}
```

### Referenced types — `resume`

- `packages/shared/src/schemas/index.ts` — `ResumeValidateContentRequest`

---

## `resumePipeline`

The staged résumé pipeline — one fixed stage sequence; there is no depth choice.

`run` starts the background run and returns its ids immediately; stage
progress streams as `pipeline:stage` events (subscribe via `ResumePipelineContract.onStage`).

**How a run ends — the only two signals that are load-bearing.**

1. **`get(runId).status` is the authority.** The run row is written before
   the first stage and rewritten once at the end, so its terminal
   `completed`/`needsReview`/`failed`/`cancelled` is the fact everything else
   describes. Poll or re-`get` on a stage event; do not derive the outcome.
2. **The umbrella job's `job.failed` covers the runs that never get a row.**
   A résumé or posting that cannot be resolved, no configured provider, or a
   refused admission all fail BEFORE the run row is inserted — `get(runId)`
   then returns `null` forever, and the failure reaches the renderer only as
   the `jobs:event` failure for `jobId`. A surface that consumes stage
   events alone shows such a run as still starting.

**Stage events are progress, not completion.** There is no guaranteed final
`finish`/`error` event: the cancel + deadline check runs in the stage hook's
`before`, ahead of that stage's `start` emit, so a run stopped at a stage
boundary emits NOTHING for the stage it stopped at — the last event the
renderer saw is the PREVIOUS stage's `finish`, which is indistinguishable
from "the next stage is still running". Use them to drive the progress
display only.

**Why the draft stream is not the completion signal either.** The draft stage
streams under the run's own umbrella `jobId` so the user watches the résumé
appear, which means the shared stream machinery marks that job completed the
moment the draft finishes — while validation and up to two repair rounds are
still to come. That stream is DISPLAY-ONLY. A renderer that treats
`awaitAiStream` resolving as "the run is done" will show an unvalidated,
unrepaired draft as final.

Contract: `ResumePipelineContract` in `packages/shared/src/ipc/contracts/resumePipeline.ts`

### Methods — `resumePipeline`

- [`resumePipeline.run`](#resumepipelinerun)
- [`resumePipeline.get`](#resumepipelineget)
- [`resumePipeline.listForJob`](#resumepipelinelistforjob)
- [`resumePipeline.regenerateSection`](#resumepipelineregeneratesection)
- [`resumePipeline.resolveFabrication`](#resumepipelineresolvefabrication)
- [`resumePipeline.onStage`](#resumepipelineonstage)

#### `resumePipeline.run`

```ts
run(req: ResumePipelineRunRequest): Promise<PipelineRunStarted>;
```

Start one staged run. Resolves as soon as the run is admitted (the
concurrency wait happens inside the run, so a queued run still returns its
ids immediately).

#### `resumePipeline.get`

```ts
get(runId: string): Promise<PipelineRunDetail | null>;
```

One run with its full stage trail. `null` for an unknown id.

**`resumeText`/`report` come from the per-job AGGREGATE, not from this
run.** Every run of a posting merges into one `ai_generations` row (keyed by
`jobUrl`), so `listForJob` can legitimately show three runs while only ONE
document exists. An older run's `status`, `metrics`, `events` and
`stoppedReason` are genuinely its own and never change again; its
`resumeText` is whatever the aggregate holds RIGHT NOW. That is also why the
two write calls below refuse an older run outright rather than silently
editing the shared document.

**The document may be newer than the run, and that is not an error.** Four
things move it: a later run, `ResumePipelineContract.regenerateSection`, a re-check save, and the user's
own editing — applying a fabrication "Remove" is an ordinary hand edit
through the editor's save path. When that happens `report` keeps describing
the version it validated and `report.<slot>.sourceTextHash` stops matching
`resumeText`: render that as the existing "checked before your edits" state,
not as a failure, and do not suppress the report. Verdicts already recorded
survive it — `ResumePipelineContract.resolveFabrication` stamps by
`issueKey` inside the stored report and reads no text.

The one thing a WRITER must not do: persist a wrapper whose slot omits
`fabrications`. The backend merges the wrapper per TOP-LEVEL key, so an
incoming `resume` slot replaces the stored one whole — carrying the review
list (and its decisions) forward belongs to whoever saves.

#### `resumePipeline.listForJob`

```ts
listForJob(jobUrl: string): Promise<PipelineRunSummary[]>;
```

The retained runs for one posting, newest first — at most
`RETENTION_RUNS_PER_JOB` (3), which is what the backend keeps.

#### `resumePipeline.regenerateSection`

```ts
regenerateSection(req: ResumePipelineRegenerateSectionRequest): Promise<PipelineRunDetail>;
```

Re-generate ONE section of a finished run and splice it back in, through
the same primitive the repair loop uses. Rejects `"header"` (and anything
else outside the closed `PipelineSectionKey` grammar) at the boundary.

**Only the posting's LATEST run may be written to** (see `ResumePipelineContract.get`): an older `runId` is rejected with a clear
validation error rather than rewriting the newest run's document. It also
goes through the same admission bucket as `run` and is REFUSED (not queued)
when that bucket is full — surface the retriable error, do not auto-retry.

#### `resumePipeline.resolveFabrication`

```ts
resolveFabrication(req: ResumePipelineResolveFabricationRequest): Promise<PipelineRunDetail>;
```

Record the user's Remove/Keep verdict on ONE surviving fabrication finding.
The run stays `needs_review` until every flagged bullet has one.

Same latest-run rule as `ResumePipelineContract.regenerateSection`:
the report being stamped belongs to the aggregate, i.e. to the newest run.

#### `resumePipeline.onStage`

```ts
onStage(handler: (event: PipelineStageEvent) => void): () => void;
```

Subscribe to the `pipeline:stage` progress stream. Returns an unsubscribe fn.

### Channels — `resumePipeline`

`RESUME_PIPELINE_CHANNELS` in `packages/shared/src/ipc/contracts/resumePipeline.ts`:

| Key                  | Channel                             |
| -------------------- | ----------------------------------- |
| `run`                | `resumePipeline:run`                |
| `get`                | `resumePipeline:get`                |
| `listForJob`         | `resumePipeline:listForJob`         |
| `regenerateSection`  | `resumePipeline:regenerateSection`  |
| `resolveFabrication` | `resumePipeline:resolveFabrication` |

`RESUME_PIPELINE_CHANNELS` registers 5 of this namespace's 6 methods; the rest have no entry in it.

### Types — `resumePipeline`

Declared in `packages/shared/src/ipc/contracts/resumePipeline.ts`.

```ts
/** What `run` hands back: the two ids every later call and every event key on. */
export interface PipelineRunStarted {
  /** `pipeline_runs.id` — the key for `get`, `regenerateSection`,
   *  `resolveFabrication`, and every `pipeline:stage` event's `runId`. */
  runId: string;
  /** The umbrella job id — the key for `jobs.cancel`, for `jobs:event`, and
   *  for the draft stage's display-only `ai:stream` deltas. */
  jobId: string;
}

/**
 * How a run ended. `needsReview` is NOT a failure: the document exists and is
 * usable, but the report carries findings the user has to resolve per bullet
 * (see {@link ResumePipelineContract.resolveFabrication}) — a run in this state
 * must never be presented as clean.
 */
export type PipelineRunStatus = 'running' | 'completed' | 'needsReview' | 'failed' | 'cancelled';

/** One run, without its stage trail — the shape the runs list renders. */
export interface PipelineRunSummary {
  runId: string;
  jobUrl: string;
  /** Which flow produced this run. `"resume"` for this contract; the same
   *  tables host other staged runs under other kinds. */
  kind: string;
  depth: GenerationDepth;
  status: PipelineRunStatus;
  startedAt: number;
  /** When the run reached a terminal state. **`null` while it is still
   *  running** — the backend serializes the row's `Option` as an explicit null,
   *  it is not an absent key, so narrow with `!= null` (as `stoppedReason`
   *  does) rather than `!== undefined`. */
  finishedAt?: number | null;
  /**
   * The backend `StoppedReason` wire token (`done`, `run_timeout`,
   * `max_repairs`, `budgeted`, `cancelled`, …).
   *
   * **Absent/`null` in two cases, and `'done'` is never one of them:** while the
   * run is still going, and for a run that FAILED without recording a reason (a
   * provider error, a store failure). `'done'` means the last stage completed —
   * a failure carrying it would read as a clean finish in any suffix map, which
   * is exactly the bug the backend's `terminal_state` exists to prevent. Treat
   * an absent value on a terminal run as "no further detail", never as success:
   * `status` is the authority on how the run ended.
   */
  stoppedReason?: string | null;
  metrics: PipelineRunMetrics;
}

/**
 * Counts and durations only — never generated text (ADR-027).
 *
 * **Every field is absent while the run is going** — the row starts with an
 * empty metrics blob and is written once, at the terminal state — so an absent
 * field means "not known yet", never zero. The two that are additionally
 * `null`-able say so.
 */
export interface PipelineRunMetrics {
  /** Provider round-trips this run actually made. */
  calls?: number;
  /** How many of {@link PipelineRunMetrics.calls} were served from the stage
   *  cache instead of a provider. */
  cached?: number;
  /** Repair rounds run (0–2). */
  repairRounds?: number;
  /** Whether the last repair round was REVERTED because it produced strictly
   *  more criticals than the draft it was trying to fix. */
  reverted?: boolean;
  /** Findings in the terminal report. **`null` when the run never produced
   *  one** (it failed before the validate stage): serialized as an explicit
   *  null, so `!= null` — a `0` here means a clean report, and reading the null
   *  as one would present an unvalidated run as clean. */
  issueCount?: number | null;
  /** Criticals in the terminal report — `0` for a run that produced none AND
   *  for one that never validated, which is why {@link
   *  PipelineRunMetrics.issueCount}, not this, distinguishes the two. Never
   *  null. */
  criticalCount?: number;
  ms?: number;
}

/** One run plus everything needed to render its timeline and its report. */
export interface PipelineRunDetail extends PipelineRunSummary {
  events: PipelineRunEvent[];
  /**
   * The deterministic content report stored for this posting's résumé, and for
   * the cover letter when one was in scope. `null` until the validate stage has
   * run. Aggregate state, not a run snapshot — see {@link
   * ResumePipelineContract.get}; compare `sourceTextHash` with `resumeText`
   * before presenting it as current.
   */
  report: PipelineQualityReport | null;
  /** The posting's résumé as stored right now. Empty until the draft stage
   *  completes, and may carry later edits (see {@link
   *  ResumePipelineContract.get}). */
  resumeText: string;
}

/** One stage event as persisted (the durable twin of `pipeline:stage`). */
export interface PipelineRunEvent {
  seq: number;
  ts: number;
  stage: string;
  phase: 'start' | 'finish' | 'error';
  /** The stage's own small summary payload — counts, section keys, a stopped
   *  reason. Clamped at the store; never the generated document. */
  artifact: unknown;
}

/**
 * The persisted quality-report wrapper as the pipeline writes it into
 * `AiGenerationRecord.qualityReport`. Structurally the renderer's existing
 * `QualityReport` (`schemaVersion: 2`, per-document slots), with two
 * pipeline-only additions the renderer must tolerate:
 *
 * - `pipeline` is `'quality'`/`'max'` here, not only `'fast'`;
 * - a slot may carry `fabrications`, the per-bullet review list.
 */
export interface PipelineQualityReport {
  schemaVersion: 2;
  pipeline: GenerationDepth;
  generatedAt: number;
  resume?: PipelineQualityReportSlot;
  coverLetter?: PipelineQualityReportSlot;
}

/** One document's slot: its report, its staleness anchor, and any surviving
 *  fabrication findings awaiting a per-bullet verdict. */
export interface PipelineQualityReportSlot {
  report: ContentReportPayload;
  /** djb2 hash of the EXACT text `report` validated — the renderer's existing
   *  staleness anchor, computed Rust-side with the identical algorithm. */
  sourceTextHash: number;
  /** Surviving fabrication findings, in report order. Absent when the run
   *  found none. */
  fabrications?: PipelineFabrication[];
}

/** One flagged bullet awaiting (or carrying) the user's verdict. */
export interface PipelineFabrication {
  /** `<code>#<index>` — echo this back as `issueKey`. */
  issueKey: string;
  code: string;
  /**
   * The offending span, verbatim from the generated document.
   *
   * **Not an edit anchor.** A validator span is routinely a bare token
   * (`"250"`, a single keyword), so deleting "every line containing the
   * evidence" deletes whatever else quotes it — anchor a Remove on
   * {@link PipelineFabrication.line} instead and use this only to SHOW the
   * user what was flagged.
   */
  evidence: string;
  /**
   * The full, trimmed text of the first document line carrying `evidence`, as
   * located by the backend against the exact text it validated — the anchor a
   * "Remove" applies to (match a whole trimmed line, not a substring).
   *
   * **Absent when no honest anchor exists**: the span was not found in the
   * document (an entry re-issued over already-edited text), the line is blank,
   * or it is implausibly long (>1 000 chars — a paste artifact, not a bullet).
   * Treat an absent `line` as "cannot apply automatically", never as a licence
   * to fall back to substring deletion.
   */
  line?: string;
  /** `undefined` until the user decides — which is what keeps the run
   *  `needsReview`. */
  decision?: 'remove' | 'keep';
}
```

### Referenced types — `resumePipeline`

- `packages/shared/src/events/pipeline.ts` — `PipelineStageEvent`
- `packages/shared/src/ipc/contracts/resume.ts` — `ContentReportPayload`
- `packages/shared/src/schemas/index.ts` — `GenerationDepth`, `ResumePipelineRegenerateSectionRequest`, `ResumePipelineResolveFabricationRequest`, `ResumePipelineRunRequest`

---

## `scrape`

Contract: `ScrapeContract` in `packages/shared/src/ipc/contracts/scrape.ts`

### Methods — `scrape`

- [`scrape.boards`](#scrapeboards)
- [`scrape.url`](#scrapeurl)
- [`scrape.onProgress`](#scrapeonprogress)
- [`scrape.resolveUrl`](#scraperesolveurl)
- [`scrape.updateDescription`](#scrapeupdatedescription)
- [`scrape.listPostings`](#scrapelistpostings)
- [`scrape.clearPostings`](#scrapeclearpostings)
- [`scrape.listInteractions`](#scrapelistinteractions)
- [`scrape.persistJob`](#scrapepersistjob)

#### `scrape.boards`

```ts
boards(req: ScrapeBoardsRequest): Promise<{ jobId: string }>;
```

#### `scrape.url`

```ts
url(req: ScrapeUrlRequest): Promise<{ jobId: string }>;
```

#### `scrape.onProgress`

```ts
onProgress(handler: (event: ScrapeProgressEvent) => void): () => void;
```

Subscribe to live scrape progress (`scrape:progress`), a coarse
boards-done/total fraction (0..1) emitted after each board finishes.
Returns a sync unsubscribe. Event-only surface, so it has no request
channel in `SCRAPE_CHANNELS` (same shape as `autopilot.onStep`).

#### `scrape.resolveUrl`

```ts
resolveUrl(req: { url: string }): Promise<JobPosting | null>;
```

Resolve a single posting (incl. full description) from its URL.

#### `scrape.updateDescription`

```ts
updateDescription(req: { id: string; description: string }): Promise<boolean>;
```

Write a freshly-resolved full description back into the live postings cache
by posting id, so the match scorer reads the full text instead of the
truncated aggregator snippet. Returns `true` when an entry was updated,
`false` when the id is no longer in the live cache.

#### `scrape.listPostings`

```ts
listPostings(): Promise<JobPosting[]>;
```

#### `scrape.clearPostings`

```ts
clearPostings(): Promise<void>;
```

#### `scrape.listInteractions`

```ts
listInteractions(filter?: { interactionType?: string }): Promise<
    Array<{
      jobId: string;
      interactionType: string;
      timestamp: number;
      title: string;
      company: string;
      url: string;
      source: string;
      location: string;
    }>
  >;
```

#### `scrape.persistJob`

```ts
persistJob(req: { job: Record<string, unknown>; interactionType: string }): Promise<void>;
```

### Channels — `scrape`

`SCRAPE_CHANNELS` in `packages/shared/src/ipc/contracts/scrape.ts`:

| Key                 | Channel                    |
| ------------------- | -------------------------- |
| `boards`            | `scrape:boards`            |
| `url`               | `scrape:url`               |
| `resolveUrl`        | `scrape:resolveUrl`        |
| `updateDescription` | `scrape:updateDescription` |
| `listPostings`      | `scrape:listPostings`      |
| `persistJob`        | `scrape:persistJob`        |
| `clearPostings`     | `scrape:clearPostings`     |
| `listInteractions`  | `scrape:listInteractions`  |

`SCRAPE_CHANNELS` registers 8 of this namespace's 9 methods; the rest have no entry in it.

### Referenced types — `scrape`

- `packages/shared/src/events/scrape.ts` — `ScrapeProgressEvent`
- `packages/shared/src/schemas/index.ts` — `ScrapeBoardsRequest`, `ScrapeUrlRequest`
- `packages/shared/src/types/index.ts` — `JobPosting`

---

## `support`

Contract: `SupportContract` in `packages/shared/src/ipc/contracts/support.ts`

### Methods — `support`

- [`support.exportDiagnostics`](#supportexportdiagnostics)

#### `support.exportDiagnostics`

```ts
exportDiagnostics(
    dest: string
  ): Promise<{ success: true; path: string } | { success: false; error: string }>;
```

Build and save a redacted diagnostics zip to the caller-supplied path

### Channels — `support`

`SUPPORT_CHANNELS` in `packages/shared/src/ipc/contracts/support.ts`:

| Key                 | Channel                     |
| ------------------- | --------------------------- |
| `exportDiagnostics` | `support:exportDiagnostics` |

---

## `system`

Contract: `SystemContract` in `packages/shared/src/ipc/contracts/system.ts`

### Methods — `system`

- [`system.health`](#systemhealth)
- [`system.getVersion`](#systemgetversion)
- [`system.getLocale`](#systemgetlocale)
- [`system.setLocale`](#systemsetlocale)
- [`system.getPlatform`](#systemgetplatform)
- [`system.accentColor`](#systemaccentcolor)
- [`system.openExternal`](#systemopenexternal)
- [`system.setPerformanceMode`](#systemsetperformancemode)
- [`system.getLaunchAtLogin`](#systemgetlaunchatlogin)
- [`system.setLaunchAtLogin`](#systemsetlaunchatlogin)
- [`system.setCloseToTray`](#systemsetclosetotray)
- [`system.getMetrics`](#systemgetmetrics)
- [`system.checkBrowser`](#systemcheckbrowser)
- [`system.openDevtools`](#systemopendevtools)
- [`system.getProtocolVersion`](#systemgetprotocolversion)
- [`system.onAccentChanged`](#systemonaccentchanged)

#### `system.health`

```ts
health(): Promise<RuntimeHealth>;
```

#### `system.getVersion`

```ts
getVersion(): Promise<string>;
```

#### `system.getLocale`

```ts
getLocale(): Promise<Locale>;
```

#### `system.setLocale`

```ts
setLocale(locale: Locale): Promise<void>;
```

#### `system.getPlatform`

```ts
getPlatform(): Promise<string>;
```

#### `system.accentColor`

```ts
accentColor(): Promise<{ supported: boolean; color: string | null }>;
```

Best-effort OS accent color. `supported` is true only where we can read it
(Windows, macOS); elsewhere `color` is null and the renderer keeps the
Default accent. Used by the 'System' accent source in Appearance settings.

#### `system.openExternal`

```ts
openExternal(url: string): Promise<void>;
```

#### `system.setPerformanceMode`

```ts
setPerformanceMode(config: PerformanceBackendConfig): Promise<void>;
```

#### `system.getLaunchAtLogin`

```ts
getLaunchAtLogin(): Promise<boolean>;
```

Whether the app is registered to launch at login (default off).

#### `system.setLaunchAtLogin`

```ts
setLaunchAtLogin(enabled: boolean): Promise<boolean>;
```

Enable/disable launch-at-login; resolves to the resulting OS state.

#### `system.setCloseToTray`

```ts
setCloseToTray(enabled: boolean): Promise<void>;
```

Push the close-to-tray preference to the shell. When enabled, closing the
window hides the app to the tray; when disabled, the window closes / app
quits normally. The renderer's preferences store owns the value (no getter).

#### `system.getMetrics`

```ts
getMetrics(): Promise<AppMetrics>;
```

#### `system.checkBrowser`

```ts
checkBrowser(): Promise<{ detected: boolean; path?: string }>;
```

#### `system.openDevtools`

```ts
openDevtools(): Promise<void>;
```

#### `system.getProtocolVersion`

```ts
getProtocolVersion(): Promise<string>;
```

Returns the IPC protocol version string from the Tauri shell.

#### `system.onAccentChanged`

```ts
onAccentChanged(handler: () => void): () => void;
```

Subscribe to OS accent-color changes (Windows personalization). The shell
emits `system:accentChanged` from a WinRT `UISettings::ColorValuesChanged`
watcher; the renderer re-pulls `accentColor` and re-applies the theme
when the accent source is 'system'. No-op on platforms without a watcher
(macOS/Linux rely on the window-focus refetch fallback). Returns a sync
unsubscribe handle.

### Channels — `system`

`SYSTEM_CHANNELS` in `packages/shared/src/ipc/contracts/system.ts`:

| Key                  | Channel                     |
| -------------------- | --------------------------- |
| `health`             | `system:health`             |
| `getVersion`         | `system:getVersion`         |
| `getLocale`          | `system:getLocale`          |
| `setLocale`          | `system:setLocale`          |
| `getPlatform`        | `system:getPlatform`        |
| `accentColor`        | `system:accentColor`        |
| `openExternal`       | `system:openExternal`       |
| `setPerformanceMode` | `system:setPerformanceMode` |
| `getLaunchAtLogin`   | `system:getLaunchAtLogin`   |
| `setLaunchAtLogin`   | `system:setLaunchAtLogin`   |
| `setCloseToTray`     | `system:setCloseToTray`     |
| `getMetrics`         | `system:getMetrics`         |
| `checkBrowser`       | `system:checkBrowser`       |
| `openDevtools`       | `system:openDevtools`       |
| `getProtocolVersion` | `system:getProtocolVersion` |

`SYSTEM_CHANNELS` registers 15 of this namespace's 16 methods; the rest have no entry in it.

### Referenced types — `system`

- `packages/shared/src/types/index.ts` — `AppMetrics`, `Locale`, `PerformanceBackendConfig`, `RuntimeHealth`

---

## `updater`

Contract: `UpdaterContract` in `packages/shared/src/ipc/contracts/updater.ts`

### Methods — `updater`

- [`updater.check`](#updatercheck)
- [`updater.download`](#updaterdownload)
- [`updater.install`](#updaterinstall)
- [`updater.changelog`](#updaterchangelog)
- [`updater.onStatus`](#updateronstatus)

#### `updater.check`

```ts
check(): Promise<UpdateCheckResult>;
```

Trigger a check. Resolves with the outcome (also emitted on `onStatus`).

#### `updater.download`

```ts
download(): Promise<void>;
```

#### `updater.install`

```ts
install(): Promise<void>;
```

#### `updater.changelog`

```ts
changelog(): Promise<ChangelogResult>;
```

Recent release history (newest first) for the in-app changelog.

#### `updater.onStatus`

```ts
onStatus(handler: (status: unknown) => void): () => void;
```

### Channels — `updater`

`UPDATER_CHANNELS` in `packages/shared/src/ipc/contracts/updater.ts`:

| Key         | Channel             |
| ----------- | ------------------- |
| `check`     | `updater:check`     |
| `download`  | `updater:download`  |
| `install`   | `updater:install`   |
| `changelog` | `updater:changelog` |

`UPDATER_CHANNELS` registers 4 of this namespace's 5 methods; the rest have no entry in it.

### Types — `updater`

Declared in `packages/shared/src/ipc/contracts/updater.ts`.

```ts
/** One release entry surfaced in the in-app changelog. */
export interface ChangelogRelease {
  /** Version without a leading `v` (e.g. `"0.28.0"`). */
  version: string;
  /** Release title, if GitHub has one. */
  name: string | null;
  /** Release notes body (Markdown), if any. */
  body: string | null;
  /** ISO 8601 publish timestamp, if any. */
  publishedAt: string | null;
  /** GitHub release page URL. */
  url: string;
  prerelease: boolean;
}

/** Result of {@link UpdaterContract.changelog}. Never rejects — errors surface here. */
export interface ChangelogResult {
  releases?: ChangelogRelease[];
  error?: string;
}

/** Result of {@link UpdaterContract.check}. Mirrors the shell's `updater_check`
 *  JSON: an available update with its version, no update, or an error string.
 *  `downloaded`/`downloading` are only ever `true` together with `available`
 *  — the backend refuses to discard a download already finished or in
 *  flight, and reports that state back instead of re-fetching, so a
 *  returning caller can re-attach rather than restart. Detailed progress
 *  still arrives via the `updater:status` event stream. */
export type UpdateCheckResult =
  | { available: true; version: string; downloaded?: boolean; downloading?: boolean }
  | { available: false }
  | { error: string };
```
