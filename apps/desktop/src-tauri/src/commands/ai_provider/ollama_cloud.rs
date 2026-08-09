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
    AgentTurn, AiGenerateRequest, AiProvider, ChatMsg, Intent, ModelCapabilities, ProviderId,
    SamplingProfile, ToolSpec, Usage,
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

    fn effort_levels(&self, model: &str) -> Vec<&'static str> {
        self.inner.effort_levels(model)
    }

    fn sampling_profile(&self, model: &str, intent: Intent) -> SamplingProfile {
        // The inner client's `sampling_profile` already special-cases
        // `self.id == ProviderId::OllamaCloud` (its `id` field is set to
        // `OllamaCloud` in `new()` above), so this delegates to the SAME
        // gpt-oss-aware table `chat_stream` actually uses — never a second,
        // divergent copy. Delegating (rather than leaving the trait default)
        // matters for any direct caller of this method, e.g. a future
        // capabilities surface or a test — `chat_stream` reaches the inner
        // client's own `sampling_profile` regardless of this override.
        self.inner.sampling_profile(model, intent)
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

    /// Delegated, NOT reimplemented with local Ollama's native `format` field:
    /// Ollama Cloud is reached over its **OpenAI-compatible** `/v1` endpoint
    /// (see this module's doc comment), which takes `response_format` — the
    /// `/api/chat`-only `format` key would be ignored there, silently dropping
    /// the constraint. The inner client's own `supports_response_format` gate
    /// admits `ProviderId::OllamaCloud` for exactly this call.
    async fn complete_structured(
        &self,
        app: &AppHandle,
        req: &AiGenerateRequest,
        schema_hint: &str,
        schema: Option<&Value>,
    ) -> AppResult<(String, Usage)> {
        self.inner
            .complete_structured(app, req, schema_hint, schema)
            .await
    }

    fn has_native_search(&self, _model: &str) -> bool {
        // Advertises web search for the family, but the MODEL never searches —
        // the hosted Web Search API does, through `native_searcher`.
        false
    }

    fn native_searcher(
        &self,
        app: &AppHandle,
        model: &str,
    ) -> Option<Box<dyn super::search::WebSearcher>> {
        // Same hosted Web Search API as local Ollama, and the account key IS
        // this provider's generation key — so a configured Ollama Cloud always
        // has it, and search never falls back.
        super::ollama::OllamaSearcher::from_credentials(app, model)
            .map(|s| Box::new(s) as Box<dyn super::search::WebSearcher>)
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

    async fn list_models(&self, app: &AppHandle) -> AppResult<Vec<Value>> {
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
    use super::super::{
        PROSE_FREQUENCY_PENALTY, PROSE_PRESENCE_PENALTY, PROSE_TEMPERATURE, PROSE_TOP_P,
    };
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

    #[test]
    fn effort_levels_delegate_to_the_inner_client() {
        assert_eq!(
            OllamaCloudClient::new().effort_levels("gpt-oss:120b"),
            vec!["low", "medium", "high"]
        );
        assert!(OllamaCloudClient::new()
            .effort_levels("qwen3-coder:480b")
            .is_empty());
    }

    /// Regression guard, same shape as the two tests above: a naive
    /// delegation to `self.inner` (a generic `OpenAiClient`) would need to
    /// reach the SAME `self.id == ProviderId::OllamaCloud` branch inside
    /// `OpenAiClient::sampling_profile` that `chat_stream` actually uses —
    /// this pins the exact path production traffic takes, not a parallel copy.
    #[test]
    fn sampling_profile_delegates_to_the_inner_clients_ollama_cloud_table() {
        let gpt_oss = OllamaCloudClient::new().sampling_profile("gpt-oss:120b", Intent::Prose);
        assert_eq!(gpt_oss.temperature, Some(1.0));
        assert_eq!(gpt_oss.top_p, Some(1.0));

        // `ollama_cloud_sampling_profile` returns the gpt-oss 1.0/1.0 override
        // BEFORE it ever inspects `intent` — pin a second, different intent on
        // the SAME model so a future refactor that starts branching gpt-oss on
        // intent (breaking the family-wide override) fails here, not silently.
        let gpt_oss_deterministic =
            OllamaCloudClient::new().sampling_profile("gpt-oss:120b", Intent::Deterministic);
        assert_eq!(gpt_oss_deterministic.temperature, Some(1.0));
        assert_eq!(gpt_oss_deterministic.top_p, Some(1.0));

        // Every OTHER family reuses the SAME per-intent targets native
        // OpenAI does (the `/v1` layer hardcodes 1.0/1.0 on omission for
        // every family, not just gpt-oss — see `ollama_cloud_sampling_profile`'s
        // doc comment) — never left neutral.
        let other = OllamaCloudClient::new().sampling_profile("qwen3-coder:480b", Intent::Prose);
        assert_eq!(
            other,
            SamplingProfile {
                temperature: Some(PROSE_TEMPERATURE),
                top_p: Some(PROSE_TOP_P),
                frequency_penalty: Some(PROSE_FREQUENCY_PENALTY),
                presence_penalty: Some(PROSE_PRESENCE_PENALTY),
                ..SamplingProfile::default()
            }
        );
    }
}
