//! Ollama Cloud provider — hosted Ollama models over the OpenAI-compatible
//! endpoint (`ollama.com/v1`), **composed** from the OpenAI client. Chat, model
//! listing, and auth are forwarded to the inner client unchanged; only
//! [`research`](AiProvider::research) is overridden to use the Ollama Web Search
//! API (the same `ai:ollama-cloud` account key). This keeps the generic OpenAI
//! client free of any Ollama special-case (no `match self.id`).

use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;

use crate::error::AppResult;

use super::openai::OpenAiClient;
use super::{
    AgentTurn, AiGenerateRequest, AiProvider, ChatMsg, ModelCapabilities, ProviderId, ToolSpec,
    Usage,
};

/// Ollama Cloud's OpenAI-compatible base URL.
const CLOUD_BASE: &str = "https://ollama.com/v1";

pub struct OllamaCloudClient {
    inner: OpenAiClient,
}

impl OllamaCloudClient {
    pub fn new() -> Self {
        Self {
            inner: OpenAiClient::new(ProviderId::OllamaCloud, Some(CLOUD_BASE.to_string())),
        }
    }
}

impl Default for OllamaCloudClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiProvider for OllamaCloudClient {
    fn id(&self) -> ProviderId {
        ProviderId::OllamaCloud
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        // The inner client's `id` is `OllamaCloud`, so its own
        // `supports_web_search` reads `false` (only native OpenAI passes that
        // check) — but Ollama Cloud DOES search, via the Ollama Web Search API
        // (overridden below), so force it back to `true` here.
        ModelCapabilities {
            supports_web_search: true,
            ..self.inner.capabilities(model)
        }
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        self.inner.chat_stream(app, job_id, req).await
    }

    async fn complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<String> {
        self.inner
            .complete(app, model, system, user, temperature)
            .await
    }

    async fn complete_with_usage(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
        // Ollama Cloud's `/v1` endpoint is served by the inner OpenAI-compatible
        // client, which sends `stream_options.include_usage` and parses the
        // real `usage.{prompt_tokens,completion_tokens}` OpenAI-shape response —
        // real token counts, not a naive default-to-zero delegation.
        self.inner
            .complete_with_usage(app, model, system, user, temperature)
            .await
    }

    async fn research(
        &self,
        app: &AppHandle,
        model: &str,
        company: &str,
        role: &str,
    ) -> AppResult<String> {
        // Search via the Ollama Web Search API (same account key), then synthesize
        // through the cloud chat model (the inner OpenAI-compatible client).
        super::ollama::ollama_research(app, &self.inner, model, company, role).await
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
        super::ollama::ollama_research_salary(
            app,
            &self.inner,
            model,
            role,
            company,
            location,
            country,
            currency,
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
        super::ollama::ollama_research_answer(app, &self.inner, model, question, role, company)
            .await
    }

    async fn embed(&self, app: &AppHandle, model: &str, text: &str) -> AppResult<Vec<f64>> {
        self.inner.embed(app, model, text).await
    }

    async fn embed_with_usage(
        &self,
        app: &AppHandle,
        model: &str,
        text: &str,
    ) -> AppResult<(Vec<f64>, Usage)> {
        // Real usage parsing lives on the inner OpenAI-compatible client, exactly
        // like `complete_with_usage` above — never a naive default-to-zero.
        self.inner.embed_with_usage(app, model, text).await
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        self.inner.default_embedding_model()
    }

    async fn list_models(&self, app: &AppHandle) -> Vec<Value> {
        self.inner.list_models(app).await
    }

    async fn test_key(&self, app: &AppHandle) -> AppResult<()> {
        self.inner.test_key(app).await
    }

    async fn chat_with_tools(
        &self,
        app: &AppHandle,
        model: &str,
        messages: &[ChatMsg],
        tools: &[ToolSpec],
        temperature: Option<f64>,
    ) -> AppResult<AgentTurn> {
        // `capabilities()` above already delegates to the inner OpenAI-compatible
        // client, which reports `supports_tools: true` — Ollama Cloud's `/v1`
        // endpoint IS OpenAI-compatible and tool-capable, so delegate the turn too
        // rather than silently degrading to a single-shot answer.
        self.inner
            .chat_with_tools(app, model, messages, tools, temperature)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression guard: the inner `OpenAiClient`'s own `supports_web_search`
    /// only passes for `ProviderId::OpenAi`, so a naive delegation would
    /// wrongly report `false` for Ollama Cloud — which DOES search via the
    /// Ollama Web Search API (see `research_answer` above). The override in
    /// `capabilities()` must force it back to `true`.
    #[test]
    fn capabilities_reports_web_search_support_despite_the_inner_openai_compatible_id() {
        let caps = OllamaCloudClient::new().capabilities("gpt-oss:120b");
        assert!(caps.supports_web_search);
    }
}
