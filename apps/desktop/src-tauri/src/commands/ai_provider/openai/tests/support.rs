//! Shared fixtures for the `openai` adapter's test topics.

use serde_json::Value;

use super::super::super::{
    AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, SamplingProfile, TokenParam,
};
use super::super::body::StructuredCall;
use super::super::OpenAiClient;
use crate::ipc_contracts::ai::AiGenerateRequestMessage;

pub(super) fn base_request() -> AiGenerateRequest {
    AiGenerateRequest {
        model: "gpt-4o".to_string(),
        messages: vec![AiGenerateRequestMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
        }],
        locale: "en".to_string(),
        temperature: Some(0.8),
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        repeat_penalty: None,
        max_tokens: None,
        context_window: None,
        effort: None,
        intent: None,
    }
}

pub(super) fn chat_caps(supports_temperature: bool) -> ModelCapabilities {
    ModelCapabilities {
        supports_temperature,
        supports_system_role: true,
        supports_streaming: true,
        supports_reasoning: !supports_temperature,
        supports_tools: true,
        supports_json_mode: true,
        supports_embeddings: true,
        supports_web_search: false,
        token_param: TokenParam::MaxTokens,
    }
}

/// Mirrors what `OpenAiClient::chat_stream` does: resolve this adapter's own
/// profile for `id` + `req.model` + `req.intent`, merged with the request's
/// explicit numeric overrides. `id` lets a test exercise Ollama Cloud's
/// distinct table through the SAME production code path.
pub(super) fn sampling_for(id: ProviderId, req: &AiGenerateRequest) -> SamplingProfile {
    OpenAiClient::new(id, None)
        .sampling_profile(&req.model, super::super::super::resolve_intent(req))
        .resolve(req)
}

/// End-to-end wire body for one `(id, model, intent)` combination — mirrors
/// exactly what `OpenAiClient::chat_stream` does (resolve → merge → build),
/// so these tests pin what actually reaches the request JSON, not just the
/// intermediate `SamplingProfile` object. `base_request()`'s own explicit
/// `temperature: Some(0.8)` is cleared so each case exercises the adapter's
/// OWN per-intent default, not the explicit-override path (covered
/// separately).
pub(super) fn body_for(id: ProviderId, model: &str, intent: Option<&str>) -> Value {
    let mut req = base_request();
    req.model = model.to_string();
    req.temperature = None;
    req.intent = intent.map(str::to_string);
    let client = OpenAiClient::new(id, None);
    let caps = client.capabilities(&req.model);
    let sampling = client
        .sampling_profile(&req.model, super::super::super::resolve_intent(&req))
        .resolve(&req);
    super::super::body::build_chat_stream_body(&req, caps, sampling)
}

/// The structured half of a non-streaming call with only `response_format`
/// set — the per-test variations override the one field they are about.
pub(super) fn structured_call(response_format: Value) -> StructuredCall<'static> {
    StructuredCall {
        response_format,
        effort: None,
        max_tokens: None,
    }
}
