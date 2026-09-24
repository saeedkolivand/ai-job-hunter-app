//! Cursor CLI backend — the `cursor-agent` CLI run headless with the prompt on stdin.
//!
//! Streaming uses `-p --output-format stream-json --stream-partial-output`,
//! which emits assistant events. With `--stream-partial-output`, **only
//! assistant events with `timestamp_ms` present and `model_call_id` absent are
//! new text: append those.** Both present = buffered pre-tool flush: skip.
//! Both absent = final flush: skip (else text doubles). Terminal event:
//! `{"type":"result","subtype":"success","is_error":false,"result":"<full text>",…}`.
//! On failure: non-zero exit, stderr message, no JSON.
//!
//! One-shot: `--output-format json` → a single `{"type":"result",…,"result":"…"}`.
//!
//! Models: `cursor-agent --list-models` prints one model id per line.
//!
//! **Security (tools off): a workspace file `.cursor/cli.json` = `{"permissions":{"deny":["Shell(*)","Read(**)","Write(**)","Mcp(*:*)"]}}`
//! (https://cursor.com/docs/cli/reference/permissions: deny beats allow).
//! Never pass `--force`.
//!
//! The (untrusted, JD-bearing) prompt is delivered on **stdin**
//! ([`PromptDelivery::Stdin`]): confirmed in docs that headless mode reads
//! from stdin when `-p` is given without a prompt argument. Nothing
//! prompt-derived ever reaches argv (or, on Windows, `cmd.exe` — see the
//! CVE-2024-24576 note on [`PromptDelivery`]). argv holds only fixed flags.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::commands::ai_provider::ProviderId;
use crate::error::{AppError, AppResult};

use super::{CliAgentBackend, CliEvent, CliInvocation, PromptDelivery};

const MODELS: &[&str] = &[]; // No static fallback — rely on `discover_models` entirely.

/// The config that denies every tool action. Written to `.cursor/cli.json`
/// in the private per-agent workspace (see [`super::mod::workspace_files`]).
const CURSOR_CLI_CONFIG: &str = r#"{
  "permissions": {
    "deny": ["Shell(*)", "Read(**)", "Write(**)", "Mcp(*:*)"]
  }
}"#;

pub struct CursorAgent;

#[async_trait]
impl CliAgentBackend for CursorAgent {
    fn id(&self) -> ProviderId {
        ProviderId::Cursor
    }

    fn default_binary(&self) -> &'static str {
        "cursor-agent"
    }

    fn env_override(&self) -> &'static str {
        "CURSOR_AGENT_BIN"
    }

    fn models(&self) -> &'static [&'static str] {
        MODELS
    }

    /// Live discovery via `cursor-agent --list-models` — prints one model id per line.
    async fn discover_models(&self) -> Option<Vec<Value>> {
        let binary = self.binary();
        let args = vec!["--list-models".to_string()];
        let out = tokio::time::timeout(
            super::super::timeouts::LIST_MODELS_TOTAL,
            super::cli_command(&binary, &args).output(),
        )
        .await
        .ok()?
        .ok()?;
        if !out.status.success() {
            return None;
        }
        parse_models_output(&String::from_utf8_lossy(&out.stdout))
    }

    fn install_package(&self) -> Option<&'static str> {
        // Not on npm — no one-click install, only the docs/guide path.
        None
    }

    fn docs_url(&self) -> &'static str {
        "https://cursor.com/docs/cli"
    }

    fn inline_system(&self) -> bool {
        // No system-prompt flag — harness inlines system prompt onto user prompt.
        true
    }

    fn stream_invocation(
        &self,
        model: &str,
        _system: &str,
        _effort: Option<&str>,
    ) -> CliInvocation {
        CliInvocation {
            args: stream_args(model),
            prompt: PromptDelivery::Stdin,
        }
    }

    fn complete_invocation(
        &self,
        model: &str,
        _system: &str,
        _effort: Option<&str>,
    ) -> CliInvocation {
        CliInvocation {
            args: complete_args(model),
            prompt: PromptDelivery::Stdin,
        }
    }

    fn parse_stream_line(&self, line: &str) -> Option<CliEvent> {
        let v: Value = serde_json::from_str(line.trim()).ok()?;
        match v.get("type").and_then(|t| t.as_str())? {
            "assistant" => {
                let message = v.get("message")?;
                let content = message.get("content")?.as_array()?;
                // Extract text from content array
                let text = super::text_blocks(content);

                let has_timestamp = v.get("timestamp_ms").is_some();
                let has_model_call_id = v.get("model_call_id").is_some();

                // Only emit new text deltas when timestamp_ms present AND model_call_id absent
                if has_timestamp && !has_model_call_id && !text.is_empty() {
                    Some(CliEvent::Delta(text))
                } else {
                    // Buffered pre-tool flush (both present) or final flush (both absent) — skip
                    None
                }
            }
            "result" => {
                let is_error = v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
                if is_error {
                    let msg = v
                        .get("result")
                        .and_then(|r| r.as_str())
                        .unwrap_or("Cursor CLI reported an error");
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
            .map_err(|e| AppError::Provider(format!("Cursor CLI: invalid JSON output: {e}")))?;
        if v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false) {
            let msg = v
                .get("result")
                .and_then(|r| r.as_str())
                .unwrap_or("Cursor CLI reported an error");
            return Err(AppError::Provider(format!("Cursor CLI: {msg}")));
        }
        v.get("result")
            .and_then(|r| r.as_str())
            .map(String::from)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::Provider("Cursor CLI: empty result".to_string()))
    }

    fn workspace_files(&self) -> Vec<(&'static str, String)> {
        vec![(".cursor/cli.json", CURSOR_CLI_CONFIG.to_string())]
    }
}

