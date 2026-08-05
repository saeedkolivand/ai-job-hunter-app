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

use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::events::{emit_event, AiStreamChunk, AiStreamChunkError, AI_STREAM};
pub use crate::ipc_contracts::ai::{AiGenerateRequest, AiGenerateRequestMessage};

mod anthropic;
pub mod cli_agent; // pub: its registry/detection back the CLI-agent health probe
mod embed; // adaptive chunk-and-mean-pool embedding machinery (R8 split — self-contained subsystem, own tests)
mod gemini;
pub mod ollama; // pub: its Ollama-only helpers back the local model list / health / embeddings
mod ollama_cloud;
mod openai;
mod research; // shared company-research prompt spec + helpers used by every `research()`
mod retry; // bounded exponential backoff for the non-streaming complete/embed paths
mod stream; // shared streaming loop (cancel-check + chunk read + emit + complete) for cloud adapters
mod timeouts; // semantically-named per-request HTTP timeouts (pure extraction of the magic-number literals)

use anthropic::AnthropicClient;
use cli_agent::CliAgentClient;
use embed::{embed_adaptive, ProviderEmbedAttempt};
use gemini::GeminiClient;
use ollama::OllamaClient;
use ollama_cloud::OllamaCloudClient;
use openai::OpenAiClient;

// ── Provider identity ─────────────────────────────────────────────────────────

/// Every supported provider. Stringly-typed provider checks are banned in favor
/// of this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderId {
    Ollama,
    /// Ollama Cloud — hosted Ollama models over its OpenAI-compatible endpoint
    /// (`ollama.com/v1`). Chat reuses the OpenAI client; the same account key
    /// (`ai:ollama-cloud`) also powers Ollama Web Search for company research.
    OllamaCloud,
    OpenAi,
    /// Any OpenAI-compatible server (LM Studio, vLLM, OpenRouter, Groq,
    /// Together, DeepSeek, Azure-style gateways…) addressed via a custom base URL.
    OpenAiCompatible,
    Anthropic,
    Gemini,
    /// Anthropic Claude Code CLI run headless (a [`cli_agent`] backend). Local +
    /// keyless: authenticates with the user's own Claude Code login.
    ClaudeCode,
    /// OpenAI Codex CLI run headless (a [`cli_agent`] backend). Keyless: uses the
    /// user's ChatGPT login or `OPENAI_API_KEY`.
    Codex,
    /// Google Gemini CLI run headless (a [`cli_agent`] backend) — distinct from the
    /// cloud [`Gemini`](Self::Gemini) API. Keyless: uses the user's Google login.
    GeminiCli,
    /// Google Antigravity CLI (`agy`) run headless (a [`cli_agent`] backend).
    /// Keyless: uses `agy`'s own Google sign-in. **UNVERIFIED** — implemented to
    /// the documented CLI contract but not runtime-tested (see `cli_agent::antigravity`).
    Antigravity,
}

impl ProviderId {
    /// Parse a wire string. Unknown values are a hard error — never a fallback.
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "ollama" => Ok(Self::Ollama),
            "ollama-cloud" => Ok(Self::OllamaCloud),
            "openai" => Ok(Self::OpenAi),
            "openai-compatible" => Ok(Self::OpenAiCompatible),
            "anthropic" => Ok(Self::Anthropic),
            "gemini" => Ok(Self::Gemini),
            "claude-code" => Ok(Self::ClaudeCode),
            "codex" => Ok(Self::Codex),
            "gemini-cli" => Ok(Self::GeminiCli),
            "antigravity" => Ok(Self::Antigravity),
            other => Err(AppError::Config(format!(
                "Unknown AI provider '{other}'. Select a configured provider in Settings → AI."
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::OllamaCloud => "ollama-cloud",
            Self::OpenAi => "openai",
            Self::OpenAiCompatible => "openai-compatible",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::GeminiCli => "gemini-cli",
            Self::Antigravity => "antigravity",
        }
    }

    /// Credential-store key suffix (`ai:<key>`). Ollama needs none.
    pub fn credential_key(&self) -> &'static str {
        self.as_str()
    }

