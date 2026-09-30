//! Agentic tool-calling vocabulary (Phase 1 foundation) + the transcript
//! helpers every single-slot provider path shares. Split out of `mod.rs`
//! (R8 line-budget split).
//!
//! A [`ToolSpec`] is the schema handed to the model; a [`ToolCall`] is what
//! the model asks to run; an [`AgentTurn`] is one assistant response (text +
//! any tool calls + why it stopped); [`ChatMsg`] is the running transcript.
//!
//! SECURITY INVARIANT: only [`Role::System`] carries trusted, fixed
//! instructions. The user's question and (untrusted) tool results ride in
//! `User`/`Tool` turns and must never be merged into the system prompt or a
//! tool description. The agentic controller that enforced this was deleted
//! (PR-5 step 2) along with its only caller of [`AiProvider::chat_with_tools`]/
//! [`ToolSpec`] below — this Phase-1 tool-calling surface currently has no
//! live caller in the crate.

use serde_json::Value;
use tauri::AppHandle;

use crate::error::AppResult;

use super::{AiProvider, Usage};

/// A tool offered to the model: name, a natural-language description, and a
/// JSON-Schema object describing its arguments. Provider-agnostic; each adapter
/// maps it to that vendor's function/tool shape.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

/// One tool invocation the model asked for. `args` is already-decoded JSON — each
/// adapter parses the vendor's string/object argument form into a `Value`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

/// Why a provider ended a turn. `ToolUse` means the model wants tool results back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    End,
    ToolUse,
    Length,
    Other,
}

/// One assistant turn: visible text, any tool calls, the stop reason, and the
/// provider's REAL reported token usage for this turn (zero when a provider
/// genuinely reports none — a CLI agent, or a `single_shot_turn` fallback
/// against one that does). Consumed by `pipeline::Completer::chat_with_tools`
/// to record AI spend for the agent controller's tool-calling turns —
/// plausibly the biggest paid-token consumer, since one agent run fans out
/// into several turns.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentTurn {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub stop: StopReason,
    pub usage: Usage,
}

/// Transcript role. `System` is trusted + fixed; every other role is untrusted data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    /// Wire role string shared by the OpenAI / Ollama chat shapes. `Tool` results
    /// fold into a `user` turn (already fenced by the caller) so no adapter
    /// needs native tool-call-id linkage in Phase 1. `pub(crate)` (wider than
    /// this module's descendants) — the now-deleted agentic controller's own
    /// tests used to assert wire-alternation against this mapping instead of a
    /// duplicate.
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User | Role::Tool => "user",
            Role::Assistant => "assistant",
        }
    }
}

/// One message in the running agent transcript.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatMsg {
    pub role: Role,
    pub content: String,
}

impl ChatMsg {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
    pub fn tool(content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
        }
    }
}

/// Flatten a transcript to a `(system, user)` pair for the single-shot fallback:
/// `system` is every `Role::System` message concatenated (trusted, fixed);
/// everything else — the user question plus any prior assistant/tool turns
/// (already fenced) — is concatenated with role labels into the user prompt, so
/// untrusted content never lands in the system slot. Pure + unit-tested.
pub(crate) fn flatten_messages(messages: &[ChatMsg]) -> (String, String) {
    let system = messages
        .iter()
        .filter(|m| m.role == Role::System)
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let user = messages
        .iter()
        .filter(|m| m.role != Role::System)
        .map(|m| match m.role {
            Role::Assistant => format!("Assistant: {}", m.content),
            Role::Tool => format!("Tool result: {}", m.content),
            _ => m.content.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    (system, user)
}

/// Split a transcript into `(system, non-system messages)` for the providers
/// (Anthropic, Gemini) that carry the system prompt in a dedicated field. Pure.
pub(crate) fn split_system(messages: &[ChatMsg]) -> (String, Vec<&ChatMsg>) {
    let system = messages
        .iter()
        .filter(|m| m.role == Role::System)
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let rest = messages.iter().filter(|m| m.role != Role::System).collect();
    (system, rest)
}

/// The single-shot tool-calling fallback: run `complete_with_usage` and return
/// an [`AgentTurn`] carrying no tool calls but the real reported usage (zero
/// for a provider that genuinely reports none, e.g. a CLI agent). Used by the
/// trait default and by any adapter whose model doesn't support tools.
/// Generic over `?Sized` so it works from both the trait default (`&Self`) and
/// a concrete adapter.
pub(crate) async fn single_shot_turn<P: AiProvider + ?Sized>(
    provider: &P,
    app: &AppHandle,
    model: &str,
    messages: &[ChatMsg],
    temperature: Option<f64>,
) -> AppResult<AgentTurn> {
    let (system, user) = flatten_messages(messages);
    let (text, usage) = provider
        .complete_with_usage(app, model, &system, &user, temperature)
        .await?;
    Ok(AgentTurn {
        text,
        tool_calls: Vec::new(),
        stop: StopReason::End,
        usage,
    })
}
