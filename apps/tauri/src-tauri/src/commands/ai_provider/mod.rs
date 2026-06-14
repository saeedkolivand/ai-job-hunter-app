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
use serde_json::Value;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::events::{emit_event, AiStreamChunk, AiStreamChunkError, AI_STREAM};
pub use crate::ipc_contracts::ai::AiGenerateRequest;

mod anthropic;
pub mod cli_agent; // pub: its registry/detection back the CLI-agent health probe
mod gemini;
pub mod ollama; // pub: its Ollama-only helpers back the local model list / health / embeddings
mod ollama_cloud;
mod openai;
mod research; // shared company-research prompt spec + helpers used by every `research()`
mod retry; // bounded exponential backoff for the non-streaming complete/embed paths
mod stream; // shared streaming loop (cancel-check + chunk read + emit + complete) for cloud adapters

use anthropic::AnthropicClient;
use cli_agent::CliAgentClient;
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
    pub token_param: TokenParam,
}

// ── Provider trait & registry ────────────────────────────────────────────────

/// A chat backend. Object-safe so the registry can return `Box<dyn AiProvider>`.
#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> ProviderId;

    /// Capability matrix for a given model on this provider.
    fn capabilities(&self, model: &str) -> ModelCapabilities;

    /// Stream a chat completion, emitting `ai:stream` deltas and marking the job
    /// complete/failed. Resolves its own API key (isolated auth per provider).
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

    /// Embed a single text, returning the raw vector. Errors when this provider
    /// has no embeddings API (callers gate on `capabilities().supports_embeddings`).
    async fn embed(&self, app: &AppHandle, model: &str, text: &str) -> AppResult<Vec<f64>>;

    /// The provider's default embedding model, or `None` if it has no embeddings API.
    fn default_embedding_model(&self) -> Option<&'static str>;

    /// List the models this provider exposes. Resolves its own credentials/client
    /// (exactly like `chat_stream`/`complete`), so no HTTP/key transport detail
    /// leaks into the trait — a CLI agent has neither and just lists its aliases.
    async fn list_models(&self, app: &AppHandle) -> Vec<Value>;

    /// Validate that the provider is usable: cloud → the stored key authenticates;
    /// local server / CLI agent → reachable / installed. Resolves its own deps from
    /// `app`, returning a clear error when nothing is configured.
    async fn test_key(&self, app: &AppHandle) -> AppResult<()>;
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
        ProviderId::ClaudeCode | ProviderId::Codex | ProviderId::GeminiCli => {
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

/// The identity of an embedding "space": vectors are only comparable when they
/// share the same `(provider, model, dim)`. Stored alongside every vector so
/// incompatible vectors can never be silently mixed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSpace {
    pub provider: String,
    pub model: String,
    pub dim: usize,
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
pub async fn embed_text(
    app: &AppHandle,
    provider: ProviderId,
    model: &str,
    base_url: Option<String>,
    text: &str,
) -> AppResult<EmbeddingVector> {
    let client = resolve(provider, base_url);
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
    let values = client.embed(app, &model, text).await?;
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

/// Raw cosine similarity. Prefer [`compare`] for stored vectors so spaces are checked.
pub fn cosine(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

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
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_vectors_is_one() {
        let a = vec![1.0, 2.0, 3.0];
        assert!((cosine(&a, &a) - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_orthogonal_vectors_is_zero() {
        assert!((cosine(&[1.0, 0.0], &[0.0, 1.0]) - 0.0).abs() < 0.001);
    }

    #[test]
    fn cosine_edge_cases_return_zero() {
        // Empty vectors, mismatched lengths, and zero vectors all yield 0.0.
        assert_eq!(cosine(&[], &[]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
    }

    #[test]
    fn provider_id_round_trips() {
        for id in [
            ProviderId::Ollama,
            ProviderId::OllamaCloud,
            ProviderId::OpenAi,
            ProviderId::OpenAiCompatible,
            ProviderId::Anthropic,
            ProviderId::Gemini,
            ProviderId::ClaudeCode,
            ProviderId::Codex,
            ProviderId::GeminiCli,
        ] {
            assert_eq!(ProviderId::parse(id.as_str()).unwrap(), id);
        }
        assert!(ProviderId::parse("nope").is_err());
    }

    #[test]
    fn ollama_cloud_wire_and_credential_key() {
        assert_eq!(ProviderId::OllamaCloud.as_str(), "ollama-cloud");
        assert_eq!(
            ProviderId::parse("ollama-cloud").unwrap(),
            ProviderId::OllamaCloud
        );
        // Shares the `ai:ollama-cloud` credential slot used by Ollama Web Search.
        assert_eq!(ProviderId::OllamaCloud.credential_key(), "ollama-cloud");
        // Cloud, not a local CLI agent.
        assert!(!ProviderId::OllamaCloud.is_cli_agent());
        assert!(!ProviderId::OllamaCloud.is_local());
    }

    #[test]
    fn resolve_ollama_cloud_returns_cloud_client() {
        // Composed client reports its own id (chat is delegated to the inner
        // OpenAI client against ollama.com/v1).
        assert_eq!(
            resolve(ProviderId::OllamaCloud, None).id(),
            ProviderId::OllamaCloud
        );
    }

    #[test]
    fn claude_code_is_a_local_cli_agent() {
        assert!(ProviderId::ClaudeCode.is_cli_agent());
        assert!(ProviderId::ClaudeCode.is_local());
        assert!(!ProviderId::Anthropic.is_cli_agent());
    }

    #[test]
    fn validate_model_allows_unknown_new_names() {
        // A model the code has never heard of must still be accepted, so newly
        // released models work with no code change.
        assert!(ProviderId::OpenAi.validate_model("gpt-6-ultra").is_ok());
        assert!(ProviderId::OpenAi.validate_model("o9-pro").is_ok());
        assert!(ProviderId::Anthropic
            .validate_model("claude-5-haiku")
            .is_ok());
        assert!(ProviderId::Gemini.validate_model("gemini-9-ultra").is_ok());
    }

    #[test]
    fn validate_model_blocks_clear_cross_provider_mistakes() {
        assert!(ProviderId::Anthropic.validate_model("gpt-4o").is_err());
        assert!(ProviderId::OpenAi
            .validate_model("claude-opus-4-7")
            .is_err());
        assert!(ProviderId::Gemini.validate_model("claude-3").is_err());
    }

    #[test]
    fn validate_model_openai_compatible_accepts_any_family() {
        // OpenRouter (openai-compatible) serves anthropic/* and google/* models.
        assert!(ProviderId::OpenAiCompatible
            .validate_model("anthropic/claude-3.5-sonnet")
            .is_ok());
        assert!(ProviderId::OpenAiCompatible
            .validate_model("google/gemini-2.0-flash")
            .is_ok());
    }

    #[test]
    fn validate_model_cli_agent_allows_empty_and_aliases() {
        assert!(ProviderId::ClaudeCode.validate_model("").is_ok());
        assert!(ProviderId::ClaudeCode.validate_model("sonnet").is_ok());
    }

    #[test]
    fn validate_model_cloud_requires_a_model() {
        assert!(ProviderId::OpenAi.validate_model("").is_err());
    }
}