    /// Whether this provider runs locally (no API key, no outbound cloud call):
    /// the Ollama server or any CLI agent.
    #[allow(dead_code)]
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Ollama) || self.is_cli_agent()
    }

    /// Whether this provider is a headless CLI agent (Claude Code, …).
    pub fn is_cli_agent(&self) -> bool {
        cli_agent::backend_for(*self).is_some()
    }

    /// Guard against picking a model that clearly belongs to a *different*
    /// provider (a likely UI mistake). Deliberately permissive otherwise:
    /// **unknown / newly-released model names are allowed**, so the app adopts new
    /// models with no code change. Ollama, OpenAI-compatible (OpenRouter serves
    /// `anthropic/…` and `google/…` models!), and CLI agents accept any name.
    pub fn validate_model(&self, model: &str) -> AppResult<()> {
        let m = model.trim().to_ascii_lowercase();
        if m.is_empty() {
            // CLI agents fall back to the tool's own configured default model.
            if self.is_cli_agent() {
                return Ok(());
            }
            return Err(AppError::Config(
                "No model selected for the active provider.".to_string(),
            ));
        }
        let looks_anthropic = m.starts_with("claude");
        let looks_gemini = m.starts_with("gemini") || m.starts_with("models/gemini");
        let looks_openai = m.starts_with("gpt")
            || m.starts_with("chatgpt")
            || m.starts_with("o1")
            || m.starts_with("o3")
            || m.starts_with("o4");

        let mismatch = || {
            Err(AppError::Validation(format!(
                "Model '{model}' looks like another provider's model, but the active provider is {}. \
                 Pick a matching model or switch providers.",
                self.as_str()
            )))
        };

        // Only reject a model that unambiguously belongs to a *different* native
        // cloud family — never reject a merely-unrecognized name, so new releases
        // work without a code change.
        match self {
            Self::Anthropic if looks_openai || looks_gemini => mismatch(),
            Self::Gemini if looks_anthropic || looks_openai => mismatch(),
            Self::OpenAi if looks_anthropic || looks_gemini => mismatch(),
            _ => Ok(()),
        }
    }
}

// ── Model capabilities ─────────────────────────────────────────────────────────

/// Which token-limit field a model's API expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenParam {
    MaxTokens,
    MaxCompletionTokens,
    NumPredict,
    MaxOutputTokens,
}

/// Per-model feature matrix. All provider/model-specific behavior lives here so
/// the request builders never special-case providers inline. Some flags are
/// declared ahead of their consumers (tools / JSON mode / embeddings) to keep
/// adding capability-gated features cheap.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct ModelCapabilities {
    pub supports_temperature: bool,
    pub supports_system_role: bool,
    pub supports_streaming: bool,
    pub supports_reasoning: bool,
    pub supports_tools: bool,
    pub supports_json_mode: bool,
    pub supports_embeddings: bool,
    /// Whether this provider/model can attempt a `research*` web search at
    /// all — a static, network-free check distinct from whether a search
    /// actually succeeds (which also depends on a configured account key,
    /// checked at call time). Lets callers that fan out a research call per
    /// item (e.g. `ai_research_answer`, one call per selected question) skip
    /// the daily-budget charge entirely for a provider that can never search,
    /// instead of charging N times for N guaranteed-empty results.
    pub supports_web_search: bool,
    pub token_param: TokenParam,
}

// ── Agentic tool-calling (Phase 1 foundation) ───────────────────────────────
//
// Shared vocabulary for multi-turn tool-calling. A `ToolSpec` is the schema handed
// to the model; a `ToolCall` is what the model asks to run; an `AgentTurn` is one
// assistant response (text + any tool calls + why it stopped); `ChatMsg` is the
// running transcript.
//
// SECURITY INVARIANT: only `Role::System` carries trusted, fixed instructions.
// The user's question and (untrusted) tool results ride in `User`/`Tool` turns and
// are NEVER merged into the system prompt or a tool description — the controller
// in `crate::agent` enforces this.

