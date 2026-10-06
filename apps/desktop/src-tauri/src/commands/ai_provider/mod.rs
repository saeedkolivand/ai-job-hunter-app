//! Strictly-typed AI provider layer.
//!
//! Every backend lives in its own client module (`ollama`, `openai`,
//! `anthropic`, `gemini`). Routing is by the `ProviderId` enum — there is **no
//! silent fallback to Ollama**. All Ollama-specific assumptions (host,
//! `/api/*` endpoints) are isolated inside `ollama.rs`.
//!
//! Adding a provider = new client module + one `ProviderId` arm + one `resolve`
//! arm. This keeps OpenRouter / DeepSeek / Azure / Groq / Together / LM Studio /
//! vLLM (all OpenAI-compatible) and future native APIs cheap to add.
//!
//! This file is the trait/registry hub (R8 line-budget split): model
//! capabilities + sampling intent live in [`sampling`], the agentic
//! tool-calling vocabulary + transcript helpers in [`chat`], AI-spend
//! [`usage::Usage`]/[`usage::record_usage`] in [`usage`], the `list_models`
//! projection helpers in [`catalogue`], the embedding vector types +
//! [`embeddings::embed_text`] in [`embeddings`], and HTTP/transport error
//! mapping in [`error_map`] — each re-exported here so every existing call
//! site (`super::X`) keeps compiling unchanged.

use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;

use crate::error::AppResult;
// `AppError` isn't referenced by this module's own (production) code, but
// `anthropic::tests::{list_models, transport}` (batch-3a files, out of this
// split's scope) reach it via `super::super::super::AppError` — keep this
// private re-export test-only so a non-test build never sees it as unused.
#[cfg(test)]
use crate::error::AppError;
pub use crate::ipc_contracts::ai::{AiGenerateRequest, AiGenerateRequestMessage};

mod anthropic;
pub mod cli_agent; // pub: its registry/detection back the CLI-agent health probe
mod embed; // adaptive chunk-and-mean-pool embedding machinery (R8 split — self-contained subsystem, own tests)
mod gemini;
pub mod ollama; // pub: its Ollama-only helpers back the local model list / health / embeddings
mod ollama_cloud;
mod openai;
pub mod provider_id; // ProviderId enum + impls (split to stay under R8 LOC cap)
mod research; // shared company-research prompt spec + helpers used by every `research()`
mod retry; // bounded exponential backoff for the non-streaming complete/embed paths
/// Re-exported for `autopilot::rerank`'s compile-time budget assertion — the
/// re-rank breaker's arithmetic depends on how many attempts one embed spends.
pub(crate) use retry::EMBED_BUDGET_ATTEMPTS;
pub mod search; // web-search backends (the retrieval half of research) — NOT AI providers
pub(crate) mod stream; // shared streaming loop (cancel-check + chunk read + emit + complete) for cloud adapters. `pub(crate)` for its ONE crate-visible item (`is_empty_answer_length_cut`, plus the `#[cfg(test)]` fixture builder that constructs its input); every other item is `pub(super)`/private
mod structured; // `complete_structured`'s prompt-discipline default + the per-provider JSON wire shapes
pub(crate) mod timeouts; // semantically-named per-request HTTP timeouts (pure extraction of the magic-number literals)

mod catalogue;
mod chat;
mod embeddings;
mod error_map;
mod sampling;
mod usage;

pub use catalogue::{model_entry, parse_rfc3339_millis};
pub(crate) use chat::{flatten_messages, single_shot_turn, split_system};
pub use chat::{AgentTurn, ChatMsg, Role, StopReason, ToolCall, ToolSpec};
pub use embeddings::{
    compare, embed_text, EmbeddingSpace, EmbeddingVector, EMBEDDING_VECTOR_VERSION,
};
pub use error_map::{
    emit_stream_error, extract_error_message, finish_provider_result, friendly_api_error,
    map_completion_transport_error, redact_body_for_log, redact_provider_error,
    redact_upstream_text, strip_provider_secrets, strip_secrets_in_place,
};
pub use sampling::{
    resolve_intent, Intent, ModelCapabilities, SamplingProfile, TokenParam,
    DETERMINISTIC_TEMPERATURE, PROSE_FREQUENCY_PENALTY, PROSE_GROUNDED_TEMPERATURE,
    PROSE_PRESENCE_PENALTY, PROSE_REPEAT_PENALTY, PROSE_TEMPERATURE, PROSE_TOP_P,
};
pub(crate) use usage::record_usage;
pub use usage::Usage;

