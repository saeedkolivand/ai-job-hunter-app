//! OpenAI Codex backend — the `codex` CLI run non-interactively (`codex exec`).
//!
//! Uses `--json` (JSONL event stream) and surfaces the agent's messages, and a
//! read-only sandbox so a generation can never modify the filesystem. Codex has no
//! system-prompt flag, so the harness inlines the system prompt
//! ([`CliAgentBackend::inline_system`]). Authenticates with the user's ChatGPT
//! login (or `OPENAI_API_KEY`). Output is surfaced at message granularity — robust
//! across Codex versions whether or not token deltas are emitted.

use async_trait::async_trait;
use serde_json::Value;

use crate::commands::ai_provider::ProviderId;
use crate::error::{AppError, AppResult};

use super::{CliAgentBackend, CliEvent, CliInvocation, PromptDelivery};

const MODELS: &[&str] = &["gpt-5-codex", "o4-mini"];

pub struct CodexAgent;

#[async_trait]
impl CliAgentBackend for CodexAgent {
    fn id(&self) -> ProviderId {
        ProviderId::Codex
    }

    fn default_binary(&self) -> &'static str {
        "codex"
    }

    fn env_override(&self) -> &'static str {
        "CODEX_BIN"
    }

    fn models(&self) -> &'static [&'static str] {
        MODELS
    }

    fn inline_system(&self) -> bool {
        true
    }

    fn stream_invocation(&self, model: &str, _system: &str, effort: Option<&str>) -> CliInvocation {
        CliInvocation {
            args: exec_args(model, effort),
            prompt: PromptDelivery::Arg,
        }
    }

    fn complete_invocation(
        &self,
        model: &str,
        _system: &str,
        effort: Option<&str>,
    ) -> CliInvocation {
        CliInvocation {
            args: exec_args(model, effort),
            prompt: PromptDelivery::Arg,
        }
    }

    fn parse_stream_line(&self, line: &str) -> Option<CliEvent> {
        let v: Value = serde_json::from_str(line.trim()).ok()?;
        let m = inner(&v);
        let ty = m.get("type").and_then(|t| t.as_str())?;
        if ty.contains("error") {
            return Some(CliEvent::Error(
                text_of(m).unwrap_or_else(|| "Codex reported an error".to_string()),
            ));
        }
        if ty == "agent_message" {
            // Emit at message granularity (Codex's canonical assistant output).
            return text_of(m).map(CliEvent::Delta);
        }
        if ty.contains("task_complete") || ty.contains("turn_complete") {
            return Some(CliEvent::Done);
        }
        None
    }

    fn parse_complete(&self, stdout: &str) -> AppResult<String> {
        let mut last_message: Option<String> = None;
        let mut error: Option<String> = None;
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let m = inner(&v);
            match m.get("type").and_then(|t| t.as_str()) {
                Some("agent_message") => last_message = text_of(m).or(last_message),
                Some(t) if t.contains("error") => error = text_of(m).or(error),
                _ => {}
            }
        }
        if let Some(text) = last_message {
            return Ok(text);
        }
        if let Some(e) = error {
            return Err(AppError::Provider(format!("Codex: {e}")));
        }
        Err(AppError::Provider(
            "Codex: no response in output".to_string(),
        ))
    }
}

fn exec_args(model: &str, effort: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "exec".to_string(),
        "--json".to_string(),
        "--sandbox".to_string(),
        "read-only".to_string(),
    ];
    if !model.trim().is_empty() {
        args.push("--model".to_string());
        args.push(model.to_string());
    }
    // Reasoning effort via a config override (low/medium/high). Omitted → Codex default.
    if let Some(effort) = effort.map(str::trim).filter(|e| !e.is_empty()) {
        args.push("-c".to_string());
        args.push(format!("model_reasoning_effort={effort}"));
    }
    args
}

/// Codex wraps each event under `msg`; fall back to the top level for safety.
fn inner(v: &Value) -> &Value {
    v.get("msg").unwrap_or(v)
}

/// Pull assistant text from a Codex event under any of the field names it has used.
fn text_of(m: &Value) -> Option<String> {
    ["message", "text", "delta"]
        .iter()
        .find_map(|k| m.get(*k).and_then(|x| x.as_str()))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_message_becomes_delta() {
        let line = r#"{"msg":{"type":"agent_message","message":"Hello there"}}"#;
        assert_eq!(
            CodexAgent.parse_stream_line(line),
            Some(CliEvent::Delta("Hello there".to_string()))
        );
    }

    #[test]
    fn error_event_becomes_error() {
        let line = r#"{"msg":{"type":"error","message":"boom"}}"#;
        assert_eq!(
            CodexAgent.parse_stream_line(line),
            Some(CliEvent::Error("boom".to_string()))
        );
    }

    #[test]
    fn task_complete_is_done() {
        let line = r#"{"msg":{"type":"task_complete"}}"#;
        assert_eq!(CodexAgent.parse_stream_line(line), Some(CliEvent::Done));
    }

    #[test]
    fn parse_complete_returns_final_message() {
        let out = "{\"msg\":{\"type\":\"agent_message\",\"message\":\"first\"}}\n\
                   {\"msg\":{\"type\":\"task_started\"}}\n\
                   {\"msg\":{\"type\":\"agent_message\",\"message\":\"final answer\"}}\n";
        assert_eq!(CodexAgent.parse_complete(out).unwrap(), "final answer");
    }

    #[test]
    fn exec_args_include_sandbox_and_model() {
        let inv = CodexAgent.stream_invocation("o4-mini", "", None);
        assert_eq!(inv.prompt, PromptDelivery::Arg);
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "--sandbox" && w[1] == "read-only"));
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "--model" && w[1] == "o4-mini"));
        // No effort → no reasoning-effort override.
        assert!(!inv
            .args
            .iter()
            .any(|a| a.starts_with("model_reasoning_effort=")));
    }

    #[test]
    fn effort_adds_reasoning_config_override() {
        let inv = CodexAgent.stream_invocation("o4-mini", "", Some("high"));
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "-c" && w[1] == "model_reasoning_effort=high"));
        // Blank effort is treated as none.
        let blank = CodexAgent.stream_invocation("o4-mini", "", Some("  "));
        assert!(!blank
            .args
            .iter()
            .any(|a| a.starts_with("model_reasoning_effort=")));
    }

    #[test]
    fn inlines_system_prompt() {
        assert!(CodexAgent.inline_system());
    }
}