/// A tool offered to the model: name, a natural-language description, and a
/// JSON-Schema object describing its arguments. Provider-agnostic; each adapter
/// maps it to that vendor's function/tool shape.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

/// One tool invocation the model asked for. `args` is already-decoded JSON — each
/// adapter parses the vendor's string/object argument form into a `Value`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

/// Why a provider ended a turn. `ToolUse` means the model wants tool results back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    End,
    ToolUse,
    Length,
    Other,
}

/// One assistant turn: visible text, any tool calls, the stop reason, and the
/// provider's REAL reported token usage for this turn (zero when a provider
/// genuinely reports none — a CLI agent, or a `single_shot_turn` fallback
/// against one that does). Consumed by `pipeline::Completer::chat_with_tools`
/// to record AI spend for the agent controller's tool-calling turns —
/// plausibly the biggest paid-token consumer, since one agent run fans out
/// into several turns.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentTurn {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub stop: StopReason,
    pub usage: Usage,
}

/// Transcript role. `System` is trusted + fixed; every other role is untrusted data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    /// Wire role string shared by the OpenAI / Ollama chat shapes. `Tool` results
    /// fold into a `user` turn (already fenced by the controller) so no adapter
    /// needs native tool-call-id linkage in Phase 1. `pub(crate)` (wider than
    /// this module's descendants) so `agent::controller`'s tests can assert
    /// wire-alternation against the real mapping instead of a duplicate.
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User | Role::Tool => "user",
            Role::Assistant => "assistant",
        }
    }
}

/// One message in the running agent transcript.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatMsg {
    pub role: Role,
    pub content: String,
}

impl ChatMsg {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
    pub fn tool(content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
        }
    }
}

// ── Spend visibility (real token usage) ─────────────────────────────────────

/// Real per-call token usage as reported by the provider's own response —
/// never estimated. Zero on both fields when a provider genuinely reports no
/// usage (e.g. a CLI agent — see `cli_agent`, which relies on the
/// [`AiProvider::complete_with_usage`] default rather than fabricating a
/// number). Consumed by `crate::spend` to compute an estimated dollar cost.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Record one AI call's REAL token usage against today's spend via the
/// managed [`crate::spend::SpendStore`], if one is present. Best-effort:
/// spend tracking never blocks or fails a generation — a missing store (e.g.
/// it failed to open at startup) is silently skipped, exactly like the other
/// `try_state`-gated convenience writers in this crate (see
/// `commands::notifications::push_and_notify`). `base_url` is whatever base
/// URL the caller resolved the request against — passed straight through to
/// [`crate::spend::SpendStore::record`]'s free/paid cost gate, which only
/// ever consults it for the `openai-compatible` provider id (every other
/// provider ignores it), so a local LM Studio/llama.cpp/vLLM server never
/// shows a fake dollar figure. Pass `None` when no base URL was resolved
/// (every non-`openai-compatible` provider).
///
/// Lives HERE (the command/shell layer, L3) rather than in `crate::spend`
/// (a data-layer store, L1) because it needs `AppHandle`/`Manager` to resolve
/// the managed state — the architecture boundary test (R2: no Tauri below the
/// shell layer) forbids a store module from importing `tauri::*` itself.
/// `crate::spend::SpendStore` stays Tauri-free; this is the AppHandle→
/// `try_state`→`record` hop every call site (streaming, `Completer`, CLI
/// agents, `embed_text`) goes through.
pub(crate) fn record_usage(
    app: &AppHandle,
    provider: &str,
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
    base_url: Option<&str>,
) {
    if let Some(store) = app.try_state::<crate::spend::SpendStore>() {
        store.record(crate::spend::SpendRecord {
            provider: provider.to_string(),
            model: model.to_string(),
            input_tokens,
            output_tokens,
            run_id: None,
            base_url: base_url.map(str::to_string),
        });
    }
}

