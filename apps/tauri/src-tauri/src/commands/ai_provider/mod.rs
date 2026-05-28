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

use std::time::Instant;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

pub use crate::ipc_contracts::ai::AiGenerateRequest;

mod anthropic;
mod gemini;
pub mod ollama; // pub: its Ollama-only helpers back the local model list / health / embeddings
mod openai;

use anthropic::AnthropicClient;
use gemini::GeminiClient;
use ollama::OllamaClient;
use openai::OpenAiClient;

// ── Provider identity ─────────────────────────────────────────────────────────

/// Every supported provider. Stringly-typed provider checks are banned in favor
/// of this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderId {
    Ollama,
    OpenAi,
    /// Any OpenAI-compatible server (LM Studio, vLLM, OpenRouter, Groq,
    /// Together, DeepSeek, Azure-style gateways…) addressed via a custom base URL.
    OpenAiCompatible,
    Anthropic,
    Gemini,
}

impl ProviderId {
    /// Parse a wire string. Unknown values are a hard error — never a fallback.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "ollama" => Ok(Self::Ollama),
            "openai" => Ok(Self::OpenAi),
            "openai-compatible" => Ok(Self::OpenAiCompatible),
            "anthropic" => Ok(Self::Anthropic),
            "gemini" => Ok(Self::Gemini),
            other => Err(format!(
                "Unknown AI provider '{other}'. Select a configured provider in Settings → AI."
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::OpenAi => "openai",
            Self::OpenAiCompatible => "openai-compatible",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
        }
    }

    /// Credential-store key suffix (`ai:<key>`). Ollama needs none.
    pub fn credential_key(&self) -> &'static str {
        self.as_str()
    }

    /// Whether this provider runs locally (no API key, no outbound cloud call).
    #[allow(dead_code)]
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Ollama)
    }

    /// Reject obvious provider/model mismatches server-side (do not trust the UI).
    /// Lenient for Ollama and OpenAI-compatible servers, where model names are
    /// arbitrary; strict for the native cloud providers with namespaced models.
    pub fn validate_model(&self, model: &str) -> Result<(), String> {
        let m = model.trim().to_ascii_lowercase();
        if m.is_empty() {
            return Err("No model selected for the active provider.".to_string());
        }
        let looks_anthropic = m.starts_with("claude");
        let looks_gemini = m.starts_with("gemini") || m.starts_with("models/gemini");
        let looks_openai = m.starts_with("gpt")
            || m.starts_with("chatgpt")
            || m.starts_with("o1")
            || m.starts_with("o3")
            || m.starts_with("o4");

        let mismatch = |family: &str| {
            Err(format!(
                "Model '{model}' looks like a {family} model, but the active provider is {}. \
                 Pick a matching model or switch providers.",
                self.as_str()
            ))
        };

        match self {
            Self::Anthropic if !looks_anthropic => mismatch("non-Anthropic"),
            Self::Gemini if !looks_gemini => mismatch("non-Gemini"),
            Self::OpenAi if looks_anthropic => mismatch("Anthropic"),
            Self::OpenAi if looks_gemini => mismatch("Gemini"),
            Self::OpenAi if !looks_openai => mismatch("non-OpenAI"),
            // Ollama / OpenAI-compatible: any model name is allowed.
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
    ) -> Result<(), String>;

    /// List models this provider exposes for the given key.
    async fn list_models(&self, client: &reqwest::Client, api_key: &str) -> Vec<Value>;

    /// Validate the stored API key — Ok(()) when the provider is reachable/authed.
    async fn test_key(&self, client: &reqwest::Client, api_key: &str) -> Result<(), String>;
}

/// Single routing point. `base_url` only applies to OpenAI-compatible servers.
pub fn resolve(id: ProviderId, base_url: Option<String>) -> Box<dyn AiProvider> {
    match id {
        ProviderId::Ollama => Box::new(OllamaClient),
        ProviderId::OpenAi => Box::new(OpenAiClient::new(ProviderId::OpenAi, None)),
        ProviderId::OpenAiCompatible => {
            Box::new(OpenAiClient::new(ProviderId::OpenAiCompatible, base_url))
        }
        ProviderId::Anthropic => Box::new(AnthropicClient),
        ProviderId::Gemini => Box::new(GeminiClient),
    }
}

/// Parse + resolve in one step (used by the settings commands).
pub fn resolve_by_name(name: &str, base_url: Option<String>) -> Result<Box<dyn AiProvider>, String> {
    Ok(resolve(ProviderId::parse(name)?, base_url))
}

// ── Request tracing ─────────────────────────────────────────────────────────────

/// Structured per-request log. Emits a `→` line at dispatch and a `←` line with
/// status + duration at completion, e.g.:
/// `[ai] ← provider=openai model=gpt-4o endpoint=/chat/completions status=200 duration=1842ms ok=true`
pub struct RequestTrace {
    provider: ProviderId,
    model: String,
    endpoint: String,
    base_url: String,
    streaming: bool,
    start: Instant,
}

impl RequestTrace {
    pub fn begin(
        provider: ProviderId,
        model: &str,
        endpoint: &str,
        base_url: &str,
        streaming: bool,
    ) -> Self {
        log::info!(
            "[ai] → provider={} model={} endpoint={} baseUrl={} streaming={}",
            provider.as_str(),
            model,
            endpoint,
            base_url,
            streaming
        );
        Self {
            provider,
            model: model.to_string(),
            endpoint: endpoint.to_string(),
            base_url: base_url.to_string(),
            streaming,
            start: Instant::now(),
        }
    }

    pub fn end(&self, status: Option<u16>, ok: bool) {
        log::info!(
            "[ai] ← provider={} model={} endpoint={} baseUrl={} streaming={} status={} duration={}ms ok={}",
            self.provider.as_str(),
            self.model,
            self.endpoint,
            self.base_url,
            self.streaming,
            status.map(|s| s.to_string()).unwrap_or_else(|| "-".to_string()),
            self.start.elapsed().as_millis(),
            ok
        );
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
pub fn friendly_api_error(provider: ProviderId, status: reqwest::StatusCode, body: &str) -> String {
    let name = provider.as_str();
    let code = status.as_u16();
    let detail = extract_error_message(body);
    match code {
        401 | 403 => format!("{name}: invalid or unauthorized API key."),
        404 => format!("{name}: model or endpoint not found — {detail}"),
        413 => format!("{name}: request too large — try a smaller resume/job ad."),
        422 => format!("{name}: this model rejected the request — {detail}"),
        429 => format!("{name}: rate limit or quota reached. Wait a moment or check your plan."),
        400 => format!("{name}: request rejected — {detail}"),
        500..=599 => format!("{name}: service error ({code}). Try again shortly."),
        _ => format!("{name} {code}: {detail}"),
    }
}

/// Emit the terminal `ai:stream` error event the renderer's stream reader expects.
pub fn emit_stream_error(app: &AppHandle, job_id: &str, message: &str) {
    let _ = app.emit(
        "ai:stream",
        json!({
            "jobId": job_id,
            "delta": "",
            "done": true,
            "error": { "code": "GENERATION_FAILED", "message": message },
        }),
    );
}
