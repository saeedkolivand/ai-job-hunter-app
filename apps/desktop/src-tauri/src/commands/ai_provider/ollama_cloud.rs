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
        self.inner.capabilities(model)
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

    async fn embed(&self, app: &AppHandle, model: &str, text: &str) -> AppResult<Vec<f64>> {
        self.inner.embed(app, model, text).await
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