// ── Model catalogue (`list_models`) ─────────────────────────────────────────────

/// Build one `list_models` entry: `{name, displayName?, createdAt?,
/// contextLength?}`. `name` is the canonical id everything selects on — a
/// stored model preference matches against it, so its shape/value must never
/// change here. Every other field is `None`-able because no single provider
/// endpoint returns all of them (see each adapter's `list_models`/
/// `parse_model_page` for exactly which it supplies) — a provider that omits
/// a field passes `None`, which is skipped entirely from the JSON, never a
/// fabricated zero/empty-string/"unknown" sentinel. The renderer treats
/// absent as absent.
///
/// `created_at_ms` is unix epoch MILLISECONDS — the SAME convention every
/// other timestamp field in this codebase already uses (`captured_at`,
/// `last_updated`, …: `chrono::Utc::now().timestamp_millis()`), not any
/// provider's native wire format (Anthropic ships an RFC3339 string, OpenAI a
/// unix-epoch-SECONDS integer, Ollama an RFC3339-with-offset string) — see
/// [`parse_rfc3339_millis`] for the RFC3339 → millis half of that
/// normalization. Chosen over keeping each provider's native representation
/// so the renderer sorts numerically with zero per-provider branching.
pub fn model_entry(
    name: &str,
    display_name: Option<&str>,
    created_at_ms: Option<i64>,
    context_length: Option<i64>,
) -> Value {
    let mut entry = json!({ "name": name });
    if let Some(d) = display_name {
        entry["displayName"] = json!(d);
    }
    if let Some(c) = created_at_ms {
        entry["createdAt"] = json!(c);
    }
    if let Some(l) = context_length {
        entry["contextLength"] = json!(l);
    }
    entry
}

/// Parse an RFC3339 timestamp (Anthropic's `created_at`, Ollama's
/// `modified_at` — both may carry a non-UTC offset, e.g. Ollama's
/// `-07:00`) into unix epoch milliseconds. `None` on any parse failure —
/// never a fabricated/zero timestamp; a parse failure is treated exactly
/// like the field being absent.
pub fn parse_rfc3339_millis(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

// ── Cursor-paginated `list_models` (shared by every adapter that paginates) ─────
//
// Anthropic (`after_id`) and Gemini (`pageToken`) both cursor-paginate
// `list_models` with the identical control flow — this ONE copy is that flow,
// generic over the cursor type (`String` for both today). Living here once
// means a future hardening (e.g. a stricter progress guard) can't apply to
// one adapter and silently not the other, which is exactly the defect class
// this codebase keeps re-discovering (see `docs/knowledge/automation-domain.md`
// / the PR history around this feature).

/// Outcome of one paginated `list_models` iteration — see [`pagination_step`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaginationStep<T> {
    /// Fetch another page with this cursor.
    Continue(T),
    /// A genuine stopping point: [`advance_cursor`] reports NO continuation
    /// value present at all. The caller returns its accumulated results as
    /// `Ok`. Reserved STRICTLY for this case — never for a stalled cursor
    /// (see [`Self::Stalled`]), which looks the same from "did the loop
    /// stop" alone but means the opposite thing.
    Done,
    /// The provider reported another page exists (a non-empty cursor) but
    /// handed back the SAME cursor that fetched the page just parsed —
    /// neither a clean end-of-pages nor genuine progress. A prior fix
    /// stopped the loop here to avoid an infinite re-fetch, but folding this
    /// into [`Self::Done`] converted a hang into silent truncation: the
    /// caller must reject, not return the partial catalogue as `Ok`.
    Stalled,
    /// Ran out of the caller's page budget while the cursor was STILL
    /// genuinely advancing — there IS more catalogue this fetch didn't
    /// cover. The caller must reject rather than silently return an
    /// incomplete list.
    Incomplete,
}

