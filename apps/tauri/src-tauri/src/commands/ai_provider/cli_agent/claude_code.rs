//! Claude Code backend — Anthropic's `claude` CLI run headless.
//!
//! Streaming uses `--output-format stream-json --include-partial-messages`, which
//! emits token-level `content_block_delta` events (mirroring the Anthropic
//! Messages API), terminated by a `result` event. One-shot `complete` uses
//! `--output-format json` and reads the top-level `.result`. Mutating/exec tools
//! are disabled and the harness runs in a neutral cwd, so this is text-only.

use async_trait::async_trait;
use serde_json::Value;

use crate::commands::ai_provider::ProviderId;
use crate::error::{AppError, AppResult};

use super::{CliAgentBackend, CliEvent, CliInvocation, PromptDelivery};

/// Tools the agent may not use — we only want text, never filesystem/network/exec
/// side effects. Read-only tools are additionally constrained by the temp cwd.
const DISALLOWED_TOOLS: &str = "Bash,Edit,Write,MultiEdit,NotebookEdit,WebFetch,WebSearch,Task";

const MODELS: &[&str] = &["sonnet", "opus", "haiku"];

pub struct ClaudeCodeAgent;

#[async_trait]
impl CliAgentBackend for ClaudeCodeAgent {
    fn id(&self) -> ProviderId {
        ProviderId::ClaudeCode
    }

    fn default_binary(&self) -> &'static str {
        "claude"
    }

    fn env_override(&self) -> &'static str {
        "CLAUDE_CODE_BIN"
    }

    fn models(&self) -> &'static [&'static str] {
        MODELS
    }

    fn install_package(&self) -> &'static str {
        "@anthropic-ai/claude-code"
    }

    fn docs_url(&self) -> &'static str {
        "https://code.claude.com/docs/en/setup"
    }

    // Claude Code has no headless reasoning-effort flag — `effort` is ignored.
    fn stream_invocation(&self, model: &str, system: &str, _effort: Option<&str>) -> CliInvocation {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--include-partial-messages".to_string(),
            "--disallowedTools".to_string(),
            DISALLOWED_TOOLS.to_string(),
        ];
        push_model_system(&mut args, model, system);
        CliInvocation {
            args,
            prompt: PromptDelivery::Stdin,
        }
    }

    fn complete_invocation(
        &self,
        model: &str,
        system: &str,
        _effort: Option<&str>,
    ) -> CliInvocation {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "json".to_string(),
            "--disallowedTools".to_string(),
            DISALLOWED_TOOLS.to_string(),
        ];
        push_model_system(&mut args, model, system);
        CliInvocation {
            args,
            prompt: PromptDelivery::Stdin,
        }
    }

    fn parse_stream_line(&self, line: &str) -> Option<CliEvent> {
        let v: Value = serde_json::from_str(line).ok()?;
        match v.get("type").and_then(|t| t.as_str())? {
            // Token-level deltas (require --include-partial-messages). The final
            // consolidated `assistant` message is intentionally ignored so text is
            // never emitted twice.
            "stream_event" => {
                let event = v.get("event")?;
                if event.get("type").and_then(|t| t.as_str())? != "content_block_delta" {
                    return None;
                }
                let delta = event.get("delta")?;
                match delta.get("type").and_then(|t| t.as_str())? {
                    "text_delta" => delta
                        .get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| CliEvent::Delta(s.to_string())),
                    "thinking_delta" => delta
                        .get("thinking")
                        .and_then(|t| t.as_str())
                        .map(|s| CliEvent::Thinking(s.to_string())),
                    _ => None,
                }
            }
            "result" => {
                if v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false) {
                    let msg = v
                        .get("result")
                        .and_then(|r| r.as_str())
                        .or_else(|| v.get("error").and_then(|e| e.as_str()))
                        .unwrap_or("Claude Code reported an error");
                    Some(CliEvent::Error(msg.to_string()))
                } else {
                    Some(CliEvent::Done)
                }
            }
            _ => None,
        }
    }

    fn parse_complete(&self, stdout: &str) -> AppResult<String> {
        let v: Value = serde_json::from_str(stdout.trim())
            .map_err(|e| AppError::Provider(format!("Claude Code: invalid JSON output: {e}")))?;
        if v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false) {
            let msg = v
                .get("result")
                .and_then(|r| r.as_str())
                .unwrap_or("Claude Code reported an error");
            return Err(AppError::Provider(format!("Claude Code: {msg}")));
        }
        v.get("result")
            .and_then(|r| r.as_str())
            .map(String::from)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::Provider("Claude Code: empty result".to_string()))
    }
}

fn push_model_system(args: &mut Vec<String>, model: &str, system: &str) {
    if !model.trim().is_empty() {
        args.push("--model".to_string());
        args.push(model.to_string());
    }
    if !system.trim().is_empty() {
        args.push("--append-system-prompt".to_string());
        args.push(system.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_delta() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}}"#;
        assert_eq!(
            ClaudeCodeAgent.parse_stream_line(line),
            Some(CliEvent::Delta("Hello".to_string()))
        );
    }

    #[test]
    fn parses_thinking_delta() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hmm"}}}"#;
        assert_eq!(
            ClaudeCodeAgent.parse_stream_line(line),
            Some(CliEvent::Thinking("hmm".to_string()))
        );
    }

    #[test]
    fn result_success_is_done() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"hi"}"#;
        assert_eq!(
            ClaudeCodeAgent.parse_stream_line(line),
            Some(CliEvent::Done)
        );
    }

    #[test]
    fn result_error_is_error() {
        let line = r#"{"type":"result","subtype":"error","is_error":true,"result":"boom"}"#;
        assert_eq!(
            ClaudeCodeAgent.parse_stream_line(line),
            Some(CliEvent::Error("boom".to_string()))
        );
    }

    #[test]
    fn ignores_system_and_assistant_events() {
        assert_eq!(
            ClaudeCodeAgent.parse_stream_line(r#"{"type":"system","subtype":"init"}"#),
            None
        );
        assert_eq!(
            ClaudeCodeAgent.parse_stream_line(
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"x"}]}}"#
            ),
            None
        );
    }

    #[test]
    fn complete_extracts_result() {
        let out = r#"{"type":"result","is_error":false,"result":"final text"}"#;
        assert_eq!(ClaudeCodeAgent.parse_complete(out).unwrap(), "final text");
    }

    #[test]
    fn complete_errors_on_is_error() {
        let out = r#"{"type":"result","is_error":true,"result":"nope"}"#;
        assert!(ClaudeCodeAgent.parse_complete(out).is_err());
    }

    #[test]
    fn stream_invocation_includes_model_and_system() {
        let inv = ClaudeCodeAgent.stream_invocation("sonnet", "be brief", None);
        assert!(inv.args.iter().any(|a| a == "stream-json"));
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "--model" && w[1] == "sonnet"));
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "--append-system-prompt" && w[1] == "be brief"));
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
    }

    #[test]
    fn empty_model_omits_flag() {
        let inv = ClaudeCodeAgent.stream_invocation("", "", None);
        assert!(!inv.args.iter().any(|a| a == "--model"));
        assert!(!inv.args.iter().any(|a| a == "--append-system-prompt"));
    }
}