use anthropic::AnthropicClient;
use cli_agent::CliAgentClient;
use gemini::GeminiClient;
use ollama::OllamaClient;
use ollama_cloud::OllamaCloudClient;
use openai::OpenAiClient;

// Re-export ProviderId for public API
pub use provider_id::ProviderId;

mod pagination;
/// The shared cursor-pagination control flow. Lives in its own module (this
/// one is at its R8 LOC cap) but keeps its path here, so no call site moves.
pub use pagination::{advance_cursor, bounded, pagination_step, CursorProgress, PaginationStep};

// ── Provider trait & registry ────────────────────────────────────────────────

/// A chat backend. Object-safe so the registry can return `Box<dyn AiProvider>`.
#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> ProviderId;

    /// Capability matrix for a given model on this provider.
    fn capabilities(&self, model: &str) -> ModelCapabilities;

    /// The reasoning-effort levels this provider currently offers for
    /// `model` — empty when the app has no LEVER to influence this model's
    /// reasoning effort, which is NARROWER than "the model doesn't reason at
    /// all" (`capabilities(model).supports_reasoning`). Those two are
    /// related but NOT a strict mirror: `supports_reasoning` says whether
    /// the model reasons at all (a CLI coding agent always does — it's
    /// `true` uniformly across every backend, see
    /// `cli_agent::CliAgentClient::capabilities`), while `effort_levels`
    /// says whether THIS APP can steer that effort via a request parameter
    /// — a CLI agent's `effort` field is honored by Codex and Claude Code
    /// (`cli_agent::CliAgentClient::effort_levels`), so every other backend
    /// has `supports_reasoning: true` but `effort_levels()` empty. A
    /// property of the provider's wire API otherwise: for a provider whose
    /// accepted level SET is uniform across every reasoning-capable model
    /// this is a fixed list; Gemini's genuinely varies per model tier (see
    /// `gemini::gemini_effort_levels`'s doc comment) so it overrides this
    /// per-model instead. `ai_model_capabilities` surfaces this as
    /// `effortLevels` — the renderer's effort picker gates on THIS method's
    /// result, never on `supportsReasoning` directly, and never a hardcoded
    /// per-provider TS mirror, so a new model/provider needs zero renderer
    /// change. DEFAULT: no reasoning-effort lever (empty) — every provider
    /// that offers one overrides this.
    fn effort_levels(&self, _model: &str) -> Vec<&'static str> {
        Vec::new()
    }

    /// This provider's own sampling numbers for `model` given the renderer's
    /// declared [`Intent`] (see the "Sampling intent" section above and
    /// [`resolve_intent`]) — never raw numbers dictated by the caller.
    /// DEFAULT: [`SamplingProfile::default`], the neutral profile (every field
    /// `None`) — correct-or-better on the modern frontier (Claude 4.7+/5,
    /// OpenAI's reasoning models, Gemini 3.x) and on self-hosted servers that
    /// carry their own defaults (Ollama native, vLLM-style gateways); an
    /// unknown model on an unknown/new provider therefore falls through to
    /// THAT provider's own default, preserving the zero-code-change promise.
    /// `chat_stream` merges this with the request's explicit numeric fields
    /// via [`SamplingProfile::resolve`] — those always win.
    fn sampling_profile(&self, _model: &str, _intent: Intent) -> SamplingProfile {
        SamplingProfile::default()
    }

    /// Stream a chat completion, emitting `ai:stream` deltas and marking the job
    /// complete/failed. Resolves its own API key (isolated auth per provider).
    ///
    /// `req.effort` (`AiGenerateRequest`) is the ONLY path that carries the
    /// user's reasoning-effort setting into a provider call — every adapter's
    /// effort-field wiring lives here. `complete`/`complete_with_usage`/
    /// `chat_with_tools`/`research*` take `system`/`user`/`ChatMsg` directly,
    /// not `AiGenerateRequest`, so the agent tool-calling loop, company/salary
    /// research, and answer research keep the provider's default effort —
    /// deliberately out of scope for the effort feature, not an oversight.
    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()>;

    /// Non-streaming completion: returns the full assistant text in one shot.
    /// Unlike `chat_stream` it emits no `ai:stream` events and never touches the
    /// JobTracker — it's for server-side pipelines (e.g. cover-letter research +
    /// leakage validation) that need the whole response before continuing.
    /// Resolves its own API key, exactly like `chat_stream`.
    async fn complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<String>;

    /// [`complete`](Self::complete) plus the provider's REAL reported token
    /// usage (never estimated) — the non-streaming half of AI-spend
    /// visibility (`crate::spend`), consumed by `pipeline::Completer::complete`.
    /// DEFAULT: wraps `complete` and reports [`Usage::default`] (zero) —
    /// correct for any provider that genuinely reports no usage (a CLI
    /// agent). Providers whose API returns usage (OpenAI, Anthropic, Gemini,
    /// Ollama, Ollama Cloud) override this to parse it from the same
    /// response `complete` already fetches, so there is no duplicate call.
    async fn complete_with_usage(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
        let text = self.complete(app, model, system, user, temperature).await?;
        Ok((text, Usage::default()))
    }

    /// Plain-text non-streaming completion that ALSO carries `req.effort`
    /// (and `max_tokens`/`context_window`) — what
    /// [`complete`](Self::complete)/[`complete_with_usage`](Self::complete_with_usage)
    /// cannot, since they take no [`AiGenerateRequest`]. Text in, text out: no
    /// JSON directive is added (contrast
    /// [`complete_structured`](Self::complete_structured)).
    ///
    /// DEFAULT: [`complete_with_usage`](Self::complete_with_usage) over the
    /// request's flattened slots, effort dropped — correct for a provider
    /// with no effort lever, and the permanent fallback for a new provider
    /// until it opts in. Providers with one override it and gate the effort
    /// through the SAME per-model gate their `chat_stream` uses.
    async fn complete_with_effort(
        &self,
        app: &AppHandle,
        req: &AiGenerateRequest,
    ) -> AppResult<(String, Usage)> {
        let (system, user) = structured::plain_prompt(req);
        self.complete_with_usage(app, &req.model, &system, &user, req.temperature)
            .await
    }

    /// Structured (JSON) completion: the same non-streaming call as
    /// [`complete_with_usage`](Self::complete_with_usage), but asking the model
    /// for ONE JSON value. `schema_hint` is a FILLED EXAMPLE object (not a JSON
    /// Schema) that every path puts in the prompt; `schema` is an optional flat
    /// JSON Schema for the providers that can constrain decoding against one.
    ///
    /// DEFAULT: prompt discipline only — [`structured::prompt_only`] appends a
    /// strict JSON directive + the example to the SYSTEM slot and calls
    /// `complete_with_usage` (so spend recording, retries, tracing and usage
    /// parsing are all unchanged). **This default is the PERMANENT fallback**,
    /// not a stopgap: a CLI agent, an unknown OpenAI-compatible gateway, a
    /// provider with no JSON mode, and a caller with an example but no schema
    /// all land here. No caller may require native constrained output, and a
    /// NEW provider needs zero changes to this method to work.
    ///
    /// Providers with a native constrained-output field override this and use
    /// `schema` when it is present, returning to the default when it is not.
    /// Output is still untrusted (OWASP LLM05): a schema guarantees SHAPE, not
    /// values — every caller must validate (see `crate::pipeline::json`).
    async fn complete_structured(
        &self,
        app: &AppHandle,
        req: &AiGenerateRequest,
        schema_hint: &str,
        _schema: Option<&Value>,
    ) -> AppResult<(String, Usage)> {
        structured::prompt_only(self, app, req, schema_hint).await
    }

    /// Whether this provider's MODEL performs the search itself, so
    /// [`Self::research`] is a single native call.
    ///
    /// The routing question, asked once per research call — inside
    /// `search::CompanySearchRoute::resolve` for company briefs (via
    /// `Completer::resolve_search_route`), and inline in
    /// `Completer::research_salary`/`research_answer` for the other two
    /// facets. False means the search-then-synthesize path runs instead,
    /// which is what lets a provider with no search of its own use a
    /// configured backend.
    ///
    /// Defaults to the advertised capability. The Ollama family overrides it to
    /// `false`: it advertises search for the FAMILY, but the model does not
    /// search — a separate hosted API does, via [`Self::native_searcher`].
    fn has_native_search(&self, model: &str) -> bool {
        self.capabilities(model).supports_web_search
    }

    /// This provider's OWN search backend, when it is usable right now.
    ///
    /// Only the Ollama family implements it: its search is a separate HTTP API
    /// with its own account key, so "can search" is a runtime question. Providers
    /// whose model searches for itself override [`Self::research`] and never
    /// consult this; providers with no search leave both alone and inherit the
    /// fallback below. A NEW provider needs no change either way.
    fn native_searcher(
        &self,
        _app: &AppHandle,
        _model: &str,
    ) -> Option<Box<dyn search::WebSearcher>> {
        None
    }

    /// Produce a ~150-word company-research brief with the provider's OWN
    /// model-side web search.
    ///
    /// Only reached when [`Self::has_native_search`] is true —
    /// `search::CompanySearchRoute::resolve` routes everything else through
    /// [`search::fetch_company_brief`]. Implement this ONLY if the model
    /// searches for itself; a provider that needs an explicit search backend
    /// implements [`Self::native_searcher`] instead and leaves this alone.
    ///
    /// Returns `""` (never an error) when the search finds nothing, so
    /// generation always proceeds. The brief is untrusted reference context —
    /// fenced downstream and never a source of candidate facts.
    async fn research(
        &self,
        _app: &AppHandle,
        _model: &str,
        _company: &str,
        _role: &str,
    ) -> AppResult<String> {
        Ok(String::new())
    }

    /// Web-grounded market salary-range lookup for a role — at a specific
    /// company when the search finds company-specific data, otherwise the
    /// broader market for that role/location — using the **same** web-search
    /// channel as [`research`](Self::research). Must return ONLY a compact
    /// `{"min":…,"max":…,"currency":"…"}` JSON object (or `{}` when nothing
    /// reliable is found); [`crate::salary_research::SalaryResearch`] parses and
    /// strictly validates it before anything reaches a prompt, so raw web text
    /// never does. Returns `""` (never an error) when the provider can't search
    /// or isn't configured — exactly like `research`. Default: no research.
    ///
    /// `country`/`currency` ground the report in the job's actual currency
    /// (resolved client-side from its validated ISO country) — both empty when
    /// the country is unknown, which preserves the unconstrained "local
    /// currency for that location" behavior.
    #[allow(clippy::too_many_arguments)]
    async fn research_salary(
        &self,
        _app: &AppHandle,
        _model: &str,
        _role: &str,
        _company: &str,
        _location: &str,
        _country: &str,
        _currency: &str,
    ) -> AppResult<String> {
        Ok(String::new())
    }

    /// Web-search reference notes to help ground a single application-question
    /// answer — the per-question sibling of [`research`](Self::research), using
    /// the **same** web-search channel. Returns factual notes only, never a
    /// written answer, so the candidate's own résumé-grounded answer is never
    /// shortcut by a fabricated persona; [`crate::commands::ai::ai_research_answer`]
    /// fences the result as untrusted downstream. Returns `""` (never an error)
    /// when the provider can't search or isn't configured — exactly like
    /// `research`. Default: no research.
    async fn research_answer(
        &self,
        _app: &AppHandle,
        _model: &str,
        _question: &str,
        _role: &str,
        _company: &str,
    ) -> AppResult<String> {
        Ok(String::new())
    }

    /// Embed a single text, returning the raw vector. Errors when this provider
    /// has no embeddings API (callers gate on `capabilities().supports_embeddings`).
    async fn embed(&self, app: &AppHandle, model: &str, text: &str) -> AppResult<Vec<f64>>;

    /// [`embed`](Self::embed) plus the provider's REAL reported token usage
    /// (never estimated) — consumed by [`embeddings::embed_text`], the shared
    /// chokepoint for AI-spend visibility on every embedding call (manual
    /// embed, match-score resolution, and `ai_reembed_all`'s batch re-index).
    /// DEFAULT: wraps `embed` and reports [`Usage::default`] (zero) — correct
    /// for a provider whose embeddings response carries no usage field
    /// (Ollama's local embeddings cost $0 anyway; CLI agents have no
    /// embeddings API at all). OpenAI/Gemini override this to parse the real
    /// `usage`/`usageMetadata` field their embeddings response carries.
    async fn embed_with_usage(
        &self,
        app: &AppHandle,
        model: &str,
        text: &str,
    ) -> AppResult<(Vec<f64>, Usage)> {
        let values = self.embed(app, model, text).await?;
        Ok((values, Usage::default()))
    }

    /// The provider's default embedding model, or `None` if it has no embeddings API.
    fn default_embedding_model(&self) -> Option<&'static str>;

    /// Max input length (in **chars**) accepted by this provider's embeddings API,
    /// per CHUNK. `embed_text` (via `embed_adaptive`) splits any longer input at
    /// this boundary (char-safe) into multiple chunks, embeds each, and
    /// mean-pools + L2-normalizes the result — the whole document is always
    /// embedded, never silently truncated away. The default is a conservative
    /// bound that no supported provider's API rejects, so a NEW provider works
    /// with zero code change; providers with larger real limits override upward.
    fn max_embedding_input_chars(&self) -> usize {
        8_000
    }

    /// List the models this provider exposes. Resolves its own credentials/client
    /// (exactly like `chat_stream`/`complete`), so no HTTP/key transport detail
    /// leaks into the trait — a CLI agent has neither and just lists its aliases.
    ///
    /// Each entry is `{name, displayName?, createdAt?, contextLength?}` — see
    /// [`model_entry`] for the exact contract (which fields are optional and
    /// why, and `createdAt`'s normalized unit).
    ///
    /// `Err` on a missing/blank key, a request/transport failure, a non-success
    /// status, or a response body that doesn't carry the expected model-list
    /// field — distinct from `Ok(vec![])`, which means the provider was reached
    /// and genuinely reported an empty catalogue. Callers must not conflate the
    /// two (see `commands::ai::ai_list_provider_models`).
    async fn list_models(&self, app: &AppHandle) -> AppResult<Vec<Value>>;

    /// Validate that the provider is usable: cloud → the stored key authenticates;
    /// local server / CLI agent → reachable / installed. Resolves its own deps from
    /// `app`, returning a clear error when nothing is configured.
    async fn test_key(&self, app: &AppHandle) -> AppResult<()>;

    /// One multi-turn tool-calling turn: given the running transcript and the
    /// tools the caller is willing to expose, return the assistant's text + any
    /// tool calls + the stop reason.
    ///
    /// DEFAULT: no native tool-calling — flatten the transcript to a single prompt
    /// and answer once via [`complete`](Self::complete), returning no tool calls
    /// (`stop = End`). Every provider that does NOT override this (CLI agents,
    /// non-tool models) therefore degrades to a single-shot, non-agentic answer.
    /// Overriding adapters MUST gate on `capabilities(model).supports_tools` and
    /// fall back here when it is false, so an unknown/unsupported model degrades
    /// safely instead of 400-ing on a `tools` field it doesn't understand.
    async fn chat_with_tools(
        &self,
        app: &AppHandle,
        model: &str,
        messages: &[ChatMsg],
        _tools: &[ToolSpec],
        temperature: Option<f64>,
    ) -> AppResult<AgentTurn> {
        single_shot_turn(self, app, model, messages, temperature).await
    }
}

