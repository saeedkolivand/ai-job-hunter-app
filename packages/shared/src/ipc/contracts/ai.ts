import type { AiGenerateRequest, ModelInspectResult } from '../../schemas/index.js';
import type { AiStreamChunk } from '../../types/index.js';

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

export interface AiContract {
  generate(req: AiGenerateRequest): Promise<{ jobId: string }>;

  /**
   * The backend-owned active AI *generation* provider config — the single source
   * of truth for which provider/model/baseUrl generation routes to (task #16).
   * The renderer reads this (never the request) so it can no longer point
   * generation at an arbitrary endpoint. `providers` is always present (maybe
   * empty); `activeProvider`/`model`/`baseUrl` are absent when unseeded.
   */
  activeConfig(): Promise<ActiveAiConfig>;

  /** Switch the active provider (the "switch" half of the switch-vs-edit split).
   *  Returns the fresh active config, or `{ error }` on an invalid id. */
  setActiveProvider(req: { provider: string }): Promise<ActiveAiConfig | { error: string }>;

  /**
   * Edit a (possibly non-active) provider's model/base_url/context window
   * without flipping the active provider (the "edit" half). Returns the fresh
   * active config, or `{ error }` when server-side validation rejects any of
   * them — including against fields it did NOT send (the merged row is what is
   * validated, so patching in a cross-family model still fails).
   *
   * **PATCH semantics, per field:**
   * - **omitted** → keep whatever is stored. Send only what changed.
   * - **`null`** → clear the stored value.
   * - **a value** → set it.
   *
   * Replace-everything was the first design and it erased fields at three call
   * sites before the day was out; absence is what a caller produces by
   * accident, so absence is the harmless case.
   *
   * `contextWindow` is the window for the MODEL in this entry, so send it
   * alongside a model change and let it be `null` when the new model has none.
   */
  setProviderSettings(req: {
    provider: string;
    model?: string | null;
    baseUrl?: string | null;
    /** 512–131072, validated server-side. Only Ollama acts on it (`num_ctx`). */
    contextWindow?: number | null;
  }): Promise<ActiveAiConfig | { error: string }>;

  /** One-time first-run seed from the renderer's migrated Zustand config.
   *  Row-presence gated server-side, so re-calls are safe no-ops. */
  seedActiveConfig(req: {
    config: AiConfigSnapshot;
  }): Promise<{ seeded: boolean } | { error: string }>;

  /**
   * Every explicitly-set per-stage model override, keyed by pipeline stage
   * name (the `PIPELINE_STAGES` vocabulary from `@ajh/shared`).
   *
   * **Absent means the active provider.** A stage with no entry is not
   * "overridden to the default" — it was never configured, and the backend
   * resolves it through the normal active-config path. Render the difference:
   * a suggested value must be shown as a suggestion until the user applies it,
   * because nothing here is applied on the user's behalf.
   */
  stageOverrides(): Promise<Record<string, AiStageOverride>>;

  /**
   * Point ONE stage at a provider + model. Returns the fresh override map, or
   * `{ error }` when server-side validation rejects the stage name, the
   * provider, the model (cross-family check) or the context window
   * (512–131072).
   *
   * `model` may be empty ONLY for a CLI-agent provider, which runs on its own
   * configured default.
   *
   * No base URL: the stage uses the one stored for `provider` — see
   * {@link AiStageOverride}. Change it in that provider's settings and every
   * override on it follows.
   */
  setStageOverride(req: {
    stage: string;
    provider: string;
    model?: string;
    contextWindow?: number;
  }): Promise<Record<string, AiStageOverride> | { error: string }>;

  /** Return ONE stage to the active provider. A no-op for a stage that has no
   *  override. Returns the fresh override map. */
  clearStageOverride(req: {
    stage: string;
  }): Promise<Record<string, AiStageOverride> | { error: string }>;

  /**
   * Stream a generation through the backend orchestration pipeline. Same wire
   * shape as `generate`, but the work runs as a composable `Pipeline` (so feature
   * generators share one lifecycle). Used by resume/cover-letter generation.
   */
  generatePipeline(req: AiGenerateRequest): Promise<{ jobId: string }>;

  onStream(handler: (chunk: AiStreamChunk) => void): () => void;

  listModels(): Promise<Array<{ name: string }>>;

  /**
   * Inspect a local (Ollama) model's real context window + size via `/api/show`,
   * to suggest safe generation limits. Returns `null` for non-local providers or
   * an unreachable Ollama server.
   */
  inspectModel(req: { model: string }): Promise<ModelInspectResult | null>;

  /**
   * Research the company named in a job ad and return a short factual brief —
   * used by the cover-letter "fit" paragraph and company-specific application
   * answers. Reuses the shared enricher: the active provider's own web search
   * (native tool, or the Ollama Web Search API for Ollama), cached. Degrades
   * gracefully — an empty brief, never an error, when the provider can't search
   * or the search fails. The brief is reference context only; the prompt treats
   * it as untrusted.
   */
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

  /**
   * Web-grounded market salary-range lookup for the salary application
   * question. Reuses the active provider's own web search (same channel as
   * `researchCompany`), parsed and strictly validated server-side, cached.
   * Degrades gracefully — `null`, never an error, when the provider can't
   * search, the search yields nothing reliable, or times out. Only validated
   * integers + a sanitized currency code are ever returned; no raw web text
   * crosses this boundary.
   */
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