/// Whether a freshly-reported cursor represents genuine pagination progress
/// from `current` — the three-way outcome [`pagination_step`] builds its
/// page-budget check on top of. Pure so the progress-guard is unit-testable
/// without a network mock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorProgress<T> {
    /// No cursor at all — a clean end-of-pages.
    Done,
    /// A cursor IS present, but it's identical to `current` — see
    /// [`PaginationStep::Stalled`].
    Stalled,
    /// A genuinely new cursor — safe to continue.
    Continue(T),
}

/// Compare a freshly-reported `next` cursor against `current` (the one that
/// fetched the page just parsed). `None` (no cursor at all) and "the same
/// cursor came back" are DIFFERENT outcomes — the former is a clean stop,
/// the latter means the provider claims more data exists but gave no way to
/// reach it, which must surface as an error rather than being silently
/// treated as "done".
pub fn advance_cursor<T: PartialEq>(current: &Option<T>, next: Option<T>) -> CursorProgress<T> {
    match next {
        None => CursorProgress::Done,
        Some(next) if current.as_ref() == Some(&next) => CursorProgress::Stalled,
        Some(next) => CursorProgress::Continue(next),
    }
}

/// One step of a page-budget-bounded pagination loop's control flow: given
/// the 0-based index of the page JUST fetched, the caller's own page-budget
/// bound (each adapter's `MAX_LIST_MODELS_PAGES`), and the raw `next` cursor
/// that page reported, decide whether to continue, stop cleanly, stop
/// stalled, or stop incomplete. Pure (no I/O) so the exact page-budget
/// boundary AND the stalled-cursor guard are unit-testable without live HTTP
/// round-trips — each adapter's `list_models` loop calls this once per page
/// and dispatches on the result, so this function (not a hand-duplicated
/// copy per adapter) is what actually runs in production.
pub fn pagination_step<T: PartialEq>(
    page_index: usize,
    max_pages: usize,
    current: &Option<T>,
    next: Option<T>,
) -> PaginationStep<T> {
    match advance_cursor(current, next) {
        CursorProgress::Done => PaginationStep::Done,
        CursorProgress::Stalled => PaginationStep::Stalled,
        CursorProgress::Continue(id) if page_index + 1 < max_pages => PaginationStep::Continue(id),
        CursorProgress::Continue(_) => PaginationStep::Incomplete,
    }
}

/// Race `fut` against the cumulative pagination `deadline`, converting a
/// timeout into `AppError::Network`. Used for EVERY network I/O step of a
/// paginated `list_models` fetch — the send, the error-body read, and the
/// JSON parse — not just the initial `send()`. Wrapping only `send()` was
/// the exact gap that let a stalled body read blow straight through
/// `LIST_MODELS_TOTAL`: `send()` resolves once headers arrive, so a
/// provider whose BODY is slow to arrive after that point escaped a
/// deadline that only covered the send. One shared wrapper for every step
/// means a future 4th I/O call in this loop can't repeat that gap by
/// omission.
pub async fn bounded<F: std::future::Future>(
    deadline: tokio::time::Instant,
    provider: &str,
    fut: F,
) -> AppResult<F::Output> {
    tokio::time::timeout_at(deadline, fut)
        .await
        .map_err(|_elapsed| {
            AppError::Network(format!(
                "{provider}: timed out listing models across multiple pages"
            ))
        })
}

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
    /// — a CLI agent's `effort` field is honored only by Codex
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

    /// Produce a ~150-word company-research brief using **this provider's own**
    /// web search — a native search tool (OpenAI/Anthropic/Gemini), the agent's
    /// own web tools (CLI agents), or the Ollama Web Search API (Ollama family).
    /// Returns `""` (never an error) when the provider can't search or isn't
    /// configured, so research degrades gracefully and generation always proceeds.
    /// Default: no research. The brief is untrusted reference context — fenced
    /// downstream and never a source of candidate facts.
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
    /// (never estimated) — consumed by [`embed_text`], the shared chokepoint
    /// for AI-spend visibility on every embedding call (manual embed,
    /// match-score resolution, and `ai_reembed_all`'s batch re-index).
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

