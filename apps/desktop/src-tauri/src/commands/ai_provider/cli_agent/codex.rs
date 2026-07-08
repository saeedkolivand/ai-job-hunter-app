//! OpenAI Codex backend — the `codex` CLI run non-interactively (`codex exec`).
//!
//! Uses `--json` (JSONL event stream) and surfaces the agent's messages, and a
//! read-only sandbox so a generation can never modify the filesystem. Codex has no
//! system-prompt flag, so the harness inlines the system prompt
//! ([`CliAgentBackend::inline_system`]). Authenticates with the user's ChatGPT
//! login (or `OPENAI_API_KEY`). Output is surfaced at message granularity — robust
//! across Codex versions whether or not token deltas are emitted.
//!
//! The (untrusted, JD-bearing) prompt is delivered on **stdin**
//! ([`PromptDelivery::Stdin`]): `codex exec` reads the prompt from stdin when no
//! positional prompt argument is given, so nothing prompt-derived ever reaches
//! argv (and, on Windows, `cmd.exe` — see the CVE-2024-24576 note on
//! [`PromptDelivery`]). argv holds only the fixed exec flags below.

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

    fn install_package(&self) -> &'static str {
        "@openai/codex"
    }

    fn docs_url(&self) -> &'static str {
        "https://developers.openai.com/codex/cli"
    }

    fn inline_system(&self) -> bool {
        true
    }

    fn stream_invocation(&self, model: &str, _system: &str, effort: Option<&str>) -> CliInvocation {
        CliInvocation {
            args: exec_args(model, effort),
            // Prompt on stdin, not argv — `codex exec` reads stdin when given no
            // positional prompt. Keeps untrusted JD text off the command line.
            prompt: PromptDelivery::Stdin,
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
            prompt: PromptDelivery::Stdin,
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
        if ty.contains("reasoning") {
            // Codex streams chain-of-thought as `agent_reasoning*` events; surface
            // it as thinking, like the cloud providers. Section-break markers carry
            // no text and are dropped by `text_of`.
            return text_of(m).map(CliEvent::Thinking);
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
        // The harness runs in a neutral temp cwd, which is not a git repo; without
        // this, `codex exec` refuses to run ("Not inside a trusted directory…") or
        // blocks on an approval prompt. Read-only sandbox already bars side effects.
        "--skip-git-repo-check".to_string(),
    ];
    // `arg_token` upholds the CVE-2024-24576 argv invariant defensively: model/effort
    // are user settings (not scraped), but still ride argv through `cmd.exe` on
    // Windows, so reject anything that isn't a plain identifier (drops the flag).
    if let Some(model) = super::arg_token(model) {
        args.push("--model".to_string());
        args.push(model.to_string());
    }
    // Reasoning effort via a config override (low/medium/high). Omitted → Codex default.
    if let Some(effort) = effort.and_then(super::arg_token) {
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
    fn agent_reasoning_becomes_thinking() {
        let line = r#"{"msg":{"type":"agent_reasoning","text":"weighing options"}}"#;
        assert_eq!(
            CodexAgent.parse_stream_line(line),
            Some(CliEvent::Thinking("weighing options".to_string()))
        );
    }

    #[test]
    fn empty_reasoning_section_break_is_ignored() {
        // Section-break markers carry no text — `text_of` filters them out.
        let line = r#"{"msg":{"type":"agent_reasoning_section_break"}}"#;
        assert_eq!(CodexAgent.parse_stream_line(line), None);
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
        // Prompt is delivered on stdin, never as a positional argv element — so no
        // untrusted JD text can reach `cmd.exe` on Windows (CVE-2024-24576).
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
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
        // Runs outside a git repo (temp cwd) — the check must be skipped.
        assert!(inv.args.iter().any(|a| a == "--skip-git-repo-check"));
    }

    #[test]
    fn argv_is_only_static_flags_never_the_prompt() {
        // The full argv is a fixed, trusted set of exec flags (+ resolved model) —
        // it never contains prompt/JD-derived text. That is the property that clears
        // the command-injection CRITICAL: with `Stdin` delivery the harness pipes the
        // prompt to the child, so nothing untrusted ever reaches argv / `cmd.exe`.
        let inv = CodexAgent.stream_invocation("o4-mini", "system text here", None);
        assert_eq!(
            inv.args,
            vec![
                "exec",
                "--json",
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                "--model",
                "o4-mini",
            ]
        );
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
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
