//! Google Gemini provider — generateContent (streaming) API.

use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;

use crate::error::AppResult;

use super::research;
use super::structured;
use super::timeouts;
use super::{
    AgentTurn, AiGenerateRequest, AiProvider, ChatMsg, Intent, ModelCapabilities, ProviderId,
    SamplingProfile, ToolSpec, Usage,
};

mod body;
mod capabilities;
mod chat;
mod thinking;
mod tools;
mod transport;
mod web_search;
mod wire;

// `gemini/body.rs` reaches these three model-classification predicates via
// `use super::{...}` — re-exported here, at their pre-split path, so that
// import never has to change.
use capabilities::gemini_effort_levels;
use thinking::{
    gemini_effective_temperature, gemini_omits_sampling_params, gemini_supports_thinking,
};

const BASE: &str = "https://generativelanguage.googleapis.com";

/// Requested embedding output size. `gemini-embedding-2`'s default (unspecified)
/// dimensionality is 3072 — 4x the retired text-embedding-004's 768. One of
/// the model's documented "Recommended" sizes; auto-normalized by the API
/// (see `chat::GeminiClient::embed_impl`'s call site for the full rationale).
const EMBED_OUTPUT_DIMENSIONALITY: i64 = 768;

pub struct GeminiClient;

#[async_trait]
impl AiProvider for GeminiClient {
    fn id(&self) -> ProviderId {
        ProviderId::Gemini
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        capabilities::gemini_capabilities(model)
    }

    fn effort_levels(&self, model: &str) -> Vec<&'static str> {
        capabilities::gemini_effort_levels(model)
    }

    fn sampling_profile(&self, model: &str, intent: Intent) -> SamplingProfile {
        capabilities::gemini_sampling_profile(model, intent)
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

    /// Native constrained decoding via `generationConfig.responseMimeType` +
    /// `responseSchema`. Gemini's schema is an OpenAPI-3.0 subset, not JSON
    /// Schema, so the caller's schema is translated first
    /// ([`structured::gemini_response_schema`]); a schema with no faithful
    /// equivalent yields `None` and this sends JSON mode WITHOUT a shape
    /// constraint (the prompt still carries the directive + example) rather
    /// than a silently weakened one.
    ///
    /// `req.effort` rides along (gated by the same per-model table
    /// `chat_stream` uses) for the same reason: this is a full
    /// [`AiGenerateRequest`], and a structured call that silently ran at
    /// Gemini's default thinking level was the user's setting being dropped,
    /// not honored. Same for `req.max_tokens` — the streaming body has always
    /// sent it, and a structured call is the one carrying a whole résumé plus
    /// a job ad.
    async fn complete_structured(
        &self,
        app: &AppHandle,
        req: &AiGenerateRequest,
        schema_hint: &str,
        schema: Option<&Value>,
    ) -> AppResult<(String, Usage)> {
        let (system, user) = structured::structured_prompt(req, schema_hint);
        self.complete_impl(
            app,
            &req.model,
            &system,
            &user,
            structured::structured_temperature(self, req),
            Some(body::StructuredCall {
                schema: schema.and_then(structured::gemini_response_schema),
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

    fn default_embedding_model(&self) -> Option<&'static str> {
        // text-embedding-004 was retired (shutdown Jan 14, 2026 — the exact
        // error this app was seeing). Google's own deprecation table names
        // `gemini-embedding-2` as the migration target for every retired
        // embedding model (verified via the live Gemini API docs, not memory).
        Some("gemini-embedding-2")
    }

    fn max_embedding_input_chars(&self) -> usize {
        // gemini-embedding-2's documented input limit is 8,192 tokens (~4
        // chars/token ≈ 32000 chars for English). Cap conservatively at 8000
        // chars: in the worst case (token-dense scripts, ~1 char/token) that
        // still stays under 8,192 tokens for every language. This is the
        // per-CHUNK size `embed_adaptive` uses — a document longer than this
        // is split into multiple chunks and mean-pooled (never truncated
        // away), and `embed_chunk_adaptive` halves-and-retries a single chunk
        // on an actual context-length error, so this default only needs to be
        // a safe starting point, not a perfect guess.
        8_000
    }

    async fn list_models(&self, app: &AppHandle) -> AppResult<Vec<Value>> {
        // `/v1beta`, not `/v1` — every generation path here already uses
        // `/v1beta` (see the `endpoint_label`s elsewhere in this file), and
        // `/v1` omits `-preview`/experimental models (e.g. the curated
        // Pro-tier default in `provider-meta.ts` is a `-preview` id, which
        // would silently vanish from the picker on `/v1`).
        let api_key = transport::require_gemini_key(app)?;
        self.list_models_transport(BASE, &api_key, timeouts::LIST_MODELS_TOTAL)
            .await
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