/// Flatten a transcript to a `(system, user)` pair for the single-shot fallback:
/// `system` is every `Role::System` message concatenated (trusted, fixed);
/// everything else — the user question plus any prior assistant/tool turns
/// (already fenced) — is concatenated with role labels into the user prompt, so
/// untrusted content never lands in the system slot. Pure + unit-tested.
pub(crate) fn flatten_messages(messages: &[ChatMsg]) -> (String, String) {
    let system = messages
        .iter()
        .filter(|m| m.role == Role::System)
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let user = messages
        .iter()
        .filter(|m| m.role != Role::System)
        .map(|m| match m.role {
            Role::Assistant => format!("Assistant: {}", m.content),
            Role::Tool => format!("Tool result: {}", m.content),
            _ => m.content.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    (system, user)
}

/// Split a transcript into `(system, non-system messages)` for the providers
/// (Anthropic, Gemini) that carry the system prompt in a dedicated field. Pure.
pub(crate) fn split_system(messages: &[ChatMsg]) -> (String, Vec<&ChatMsg>) {
    let system = messages
        .iter()
        .filter(|m| m.role == Role::System)
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let rest = messages.iter().filter(|m| m.role != Role::System).collect();
    (system, rest)
}

/// The single-shot tool-calling fallback: run `complete_with_usage` and return
/// an [`AgentTurn`] carrying no tool calls but the real reported usage (zero
/// for a provider that genuinely reports none, e.g. a CLI agent). Used by the
/// trait default and by any adapter whose model doesn't support tools.
/// Generic over `?Sized` so it works from both the trait default (`&Self`) and
/// a concrete adapter.
pub(crate) async fn single_shot_turn<P: AiProvider + ?Sized>(
    provider: &P,
    app: &AppHandle,
    model: &str,
    messages: &[ChatMsg],
    temperature: Option<f64>,
) -> AppResult<AgentTurn> {
    let (system, user) = flatten_messages(messages);
    let (text, usage) = provider
        .complete_with_usage(app, model, &system, &user, temperature)
        .await?;
    Ok(AgentTurn {
        text,
        tool_calls: Vec::new(),
        stop: StopReason::End,
        usage,
    })
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
        | ProviderId::Antigravity => {
            unreachable!("CLI agents are resolved via cli_agent::backend_for")
        }
    }
}

/// Parse + resolve in one step (used by the settings commands).
pub fn resolve_by_name(name: &str, base_url: Option<String>) -> AppResult<Box<dyn AiProvider>> {
    Ok(resolve(ProviderId::parse(name)?, base_url))
}

/// Discover a reachable local chat model for `provider_id`.
///
/// Returns `Some(model_name)` only when the provider is reachable and has an
/// available chat model. CLI agents and cloud providers always return `None`.
pub async fn reachable_chat_model(provider_id: ProviderId) -> Option<String> {
    match provider_id {
        ProviderId::Ollama => {
            let (reachable, model) = ollama::reachable_model().await;
            if reachable {
                model
            } else {
                None
            }
        }
        _ => None,
    }
}

// ── Embeddings ────────────────────────────────────────────────────────────────

/// Vector-FORMAT version — bumped whenever the ALGORITHM that produces a
/// stored vector's VALUES changes for the same `(provider, model, dim)`, even
/// though the provider/model IDENTITY is unchanged (e.g. replacing a naive
/// single truncation with chunk-and-mean-pool — same tag, semantically
/// different vector). `EmbeddingConfig::matches` checks this so a vector
/// persisted before a bump is treated as stale and re-embedded, instead of
/// being silently compared against a new-format vector under the identical
/// `(provider, model, dim)` tag.
pub const EMBEDDING_VECTOR_VERSION: i64 = 2;

/// The identity of an embedding "space": vectors are only comparable when they
/// share the same `(provider, model, dim)` AND the same [`EMBEDDING_VECTOR_VERSION`]
/// they were produced under. Stored alongside every vector so incompatible —
/// or differently-produced — vectors can never be silently mixed. `version`
/// is a storage-format detail, not part of the wire shape (`#[serde(skip)]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSpace {
    pub provider: String,
    pub model: String,
    pub dim: usize,
    #[serde(skip)]
    pub version: i64,
}

