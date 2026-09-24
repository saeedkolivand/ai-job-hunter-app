//! opencode backend — the `opencode` CLI run headless with the prompt on stdin.
//!
//! Streaming uses `--format json` (JSONL), which emits one JSON event per line.
//! Text arrives as `{"type":"text",...,"part":{...,"type":"text","text":"OK",...}}`.
//! One-shot uses the same flags; the last `text` part is the answer.
//!
//! **Security (every tool call refused):** a workspace file `.opencode/opencode.json`
//! sets one top-level rule, `{ "action": "*", "resource": "*", "effect": "ask" }`.
//! `opencode run` is headless with nobody to answer, so every tool call is
//! declined, and the app never passes `--auto` (which would approve `ask`).
//! See [`OPENCODE_CONFIG`] for why this is `ask` and not `deny`.
//! Docs: https://opencode.ai/v2/docs/permissions
//!
//! The (untrusted, JD-bearing) prompt is delivered on **stdin**
//! ([`PromptDelivery::Stdin`]): `opencode run` reads the prompt from stdin, so
//! nothing prompt-derived ever reaches argv (or, on Windows, `cmd.exe` — see the
//! CVE-2024-24576 note on [`PromptDelivery`]). argv holds only fixed flags.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::commands::ai_provider::ProviderId;
use crate::error::{AppError, AppResult};

use super::{CliAgentBackend, CliEvent, CliInvocation, PromptDelivery};

const MODELS: &[&str] = &[]; // No static fallback — rely on `discover_models` entirely.

/// Written to `.opencode/opencode.json` in the private per-agent workspace.
///
/// `ask`, not `deny`, both verified live on opencode 2.0.16 with a
/// prompt-injection probe ("use your shell tool to write a file", "read
/// secret.txt"):
/// - `ask` headless: every tool call is declined ("The user declined this tool
///   call"), no file written or read, and a normal prompt still answers with
///   exit 0. OpenCode Zen's FREE models accept the request.
/// - `deny` (any form, even denying shell alone) removes tools from what the
///   model is offered, and Zen's free tier then refuses the request outright
///   ("free tier can only be used from within OpenCode"). Free Zen models are a
///   core reason to support opencode, so `deny` is not an option.
/// - With no config at all, the shell tool really runs. Never ship without this.
///
/// This relies on `--auto` NEVER being passed: `--auto` approves every
/// permission that is not explicitly denied. Pinned by `argv_never_auto_approves`.
const OPENCODE_CONFIG: &str = r#"{
  "$schema": "https://opencode.ai/config.json",
  "permissions": [
    { "action": "*", "resource": "*", "effect": "ask" }
  ]
}"#;

/// Map opencode's own error text to what the user can act on. Shared by the
/// streaming and one-shot paths so both say the same thing.
fn friendly_error(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if lower.contains("rate limit") || lower.contains("quota") {
        return "opencode: rate limit exceeded. Please try again later.".to_string();
    }
    format!("opencode: {message}")
}

pub struct OpencodeAgent;

#[async_trait]
impl CliAgentBackend for OpencodeAgent {
    fn id(&self) -> ProviderId {
        ProviderId::Opencode
    }

