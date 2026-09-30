//! Anthropic provider — Messages API only.

use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;

use crate::error::{AppError, AppResult};

use super::research;
use super::stream::StreamPiece;
use super::structured;
use super::timeouts;
use super::{
    AgentTurn, AiGenerateRequest, AiProvider, ChatMsg, Intent, ModelCapabilities, ProviderId,
    SamplingProfile, StopReason, ToolCall, ToolSpec, Usage,
};

mod body;
mod capabilities;
mod chat;
mod thinking;
mod transport;
mod wire;

const BASE: &str = "https://api.anthropic.com/v1";
const VERSION: &str = "2023-06-01";

pub struct AnthropicClient;

#[async_trait]
impl AiProvider for AnthropicClient {
    fn id(&self) -> ProviderId {
        ProviderId::Anthropic
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        capabilities::anthropic_capabilities(model)
    }

    fn effort_levels(&self, model: &str) -> Vec<&'static str> {
        capabilities::anthropic_effort_levels(model)
    }

    fn sampling_profile(&self, model: &str, intent: Intent) -> SamplingProfile {
        capabilities::anthropic_sampling_profile(model, intent)
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

    /// Native structured output via `output_config.format` (GA — no beta
    /// header). `effort` is gated per-model ([`capabilities::anthropic_structured_effort`])
    /// before it ever reaches [`structured::anthropic_output_config`]. Off
    /// the supported-model list, or without a schema
    /// [`structured::anthropic_output_format`] can close, this falls back to
    /// the trait default (prompt discipline) — mirrors the shape of
    /// [`super::openai::OpenAiClient::complete_structured`].
    async fn complete_structured(
        &self,
        app: &AppHandle,
        req: &AiGenerateRequest,
        schema_hint: &str,
        schema: Option<&Value>,
    ) -> AppResult<(String, Usage)> {
        if !capabilities::anthropic_supports_structured_outputs(&req.model) {
            return structured::prompt_only(self, app, req, schema_hint).await;
        }
        let effort = capabilities::anthropic_structured_effort(&req.model, req.effort.as_deref());
        let Some(output_config) = structured::anthropic_output_config(schema, effort) else {
            return structured::prompt_only(self, app, req, schema_hint).await;
        };
        let (system, user) = structured::structured_prompt(req, schema_hint);
        self.complete_impl(
            app,
            &req.model,
            &system,
            &user,
            structured::structured_temperature(self, req),
            Some(output_config),
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

    async fn embed(&self, _app: &AppHandle, _model: &str, _text: &str) -> AppResult<Vec<f64>> {
        Err(AppError::Provider(
            "Anthropic has no embeddings API. Use OpenAI, Gemini, or Ollama for embeddings."
                .to_string(),
        ))
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        None
    }

    async fn list_models(&self, app: &AppHandle) -> AppResult<Vec<Value>> {
        let api_key =
            transport::require_anthropic_key(get_provider_key(app, self.id().credential_key()))?;
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