impl std::fmt::Display for EmbeddingSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}@{}", self.provider, self.model, self.dim)
    }
}

/// A vector tagged with the space it was produced in.
#[derive(Debug, Clone)]
pub struct EmbeddingVector {
    pub values: Vec<f64>,
    pub space: EmbeddingSpace,
}

/// Embed `text` with an explicit provider/model, returning a space-tagged vector.
/// Routes through the same `resolve` + capability + auth flow as chat, so there
/// are no Ollama assumptions and no silent fallback.
///
/// This is the shared chokepoint for AI-spend visibility on embedding calls —
/// every caller (`ai_embed`, `posting_vector_or_embed`'s match-score
/// resolution, `ai_reembed_all`'s batch re-index) routes through here, so
/// each records the provider's REAL reported token usage (zero when a
/// provider genuinely reports none) with no changes needed at any call site.
pub async fn embed_text(
    app: &AppHandle,
    provider: ProviderId,
    model: &str,
    base_url: Option<String>,
    text: &str,
) -> AppResult<EmbeddingVector> {
    let client = resolve(provider, base_url.clone());
    let model = if model.trim().is_empty() {
        client
            .default_embedding_model()
            .ok_or_else(|| {
                AppError::Config(format!(
                    "{} does not support embeddings.",
                    provider.as_str()
                ))
            })?
            .to_string()
    } else {
        model.to_string()
    };
    if !client.capabilities(&model).supports_embeddings {
        return Err(AppError::Config(format!(
            "{} does not support embeddings.",
            provider.as_str()
        )));
    }
    // Cap the input to the provider's real limit, char-boundary-safe, then
    // adaptively retry on a context-length overflow — see `embed_adaptive`.
    // Applied here so every provider is consistent and a new one inherits a
    // safe default — see `AiProvider::max_embedding_input_chars`.
    let initial_cap = client.max_embedding_input_chars();
    let attempt = ProviderEmbedAttempt {
        app,
        client: client.as_ref(),
        model: &model,
    };
    // `usage` accumulates as `embed_adaptive` runs, even if it ultimately
    // errors (a multi-chunk document can bill several real provider calls
    // before failing on a later one) — record whatever was actually billed
    // BEFORE propagating the error, so a partial failure never silently
    // drops already-spent tokens from the ledger.
    let mut usage = Usage::default();
    let result = embed_adaptive(&attempt, text, initial_cap, &mut usage).await;
    record_usage(
        app,
        provider.as_str(),
        &model,
        usage.input_tokens,
        usage.output_tokens,
        base_url.as_deref(),
    );
    let values = result?;
    if values.is_empty() {
        return Err(AppError::Provider(format!(
            "{} returned an empty embedding.",
            provider.as_str()
        )));
    }
    let dim = values.len();
    Ok(EmbeddingVector {
        values,
        space: EmbeddingSpace {
            provider: provider.as_str().to_string(),
            model,
            dim,
            version: EMBEDDING_VECTOR_VERSION,
        },
    })
}