    fn default_binary(&self) -> &'static str {
        "opencode"
    }

    fn env_override(&self) -> &'static str {
        "OPENCODE_BIN"
    }

    fn models(&self) -> &'static [&'static str] {
        MODELS
    }

    /// Live discovery via `opencode models` — prints one `provider/model` per line.
    async fn discover_models(&self) -> Option<Vec<Value>> {
        let binary = self.binary();
        let args = vec!["models".to_string()];
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
        Some("@opencode/cli")
    }

    fn docs_url(&self) -> &'static str {
        "https://opencode.ai/docs"
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
            args: run_args(model),
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
            args: run_args(model),
            prompt: PromptDelivery::Stdin,
        }
    }

    fn parse_stream_line(&self, line: &str) -> Option<CliEvent> {
        let v: Value = serde_json::from_str(line.trim()).ok()?;
        match v.get("type").and_then(|t| t.as_str())? {
            // Token-level text deltas arrive as parts with type "text".
            "text" => v
                .get("part")
                .and_then(|p| p.get("type"))
                .and_then(|t| t.as_str())
                .filter(|t| *t == "text")
                .and_then(|_| {
                    v.get("part")
                        .and_then(|p| p.get("text"))
                        .and_then(|t| t.as_str())
                })
                .map(|s| CliEvent::Delta(s.to_string())),
            // Terminal result event.
            "result" => {
                if v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false) {
                    let msg = v
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .or_else(|| v.get("result").and_then(|r| r.as_str()))
                        .unwrap_or("opencode reported an error");
                    Some(CliEvent::Error(msg.to_string()))
                } else {
                    Some(CliEvent::Done)
                }
            }
            // The error shape opencode really emits (seen live on 2.0.16):
            // {"type":"error",…,"error":{"type":"provider.quota","message":"…","status":429}}
            "error" => Some(CliEvent::Error(friendly_error(
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("opencode reported an error"),
            ))),
            // Other event types (step_start, etc.) are ignored.
            _ => None,
        }
    }

    fn parse_complete(&self, stdout: &str) -> AppResult<String> {
        // Latest text per part id, in first-seen order: distinct parts are joined,
        // while a repeated id (an update to the same part) replaces instead of
        // duplicating.
        let mut parts: Vec<(String, String)> = Vec::new();
        let mut error: Option<String> = None;

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            match v.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    let part = v.get("part");
                    if let Some(t) = part.and_then(|p| p.get("text")).and_then(|t| t.as_str()) {
                        let id = part
                            .and_then(|p| p.get("id"))
                            .and_then(|i| i.as_str())
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("#{}", parts.len()));
                        match parts.iter_mut().find(|(pid, _)| *pid == id) {
                            Some((_, existing)) => *existing = t.to_string(),
                            None => parts.push((id, t.to_string())),
                        }
                    }
                }
                Some("result") => {
                    if v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false) {
                        error = v
                            .get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                            .or_else(|| v.get("result").and_then(|r| r.as_str()))
                            .map(str::to_string);
                    }
                }
                Some("error") => {
                    error = v
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .map(str::to_string);
                }
                _ => {}
            }
        }

        let text: String = parts.into_iter().map(|(_, t)| t).collect();
        if !text.trim().is_empty() {
            return Ok(text);
        }
        if let Some(e) = error {
            return Err(AppError::Provider(friendly_error(&e)));
        }
        Err(AppError::Provider(
            "opencode: no response in output".to_string(),
        ))
    }

    fn workspace_files(&self) -> Vec<(&'static str, String)> {
        vec![(".opencode/opencode.json", OPENCODE_CONFIG.to_string())]
    }
}

/// `opencode run --format json -m <model>`: the prompt is piped on stdin.
fn run_args(model: &str) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ];
    if let Some(model) = super::arg_token(model) {
        args.push("-m".to_string());
        args.push(model.to_string());
    }
    args
}

