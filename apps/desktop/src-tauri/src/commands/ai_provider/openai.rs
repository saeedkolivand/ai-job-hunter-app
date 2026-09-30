//! OpenAI and OpenAI-compatible providers (LM Studio, vLLM, OpenRouter, Groq,
//! Together, DeepSeek, Azure-style gateways…). One client, parameterized by the
//! `ProviderId` and an optional base URL.

use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;
use crate::error::AppResult;

use super::research;
use super::structured;
use super::{
    AgentTurn, AiGenerateRequest, AiProvider, ChatMsg, Intent, ModelCapabilities, ProviderId,
    SamplingProfile, TokenParam, ToolSpec, Usage,
};

mod body;
mod capabilities;
mod chat;
mod tools;
mod transport;
mod web_search;
mod wire;

use body::StructuredCall;

const DEFAULT_BASE: &str = "https://api.openai.com/v1";

/// Levels `reasoning_effort` accepts on every reasoning-capable model this
/// adapter recognizes (native OpenAI's o-series + gpt-5.x, and Ollama
/// Cloud's thinking family — see `OpenAiClient::supports_reasoning_effort`).
/// Verified against the live OpenAPI schema
/// (`raw.githubusercontent.com/openai/openai-openapi/master/openapi.yaml`,
/// `ReasoningEffort` schema, checked 2026-08-04): the real wire enum has
/// grown to SEVEN values (`none, minimal, low, medium, high, xhigh, max`),
/// and the live reasoning guide (`platform.openai.com/docs/guides/reasoning`,
/// same date) states plainly: "Some models support only a subset of these
/// values, so check the relevant model page" — genuinely per-model, the SAME
/// class of variance Gemini's `thinkingLevel` and Anthropic's
/// `output_config.effort` have (see `gemini_effort_levels` / `anthropic_effort_levels`).
///
/// This adapter deliberately exposes only the THREE values every recognized
/// reasoning model accepts with no further per-model check. Unlike
/// Gemini/Anthropic, OpenAI's guide has no single closed table mapping value
/// -> supporting models — it defers to each individual model's own page, and
/// `is_gpt5_or_later_reasoning_family` deliberately matches ANY `gpt-5.x`+
/// id (including snapshots that predate `xhigh`/`max`, which the guide
/// frames as a recent addition alongside GPT-5.6's reasoning-mode overhaul).
/// Enumerating a real per-model-id table here would mean checking each
/// model's own page individually — a materially larger, separate piece of
/// work, not a same-shaped fix as the Gemini/Anthropic tables (flag as a
/// follow-up, don't guess it here). `low`/`medium`/`high` carry no per-model
/// caveat in either source, so they stay the safe universal baseline.
///
/// Still gated with `.contains(&effort)` on the send path below, not just
/// the `supports_reasoning` boolean — the same protection Gemini/Anthropic
/// use, so this stays correct with zero further change the day a follow-up
/// DOES expose a richer, genuinely per-model set here (`effort` is stored
/// PER PROVIDER, not per model — `preferences-store.ts`).
const OPENAI_EFFORT_LEVELS: [&str; 3] = ["low", "medium", "high"];

pub struct OpenAiClient {
    id: ProviderId,
    base_url: String,
}

impl OpenAiClient {
    pub fn new(id: ProviderId, base_url: Option<String>) -> Self {
        Self {
            id,
            base_url: base_url
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_BASE.to_string()),
        }
    }
}

