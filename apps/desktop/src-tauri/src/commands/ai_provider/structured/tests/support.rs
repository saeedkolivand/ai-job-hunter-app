//! Shared fixtures for `structured`'s test topics.

use super::super::*;
use crate::ipc_contracts::ai::AiGenerateRequestMessage;

pub(super) fn request(messages: &[(&str, &str)]) -> AiGenerateRequest {
    AiGenerateRequest {
        model: "llama3.1:8b".to_string(),
        messages: messages
            .iter()
            .map(|(role, content)| AiGenerateRequestMessage {
                role: (*role).to_string(),
                content: (*content).to_string(),
            })
            .collect(),
        locale: "en".to_string(),
        temperature: None,
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

/// A schema nested `depth` levels deep through `properties`.
pub(super) fn deep_schema(depth: usize) -> Value {
    let mut schema = json!({ "type": "string" });
    for _ in 0..depth {
        schema = json!({ "type": "object", "properties": { "next": schema } });
    }
    schema
}