/// Single routing point. `base_url` only applies to OpenAI-compatible servers.
pub fn resolve(id: ProviderId, base_url: Option<String>) -> Box<dyn AiProvider> {
    // CLI agents are routed entirely by the registry — adding one never touches
    // this match.
    if let Some(backend) = cli_agent::backend_for(id) {
        return Box::new(CliAgentClient::new(backend));
    }
    match id {
        ProviderId::Ollama => Box::new(OllamaClient),
        ProviderId::OllamaCloud => Box::new(OllamaCloudClient::new()),
        ProviderId::OpenAi => Box::new(OpenAiClient::new(ProviderId::OpenAi, None)),
        ProviderId::OpenAiCompatible => {
            Box::new(OpenAiClient::new(ProviderId::OpenAiCompatible, base_url))
        }
        ProviderId::Anthropic => Box::new(AnthropicClient),
        ProviderId::Gemini => Box::new(GeminiClient),
        // Routed by the registry above; listed only to keep this match exhaustive
        // (so a new *non*-CLI provider still fails to compile until handled here).
        ProviderId::ClaudeCode
        | ProviderId::Codex
        | ProviderId::GeminiCli
        | ProviderId::Antigravity
        | ProviderId::Opencode
        | ProviderId::Cursor
        | ProviderId::QwenCode => {
            unreachable!("CLI agents are resolved via cli_agent::backend_for")
        }
    }
}