  /**
   * Best-effort, per-question web-search reference notes for an application
   * answer — opt-in sibling of `researchCompany`, scoped to a single
   * question's topic (combines it with the role + company for relevance)
   * rather than a general company brief. Reuses the same backend enricher
   * channel: the active provider's own web search (native tool, or the Ollama
   * Web Search API), gated on the provider's actual search support. Degrades
   * gracefully — an empty string, never an error, when the provider can't
   * search or the search fails, so answer generation always proceeds exactly
   * as without web search. The notes are reference context only; the prompt
   * layer fences them as untrusted and never lets them write the answer.
   */
  researchAnswer(req: { question: string; role?: string; company?: string }): Promise<string>;

  pullModel(model: string): Promise<{ jobId: string }>;

  unloadModel(model: string): Promise<void>;

  /** Synchronous embedding — returns the vector, or `{ error }` on any provider/config
   *  failure (a context-length overflow, a missing key, an unreachable host, …). */
  embed(req: {
    text: string;
    model?: string;
  }): Promise<
    { vector: number[]; dim: number; provider: string; model: string } | { error: string }
  >;

  /** Store an API key for a cloud AI provider in the OS keychain. */
  setProviderKey(req: { provider: string; apiKey: string }): Promise<{ success: boolean }>;

  /** Remove a stored provider API key from the OS keychain. */
  removeProviderKey(req: { provider: string }): Promise<{ success: boolean }>;

  /** Check whether a provider API key is stored (does not return the key). */
  hasProviderKey(req: { provider: string }): Promise<{ has: boolean }>;

  /**
   * Test whether a stored provider API key is valid by making a lightweight API call.
   * `baseUrl` is forwarded for OpenAI-compatible servers (LM Studio, vLLM, etc.).
   */
  testProviderKey(req: {
    provider: string;
    baseUrl?: string;
  }): Promise<{ success: boolean; error?: string }>;

  /**
   * Fetch available models from a cloud provider using its stored API key.
   * `baseUrl` is forwarded for OpenAI-compatible servers.
   */
  listProviderModels(req: { provider: string; baseUrl?: string }): Promise<ProviderModelInfo[]>;

  /**
   * Capability probe for a provider/model. Network-free, but NOT static: it
   * reads stored credentials to answer `supportsWebSearch` — whether it can
   * attempt a web-grounded company/role search, whether it accepts a
   * reasoning-effort value, and (when it does) exactly which levels this MODEL
   * accepts. Reads the Rust `ModelCapabilities` matrix + `AiProvider::effort_levels`
   * (the same values the backend gates `research*` and each adapter's own
   * effort field on), so the renderer never mirrors the per-provider/per-model
   * vocabulary and a new provider or model needs zero TS change — some
   * providers' accepted level SET genuinely varies by model tier (Gemini), not
   * just by provider, which is why this is a per-model lookup rather than a
   * static per-provider list. Drives the capability-driven default of the
   * tailoring "search company" toggle, and the Settings → AI effort picker
   * (which renders exactly the `effortLevels` this returns). Unknown/
   * unresolvable providers degrade to `supportsWebSearch: false`,
   * `supportsReasoning: false`, `effortLevels: []`. `baseUrl` is forwarded for
   * OpenAI-compatible servers.
   *
   * `supportsWebSearch` is a CONFIGURATION answer, not a capability one: true
   * when a search backend can actually serve research — the provider's own, or
   * the configured fallback. A provider that advertises search but has no key
   * for it reads false, because the brief it would produce is empty.
   */
  modelCapabilities(req: {
    provider: string;
    model?: string;
    baseUrl?: string;
  }): Promise<{ supportsWebSearch: boolean; supportsReasoning: boolean; effortLevels: string[] }>;

  /** Active embedding space, per-space vector counts, and document index coverage. */
  embeddingStatus(): Promise<EmbeddingStatus>;

  /**
   * Set the active embedding provider/model. An empty model resolves to the
   * provider's default. Changing it changes the embedding space — call
   * `reembedAll` afterwards to rebuild the index.
   */
  setEmbeddingConfig(req: {
    provider: string;
    model?: string;
    baseUrl?: string;
  }): Promise<{ success: boolean; error?: string; config?: EmbeddingConfig }>;

  /** Re-embed every document with the active embedding config. Returns a job id. */
  reembedAll(): Promise<{ jobId: string }>;
  /**
   * Index ONLY the documents with no usable vector in the active embedding
   * space — a newly imported résumé, or everything after the provider/model
   * changed. Backs the `autoIndexOnUpload` preference.
   *
   * Not `reembedAll`: that re-embeds every document unconditionally, which would
   * re-bill a cloud embedding provider for already-indexed documents each time
   * one new file is added.
   *
   * `jobId` is `null` when nothing was stale, so the caller can stay silent
   * rather than show progress for a no-op.
   */
  indexStaleDocuments(): Promise<{ jobId: string | null }>;

  /**
   * Read-only AI-spend summary: today's REAL per-provider token totals — as
   * reported by each provider's own response, never estimated — plus an
   * ESTIMATED USD cost from a static list-price rate table. The dollar
   * figure is a best-effort ballpark (BYO-key users have no billing API to
   * query), not a billing-accurate source. Local (Ollama) and CLI-agent
   * calls always cost $0.
   */
  spendSummary(): Promise<AiSpendSummary>;
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

export const AI_CHANNELS = {
  generate: 'ai:generate',
  listModels: 'ai:listModels',
  pullModel: 'ai:pullModel',
  unloadModel: 'ai:unloadModel',
  embed: 'ai:embed',
} as const;
