//! Shared fixtures for the `ollama` adapter's test topics.

use serde_json::Value;

use super::super::super::{AiGenerateRequest, AiProvider, SamplingProfile};
use super::super::OllamaClient;
use crate::ipc_contracts::ai::AiGenerateRequestMessage;

pub(super) fn base_request() -> AiGenerateRequest {
    AiGenerateRequest {
        model: "llama3.1:8b".to_string(),
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

/// Mirrors what `OllamaClient::chat_stream` does: resolve this adapter's own
/// profile for `req.model` + `req.intent`, merged with the request's
/// explicit numeric overrides.
pub(super) fn sampling_for(req: &AiGenerateRequest) -> SamplingProfile {
    OllamaClient
        .sampling_profile(&req.model, super::super::super::resolve_intent(req))
        .resolve(req)
}

/// The structured half of a non-streaming call with only `format` set — the
/// per-test variations override the one field they are about.
pub(super) fn structured_call(format: Value) -> super::super::chat::StructuredCall<'static> {
    super::super::chat::StructuredCall {
        format,
        effort: None,
        max_tokens: None,
        context_window: None,
    }
}