#[async_trait]
impl AiProvider for OpenAiClient {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        self.capabilities_impl(model)
    }

    fn effort_levels(&self, model: &str) -> Vec<&'static str> {
        self.effort_levels_impl(model)
    }

    fn sampling_profile(&self, model: &str, intent: Intent) -> SamplingProfile {
        self.sampling_profile_impl(model, intent)
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        self.chat_stream_impl(app, job_id, req).await
    }

    async fn complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<String> {
        self.complete_impl(app, model, system, user, temperature, None)
            .await
            .map(|(text, _)| text)
    }

    async fn complete_with_usage(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
        self.complete_impl(app, model, system, user, temperature, None)
            .await
    }

    /// Native constrained decoding via `response_format` — strict
    /// `json_schema` when the caller supplied a schema, else `json_object`
    /// (see [`structured::openai_response_format`]). The prompt still carries
    /// the directive + filled example on this path. A gateway whose id this
    /// adapter can't vouch for falls back to the trait default — see
    /// [`capabilities::OPENAI_EFFORT_LEVELS`]'s doc and
    /// `OpenAiClient::supports_response_format`.
    ///
    /// `req.effort` rides along (gated by `reasoning_effort`) for the same
    /// reason `chat_stream` sends it: this is a full [`AiGenerateRequest`], and
    /// a structured call on a reasoning model that silently ran at the vendor's
    /// default effort was the user's setting being dropped, not honored. Same
    /// for `req.max_tokens` — the streaming body has always sent it, and a
    /// structured call is the one carrying a whole résumé plus a job ad.
    async fn complete_structured(
        &self,
        app: &AppHandle,
        req: &AiGenerateRequest,
        schema_hint: &str,
        schema: Option<&Value>,
    ) -> AppResult<(String, Usage)> {
        if !self.supports_response_format() {
            return structured::prompt_only(self, app, req, schema_hint).await;
        }
        let (system, user) = structured::structured_prompt(req, schema_hint);
        self.complete_impl(
            app,
            &req.model,
            &system,
            &user,
            structured::structured_temperature(self, req),
            Some(StructuredCall {
                response_format: structured::openai_response_format(schema),
                effort: req.effort.as_deref(),
                max_tokens: req.max_tokens,
            }),
        )
        .await
    }

    async fn research(
        &self,
        app: &AppHandle,
        model: &str,
        company: &str,
        role: &str,
    ) -> AppResult<String> {
        self.web_search_complete(
            app,
            model,
            research::NATIVE_SYSTEM,
            &research::native_user(company, role),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn research_salary(
        &self,
        app: &AppHandle,
        model: &str,
        role: &str,
        company: &str,
        location: &str,
        country: &str,
        currency: &str,
    ) -> AppResult<String> {
        self.web_search_complete(
            app,
            model,
            &research::salary_system(currency),
            &research::salary_user(role, company, location, country, currency),
        )
        .await
    }

    async fn research_answer(
        &self,
        app: &AppHandle,
        model: &str,
        question: &str,
        role: &str,
        company: &str,
    ) -> AppResult<String> {
        self.web_search_complete(
            app,
            model,
            research::ANSWER_SYSTEM,
            &research::answer_user(question, role, company),
        )
        .await
    }

    async fn embed(&self, app: &AppHandle, model: &str, text: &str) -> AppResult<Vec<f64>> {
        self.embed_impl(app, model, text).await.map(|(v, _)| v)
    }

    async fn embed_with_usage(
        &self,
        app: &AppHandle,
        model: &str,
        text: &str,
    ) -> AppResult<(Vec<f64>, Usage)> {
        self.embed_impl(app, model, text).await
    }

    /// Only **native OpenAI** hosts `text-embedding-3-small`.
    ///
    /// Every other client built on this one speaks the same `/v1` protocol but
    /// serves a completely different catalog — Ollama Cloud (which delegates
    /// this method straight through) and `OpenAiCompatible` (LM Studio, vLLM,
    /// OpenRouter, …). Handing those an OpenAI model id is a guaranteed failure:
    /// `embed_text` would post `text-embedding-3-small` to `ollama.com/v1` and
    /// get a model-not-found back, which reads to the user as "embeddings are
    /// broken" rather than "you haven't chosen an embedding model".
    ///
    /// `None` instead, so `embed_text`'s existing default-resolution error fires
    /// first and says what to actually do. Same `id == OpenAi` gate as
    /// `OpenAiClient::supports_web_search`, and for the same reason.
    ///
    /// Deliberately NOT paired with flipping `supports_embeddings` to `false`
    /// for those providers: the `/v1/embeddings` endpoint may well exist on a
    /// given gateway, and an explicit model the user knows works must still go
    /// through. Only the presumed DEFAULT was ever wrong.
    fn default_embedding_model(&self) -> Option<&'static str> {
        (self.id == ProviderId::OpenAi).then_some("text-embedding-3-small")
    }

    fn max_embedding_input_chars(&self) -> usize {
        // text-embedding-3-* enforce a hard 8191-TOKEN limit and ERROR (no
        // auto-truncate) when exceeded. The old 32k-char cap assumed ~4 chars/token
        // (English); for token-dense scripts (CJK ≈ 1 char/token) 32k chars ≈ 32k
        // tokens — far over 8191 — so the request would FAIL. Cap at 8000 chars
        // PER CHUNK: in the worst case (≈1 char/token) that stays under 8191
        // tokens for every language. A document longer than this is split into
        // multiple chunks and mean-pooled by `embed_adaptive` — never silently
        // truncated away.
        8_000
    }

    async fn list_models(&self, app: &AppHandle) -> AppResult<Vec<Value>> {
        let api_key = transport::resolve_openai_key(
            self.id,
            get_provider_key(app, self.id.credential_key()),
        )?;
        self.list_models_transport(api_key.as_deref()).await
    }

    async fn test_key(&self, app: &AppHandle) -> AppResult<()> {
        self.test_key_impl(app).await
    }

    async fn chat_with_tools(
        &self,
        app: &AppHandle,
        model: &str,
        messages: &[ChatMsg],
        tools: &[ToolSpec],
        temperature: Option<f64>,
    ) -> AppResult<AgentTurn> {
        self.chat_with_tools_impl(app, model, messages, tools, temperature)
            .await
    }
}

#[cfg(test)]
mod tests;