/// Fixed prompt argument for Cursor CLI — a compile-time constant that tells
/// the agent to read the actual instructions from stdin. Never user or
/// posting text. See https://cursor.com/docs/cli/headless.
const CURSOR_FIXED_PROMPT: &str = "Follow the instructions provided on standard input exactly.";

/// `cursor-agent -p "Follow the instructions provided on standard input exactly."
/// --output-format stream-json --stream-partial-output --model <model>`
/// with the REAL prompt on stdin.
fn stream_args(model: &str) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        CURSOR_FIXED_PROMPT.to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--stream-partial-output".to_string(),
    ];
    if let Some(model) = super::arg_token(model) {
        args.push("--model".to_string());
        args.push(model.to_string());
    }
    args
}

/// `cursor-agent -p "Follow the instructions provided on standard input exactly."
/// --output-format json --model <model>` with the REAL prompt on stdin.
fn complete_args(model: &str) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        CURSOR_FIXED_PROMPT.to_string(),
        "--output-format".to_string(),
        "json".to_string(),
    ];
    if let Some(model) = super::arg_token(model) {
        args.push("--model".to_string());
        args.push(model.to_string());
    }
    args
}

/// Parse `cursor-agent --list-models` stdout (one model id per line).
fn parse_models_output(stdout: &str) -> Option<Vec<Value>> {
    let entries: Vec<Value> = stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| json!({ "name": l }))
        .collect();
    (!entries.is_empty()).then_some(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stream_partial_delta() {
        // timestamp_ms present, model_call_id absent = new text delta
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello"}]},"timestamp_ms":12345}"#;
        assert_eq!(
            CursorAgent.parse_stream_line(line),
            Some(CliEvent::Delta("Hello".to_string()))
        );
    }

    #[test]
    fn ignores_buffered_pre_tool_flush() {
        // Both timestamp_ms and model_call_id present = buffered pre-tool flush
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello"}]},"timestamp_ms":12345,"model_call_id":"call_123"}"#;
        assert_eq!(CursorAgent.parse_stream_line(line), None);
    }

    #[test]
    fn ignores_final_flush() {
        // Both absent = final flush (would double text)
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello"}]}}"#;
        assert_eq!(CursorAgent.parse_stream_line(line), None);
    }

    #[test]
    fn result_success_is_done() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"final"}"#;
        assert_eq!(CursorAgent.parse_stream_line(line), Some(CliEvent::Done));
    }

    #[test]
    fn result_error_is_error() {
        let line = r#"{"type":"result","subtype":"error","is_error":true,"result":"boom"}"#;
        assert_eq!(
            CursorAgent.parse_stream_line(line),
            Some(CliEvent::Error("boom".to_string()))
        );
    }

    #[test]
    fn complete_extracts_result() {
        let out = r#"{"type":"result","subtype":"success","is_error":false,"result":"final text"}"#;
        assert_eq!(CursorAgent.parse_complete(out).unwrap(), "final text");
    }

    #[test]
    fn complete_errors_on_is_error() {
        let out = r#"{"type":"result","is_error":true,"result":"nope"}"#;
        assert!(CursorAgent.parse_complete(out).is_err());
    }

    #[test]
    fn argv_stream_has_correct_flags_and_prompt_on_stdin() {
        let inv = CursorAgent.stream_invocation("gpt-4o", "system text", None);
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
        // Fixed trusted prompt argument (compile-time constant)
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "-p" && w[1] == CURSOR_FIXED_PROMPT));
        assert!(inv.args.iter().any(|a| a == "--output-format"));
        assert!(inv.args.iter().any(|a| a == "stream-json"));
        assert!(inv.args.iter().any(|a| a == "--stream-partial-output"));
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "--model" && w[1] == "gpt-4o"));
        // Never --force
        assert!(!inv.args.iter().any(|a| a == "--force"));
    }

    #[test]
    fn argv_complete_has_correct_flags_and_prompt_on_stdin() {
        let inv = CursorAgent.complete_invocation("gpt-4o", "system text", None);
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
        // Fixed trusted prompt argument (compile-time constant)
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "-p" && w[1] == CURSOR_FIXED_PROMPT));
        assert!(inv.args.iter().any(|a| a == "--output-format"));
        assert!(inv.args.iter().any(|a| a == "json"));
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "--model" && w[1] == "gpt-4o"));
        assert!(!inv.args.iter().any(|a| a == "--force"));
    }

    #[test]
    fn workspace_files_returns_deny_config() {
        let files = CursorAgent.workspace_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, ".cursor/cli.json");
        let config: Value = serde_json::from_str(&files[0].1).unwrap();
        let deny = config["permissions"]["deny"].as_array().unwrap();
        assert!(deny.iter().any(|v| v.as_str() == Some("Shell(*)")));
        assert!(deny.iter().any(|v| v.as_str() == Some("Read(**)")));
        assert!(deny.iter().any(|v| v.as_str() == Some("Write(**)")));
        assert!(deny.iter().any(|v| v.as_str() == Some("Mcp(*:*)")));
    }

    #[test]
    fn parse_models_output_handles_model_lines() {
        let out = "gpt-4o\ngpt-4o-mini\n";
        let entries = parse_models_output(out).unwrap();
        assert_eq!(
            entries,
            vec![
                json!({ "name": "gpt-4o" }),
                json!({ "name": "gpt-4o-mini" }),
            ]
        );
    }

    #[test]
    fn parse_models_output_empty_returns_none() {
        assert_eq!(parse_models_output(""), None);
        assert_eq!(parse_models_output("   \n  \n"), None);
    }
}