/// Cosine similarity between two vectors that MUST share an embedding space.
/// Returns `Err` on a space mismatch — incomparable vectors are never silently
/// scored (the old behavior returned 0.0 and hid the bug).
pub fn compare(a: &EmbeddingVector, b: &EmbeddingVector) -> AppResult<f64> {
    if a.space != b.space {
        return Err(AppError::Validation(format!(
            "refusing to compare embeddings from different spaces: {} vs {}",
            a.space, b.space
        )));
    }
    Ok(cosine(&a.values, &b.values))
}

/// Raw cosine similarity — re-exported from the shared L0 [`crate::vector`]
/// module so `compare` and every existing `ai_provider::cosine` caller keep the
/// same path, while `scraping::cluster` reuses the SAME implementation for
/// cross-board dedup without an upward layer import (architecture rule R7).
/// Prefer [`compare`] for stored vectors so embedding spaces are checked first.
pub use crate::vector::cosine;

// ── Request tracing ─────────────────────────────────────────────────────────────

/// Structured per-request log over the shared [`crate::observability::Span`].
/// Emits a `→` line at dispatch and a `←` line with status + duration at
/// completion, e.g.:
/// `[ai] ← provider=openai model=gpt-4o endpoint=/chat/completions … status=200 duration=1842ms ok=true`
pub struct RequestTrace {
    span: crate::observability::Span,
}

impl RequestTrace {
    pub fn begin(
        provider: ProviderId,
        model: &str,
        endpoint: &str,
        base_url: &str,
        streaming: bool,
    ) -> Self {
        let fields = format!(
            "provider={} model={} endpoint={} baseUrl={} streaming={}",
            provider.as_str(),
            model,
            endpoint,
            base_url,
            streaming
        );
        Self {
            span: crate::observability::Span::begin("ai", fields),
        }
    }

    pub fn end(&self, status: Option<u16>, ok: bool) {
        let status = status
            .map(|s| s.to_string())
            .unwrap_or_else(|| "-".to_string());
        self.span.end_with(&format!("status={status}"), ok);
    }
}

// ── Error mapping ───────────────────────────────────────────────────────────────

/// Pull a human-readable message out of a provider's JSON error body.
pub fn extract_error_message(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        if let Some(msg) = v
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .or_else(|| v.get("message").and_then(|m| m.as_str()))
            .or_else(|| {
                v.get("error")
                    .and_then(|e| e.get(0))
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
            })
        {
            return msg.to_string();
        }
    }
    body.trim().chars().take(200).collect()
}

/// Map a provider HTTP error to a clear, actionable message.
pub fn friendly_api_error(
    provider: ProviderId,
    status: reqwest::StatusCode,
    body: &str,
) -> AppError {
    let name = provider.as_str();
    let code = status.as_u16();
    let detail = extract_error_message(body);
    match code {
        401 | 403 => AppError::Config(format!("{name}: invalid or unauthorized API key.")),
        404 => AppError::Provider(format!("{name}: model or endpoint not found — {detail}")),
        413 => AppError::Provider(format!(
            "{name}: request too large — try a smaller resume/job ad."
        )),
        422 => AppError::Provider(format!(
            "{name}: this model rejected the request — {detail}"
        )),
        429 => AppError::Network(format!(
            "{name}: rate limit or quota reached. Wait a moment or check your plan."
        )),
        400 => AppError::Provider(format!("{name}: request rejected — {detail}")),
        500..=599 => AppError::Network(format!(
            "{name}: service error ({code}). Try again shortly."
        )),
        _ => AppError::Provider(format!("{name} {code}: {detail}")),
    }
}

/// Emit the terminal `ai:stream` error event the renderer's stream reader expects.
pub fn emit_stream_error(app: &AppHandle, job_id: &str, message: &str) {
    emit_event(
        app,
        AI_STREAM,
        AiStreamChunk {
            job_id: job_id.to_string(),
            delta: String::new(),
            done: true,
            error: Some(AiStreamChunkError {
                code: "GENERATION_FAILED".to_string(),
                message: message.to_string(),
            }),
            thinking: None,
        },
    );
}

#[cfg(test)]
mod tests;