/// Parse + resolve in one step — the single entry point for the
/// renderer-facing probe commands (`ai_test_provider_key`/
/// `ai_list_provider_models`/`ai_model_capabilities`), each of which hands it
/// a `base_url` straight off the wire. Applies the same two `base_url` rules
/// [`crate::ai_config::AiConfigStore::validate_settings`] applies to a
/// *persisted* value, so a probe gets the identical floor: `base_url` is
/// inert for egress on every provider except `OpenAiCompatible` —
/// [`resolve`] itself ignores it elsewhere — so it is dropped to `None`
/// rather than validated (mirrors `validate_settings`' scrub) before a
/// surviving value is checked with
/// [`crate::net::ssrf::validate_provider_base_url`] (rejects a non-`http(s)`
/// scheme, a missing host, or the cloud-metadata IP literal). Without this
/// the probe path — unlike the setter — sent an unvalidated renderer string
/// straight to `resolve`'s network call.
pub fn resolve_by_name(name: &str, base_url: Option<String>) -> AppResult<Box<dyn AiProvider>> {
    let provider_id = ProviderId::parse(name)?;
    let base_url = if matches!(provider_id, ProviderId::OpenAiCompatible) {
        base_url
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty())
    } else {
        None
    };
    if let Some(ref u) = base_url {
        crate::net::ssrf::validate_provider_base_url(u)?;
    }
    Ok(resolve(provider_id, base_url))
}

/// Raw cosine similarity — re-exported from the shared L0 [`crate::vector`]
/// module so [`embeddings::compare`] and every existing `ai_provider::cosine`
/// caller keep the same path, while `scraping::cluster` reuses the SAME
/// implementation for cross-board dedup without an upward layer import
/// (architecture rule R7). Prefer [`embeddings::compare`] for stored vectors
/// so embedding spaces are checked first.
pub use crate::vector::cosine;

mod trace;
/// Per-request `[ai] → / ←` tracing. Lives in its own module (this one is at its
/// LOC cap) but keeps its path here, so no call site moves.
pub use trace::RequestTrace;

#[cfg(test)]
mod tests;