/// Parse `opencode models` stdout (one `provider/model` per line) into
/// `ProviderModelInfo`-shaped entries.
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
    fn parses_text_part_as_delta() {
        let line = r#"{"type":"text","part":{"type":"text","text":"Hello"}}"#;
        assert_eq!(
            OpencodeAgent.parse_stream_line(line),
            Some(CliEvent::Delta("Hello".to_string()))
        );
    }

    #[test]
    fn ignores_non_text_parts() {
        let line = r#"{"type":"text","part":{"type":"tool_call","tool":"bash"}}"#;
        assert_eq!(OpencodeAgent.parse_stream_line(line), None);
    }

    #[test]
    fn result_success_is_done() {
        let line = r#"{"type":"result","is_error":false}"#;
        assert_eq!(OpencodeAgent.parse_stream_line(line), Some(CliEvent::Done));
    }

    #[test]
    fn result_error_is_error() {
        let line = r#"{"type":"result","is_error":true,"error":{"message":"boom"}}"#;
        assert_eq!(
            OpencodeAgent.parse_stream_line(line),
            Some(CliEvent::Error("boom".to_string()))
        );
    }

    #[test]
    fn complete_replaces_an_updated_part_instead_of_duplicating_it() {
        let out = concat!(
            r#"{"type":"text","part":{"id":"p1","type":"text","text":"draft"}}"#,
            "\n",
            r#"{"type":"text","part":{"id":"p1","type":"text","text":"final answer"}}"#,
            "\n",
        );
        assert_eq!(OpencodeAgent.parse_complete(out).unwrap(), "final answer");
    }

    #[test]
    fn complete_surfaces_result_error() {
        let out = r#"{"type":"result","is_error":true,"error":{"message":"Rate limit exceeded"}}"#;
        let err = OpencodeAgent.parse_complete(out).unwrap_err();
        assert!(format!("{err}").contains("rate limit"));
    }

    #[test]
    fn complete_errors_on_empty() {
        let out = "{}";
        assert!(OpencodeAgent.parse_complete(out).is_err());
    }

    #[test]
    fn argv_has_isolation_flags_and_model_only_prompt_on_stdin() {
        let inv = OpencodeAgent.stream_invocation("openai/gpt-4o", "system text", None);
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
        assert!(inv.args.iter().any(|a| a == "run"));
        assert!(inv.args.iter().any(|a| a == "--format"));
        assert!(inv.args.iter().any(|a| a == "json"));
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "-m" && w[1] == "openai/gpt-4o"));
    }

    #[test]
    fn argv_no_model_omits_flag() {
        let inv = OpencodeAgent.stream_invocation("", "", None);
        assert!(!inv.args.iter().any(|a| a == "-m"));
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
    }

    #[test]
    fn workspace_files_returns_deny_config() {
        let files = OpencodeAgent.workspace_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, ".opencode/opencode.json");
        let config: Value = serde_json::from_str(&files[0].1).unwrap();
        // The whole rule set, exactly: one top-level ask-everything rule and
        // nothing else (see OPENCODE_CONFIG for why ask and not deny).
        assert_eq!(
            config["permissions"],
            json!([{ "action": "*", "resource": "*", "effect": "ask" }])
        );
        assert!(config.get("agent").is_none() && config.get("agents").is_none());
    }

    #[test]
    fn parse_models_output_handles_provider_model_lines() {
        let out = "openai/gpt-4o\nanthropic/claude-3-5-sonnet\n";
        let entries = parse_models_output(out).unwrap();
        assert_eq!(
            entries,
            vec![
                json!({ "name": "openai/gpt-4o" }),
                json!({ "name": "anthropic/claude-3-5-sonnet" }),
            ]
        );
    }

    #[test]
    fn parse_models_output_empty_returns_none() {
        assert_eq!(parse_models_output(""), None);
        assert_eq!(parse_models_output("   \n  \n"), None);
    }

    #[test]
    fn quota_error_maps_to_friendly_message() {
        let out = r#"{"type":"result","is_error":true,"error":{"type":"provider.quota","message":"Rate limit exceeded. Please try again later.","status":429}}"#;
        let err = OpencodeAgent.parse_complete(out).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("rate limit exceeded"));
        assert!(msg.contains("try again later"));
    }
    #[test]
    fn stream_maps_the_real_error_event() {
        let line = r#"{"type":"error","timestamp":1,"sessionID":"s","error":{"type":"provider.quota","message":"Rate limit exceeded. Please try again later.","status":429}}"#;
        match OpencodeAgent.parse_stream_line(line) {
            Some(CliEvent::Error(m)) => assert!(m.contains("rate limit exceeded"), "{m}"),
            other => panic!("expected an error event, got {other:?}"),
        }
    }

    #[test]
    fn one_shot_joins_every_text_part() {
        let out = concat!(
            r#"{"type":"step_start","part":{"type":"step-start"}}"#,
            "
",
            r#"{"type":"text","part":{"type":"text","text":"Hello, "}}"#,
            "
",
            r#"{"type":"text","part":{"type":"text","text":"world"}}"#,
            "
",
        );
        assert_eq!(OpencodeAgent.parse_complete(out).unwrap(), "Hello, world");
    }

    /// `ask` only refuses tools while nobody approves them: an auto-approve flag
    /// would turn every `ask` into `allow`.
    #[test]
    fn argv_never_auto_approves() {
        for inv in [
            OpencodeAgent.stream_invocation("openai/gpt-4o", "", None),
            OpencodeAgent.complete_invocation("openai/gpt-4o", "", None),
        ] {
            for flag in ["--auto", "--yolo", "-y", "--dangerously-skip-permissions"] {
                assert!(!inv.args.iter().any(|a| a == flag), "{flag} present");
            }
        }
    }
}
