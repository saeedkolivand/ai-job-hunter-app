//! Qwen Code backend — the `qwen` CLI run headless with the prompt on stdin.
//!
//! Streaming uses `--output-format stream-json` (JSONL).
//! Assistant event: `{"type":"assistant","message":{"content":[{"type":"text","text":"..."}]}}`.
//! One-shot: `--output-format json` → a JSON **array** of messages; the final
//! element is `{"type":"result","subtype":"success","result":"...",…}`.
//!
//! **Tools off:** a settings file written into the per-spawn workspace and loaded
//! as Qwen's SYSTEM settings via `QWEN_CODE_SYSTEM_SETTINGS_PATH`. System
//! settings override user and project settings, and they are an operator scope
//! that still applies in an untrusted folder, where Qwen ignores project
//! `.qwen/settings.json`. It denies every built-in tool in [`QWEN_TOOLS`] through
//! `permissions.deny` (deny beats ask and allow) and repeats them in
//! `tools.exclude` for versions that only know that key. MCP tools are exempt
//! from deny rules, so `--allowed-mcp-server-names` with a sentinel keeps every
//! MCP server off, and `--approval-mode plan` stays as a last layer. Never
//! `--yolo`. Docs only: not yet verified against a live Qwen Code.
//! Docs: https://github.com/QwenLM/qwen-code/blob/main/docs/users/configuration/settings.md
//!
//! `--json-schema` is used for structured calls
//! (https://qwenlm.github.io/qwen-code-docs/en/users/features/structured-output/).
//!
//! The (untrusted, JD-bearing) prompt is delivered on **stdin**
//! ([`PromptDelivery::Stdin`]): `echo "task" | qwen` is documented, so we pipe
//! the prompt to stdin. Nothing prompt-derived ever reaches argv (or, on
//! Windows, `cmd.exe` — see the CVE-2024-24576 note on [`PromptDelivery`]).
//! argv holds only fixed flags.

use async_trait::async_trait;
use serde_json::Value;

use crate::commands::ai_provider::ProviderId;
use crate::error::{AppError, AppResult};

use super::{CliAgentBackend, CliEvent, CliInvocation, PromptDelivery};

/// Fallback when discovery finds nothing: the model the Qwen Code docs use in
/// their own examples. Picking none runs the CLI's configured default.
const MODELS: &[&str] = &["qwen3-coder-plus"];

/// Argv cap for `--json-schema` (UTF-16 code units, same as Claude Code).
const MAX_JSON_SCHEMA_ARG_CHARS: usize = 16_384;

/// Every built-in Qwen Code tool name the docs reference, plus the bridge tools
/// (`tool_search`, `tool_call`) that can reach other tools. A name Qwen doesn't
/// know is harmless in a deny list; a missing one is a tool left enabled.
const QWEN_TOOLS: &[&str] = &[
    "run_shell_command",
    "read_file",
    "read_many_files",
    "write_file",
    "edit",
    "replace",
    "grep",
    "grep_search",
    "search_file_content",
    "glob",
    "list_directory",
    "web_fetch",
    "web_search",
    "google_web_search",
    "save_memory",
    "todo_write",
    "task",
    "skill",
    "exit_plan_mode",
    "lsp",
    "tool_search",
    "tool_call",
];

/// Where the system settings file goes in the workspace, and the variable
/// that points Qwen at it.
const SYSTEM_SETTINGS_FILE: &str = "qwen-system-settings.json";
const SYSTEM_SETTINGS_ENV: &str = "QWEN_CODE_SYSTEM_SETTINGS_PATH";

fn system_settings() -> String {
    serde_json::json!({
        "permissions": { "deny": QWEN_TOOLS },
        "tools": { "exclude": QWEN_TOOLS },
    })
    .to_string()
}

pub struct QwenCodeAgent;

#[async_trait]
impl CliAgentBackend for QwenCodeAgent {
    fn id(&self) -> ProviderId {
        ProviderId::QwenCode
    }

    fn default_binary(&self) -> &'static str {
        "qwen"
    }

    fn env_override(&self) -> &'static str {
        "QWEN_CODE_BIN"
    }

    fn models(&self) -> &'static [&'static str] {
        MODELS
    }

    /// Live discovery — Qwen Code doesn't have a documented model listing CLI.
    /// Falls back to static `models()`.
    async fn discover_models(&self) -> Option<Vec<Value>> {
        None
    }

    fn install_package(&self) -> Option<&'static str> {
        Some("@qwen-code/qwen-code")
    }

    fn docs_url(&self) -> &'static str {
        "https://qwenlm.github.io/qwen-code-docs/en/users/features/headless/"
    }

    fn workspace_files(&self) -> Vec<(&'static str, String)> {
        vec![(SYSTEM_SETTINGS_FILE, system_settings())]
    }

    fn workspace_env(&self) -> Vec<(&'static str, &'static str)> {
        vec![(SYSTEM_SETTINGS_ENV, SYSTEM_SETTINGS_FILE)]
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
            args: build_args(model, true, None),
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
            args: build_args(model, false, None),
            prompt: PromptDelivery::Stdin,
        }
    }

    fn parse_stream_line(&self, line: &str) -> Option<CliEvent> {
        let v: Value = serde_json::from_str(line.trim()).ok()?;
        match v.get("type").and_then(|t| t.as_str())? {
            "assistant" => {
                let message = v.get("message")?;
                let content = message.get("content")?.as_array()?;
                let text = super::text_blocks(content);
                if !text.is_empty() {
                    Some(CliEvent::Delta(text))
                } else {
                    None
                }
            }
            "result" => {
                let is_error = v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
                if is_error {
                    let msg = v
                        .get("result")
                        .and_then(|r| r.as_str())
                        .unwrap_or("Qwen Code reported an error");
                    Some(CliEvent::Error(msg.to_string()))
                } else {
                    Some(CliEvent::Done)
                }
            }
            _ => None,
        }
    }

    fn parse_complete(&self, stdout: &str) -> AppResult<String> {
        // One-shot output is a JSON array; the final element carries the result.
        let v: Value = serde_json::from_str(stdout.trim())
            .map_err(|e| AppError::Provider(format!("Qwen Code: invalid JSON output: {e}")))?;
        let arr = v.as_array().ok_or_else(|| {
            AppError::Provider("Qwen Code: expected JSON array output".to_string())
        })?;
        let last = arr
            .last()
            .ok_or_else(|| AppError::Provider("Qwen Code: empty result array".to_string()))?;
        let is_error = last
            .get("is_error")
            .and_then(|b| b.as_bool())
            .unwrap_or(false);
        if is_error {
            let msg = last
                .get("result")
                .and_then(|r| r.as_str())
                .unwrap_or("Qwen Code reported an error");
            return Err(AppError::Provider(format!("Qwen Code: {msg}")));
        }
        last.get("result")
            .and_then(|r| r.as_str())
            .map(String::from)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::Provider("Qwen Code: empty result".to_string()))
    }

    fn native_json_schema_invocation(
        &self,
        model: &str,
        _system: &str,
        _effort: Option<&str>,
        schema: &Value,
    ) -> Option<CliInvocation> {
        if !schema_is_under_argv_cap(schema) {
            return None;
        }
        Some(CliInvocation {
            args: build_args(model, false, Some(&compact_json_schema(schema))),
            prompt: PromptDelivery::Stdin,
        })
    }

    fn parse_structured_complete(&self, stdout: &str) -> AppResult<String> {
        // Structured output uses the same JSON array format; final element has `structured_output`.
        let v: Value = serde_json::from_str(stdout.trim())
            .map_err(|e| AppError::Provider(format!("Qwen Code: invalid JSON output: {e}")))?;
        let arr = v.as_array().ok_or_else(|| {
            AppError::Provider("Qwen Code: expected JSON array output".to_string())
        })?;
        let last = arr
            .last()
            .ok_or_else(|| AppError::Provider("Qwen Code: empty result array".to_string()))?;
        let is_error = last
            .get("is_error")
            .and_then(|b| b.as_bool())
            .unwrap_or(false);
        if is_error {
            let msg = last
                .get("result")
                .and_then(|r| r.as_str())
                .unwrap_or("Qwen Code reported an error");
            return Err(AppError::Provider(format!("Qwen Code: {msg}")));
        }
        // Prefer `structured_output` when present and non-null.
        match last.get("structured_output") {
            Some(Value::Null) | None => last
                .get("result")
                .and_then(|r| r.as_str())
                .map(String::from)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::Provider("Qwen Code: empty result".to_string())),
            Some(value) => serde_json::to_string(value).map_err(|e| {
                AppError::Provider(format!("Qwen Code: invalid structured output: {e}"))
            }),
        }
    }
}

/// Build args for qwen: `--approval-mode plan --max-session-turns 1
/// --allowed-mcp-server-names __ajh_none__ [--json-schema <schema>] -m <model>`
/// with prompt on stdin.
/// Tools are refused by the system settings file (see the module doc);
/// `--approval-mode plan` is only a last layer.
fn build_args(model: &str, streaming: bool, json_schema: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--approval-mode".to_string(),
        "plan".to_string(), // read-only floor
        "--max-session-turns".to_string(),
        "1".to_string(), // bound run
        "--allowed-mcp-server-names".to_string(),
        "__ajh_none__".to_string(), // no MCP (same sentinel as Gemini CLI)
    ];
    if streaming {
        args.push("--output-format".to_string());
        args.push("stream-json".to_string());
    } else {
        args.push("--output-format".to_string());
        args.push("json".to_string());
    }
    if let Some(schema) = json_schema {
        args.push("--json-schema".to_string());
        args.push(schema.to_string());
    }
    if let Some(model) = super::arg_token(model) {
        args.push("-m".to_string());
        args.push(model.to_string());
    }
    args
}

/// Compact (no-whitespace) serialization of `schema` for `--json-schema` argv transport.
fn compact_json_schema(schema: &Value) -> String {
    schema.to_string()
}

/// Whether `schema`'s compact form fits the `--json-schema` argv cap, measured in UTF-16 code units.
fn schema_is_under_argv_cap(schema: &Value) -> bool {
    compact_json_schema(schema).encode_utf16().count() <= MAX_JSON_SCHEMA_ARG_CHARS
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_assistant_delta() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"}]}}"#;
        assert_eq!(
            QwenCodeAgent.parse_stream_line(line),
            Some(CliEvent::Delta("Hello".to_string()))
        );
    }

    #[test]
    fn ignores_non_text_content() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_call"}]}}"#;
        assert_eq!(QwenCodeAgent.parse_stream_line(line), None);
    }

    #[test]
    fn result_success_is_done() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"final"}"#;
        assert_eq!(QwenCodeAgent.parse_stream_line(line), Some(CliEvent::Done));
    }

    #[test]
    fn result_error_is_error() {
        let line = r#"{"type":"result","subtype":"error","is_error":true,"result":"boom"}"#;
        assert_eq!(
            QwenCodeAgent.parse_stream_line(line),
            Some(CliEvent::Error("boom".to_string()))
        );
    }

    #[test]
    fn complete_extracts_from_final_array_element() {
        let out = r#"[{"type":"assistant","message":{"content":[{"type":"text","text":"first"}]}},{"type":"result","subtype":"success","is_error":false,"result":"final answer"}]"#;
        assert_eq!(QwenCodeAgent.parse_complete(out).unwrap(), "final answer");
    }

    #[test]
    fn complete_errors_on_is_error() {
        let out = r#"[{"type":"result","is_error":true,"result":"nope"}]"#;
        assert!(QwenCodeAgent.parse_complete(out).is_err());
    }

    #[test]
    fn complete_errors_on_empty_array() {
        let out = "[]";
        assert!(QwenCodeAgent.parse_complete(out).is_err());
    }

    #[test]
    fn argv_stream_has_isolation_flags_and_prompt_on_stdin() {
        let inv = QwenCodeAgent.stream_invocation("qwen-max", "system text", None);
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
        assert!(inv.args.iter().any(|a| a == "--approval-mode"));
        assert!(inv.args.iter().any(|a| a == "plan"));
        assert!(inv.args.iter().any(|a| a == "--max-session-turns"));
        assert!(inv.args.iter().any(|a| a == "1"));
        assert!(inv.args.iter().any(|a| a == "--allowed-mcp-server-names"));
        assert!(inv.args.iter().any(|a| a == "__ajh_none__"));
        assert!(inv.args.iter().any(|a| a == "--output-format"));
        assert!(inv.args.iter().any(|a| a == "stream-json"));
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "-m" && w[1] == "qwen-max"));
        // -e none removed (it only disables extensions, not tools)
        assert!(!inv.args.iter().any(|a| a == "-e"));
        // Never --yolo
        assert!(!inv.args.iter().any(|a| a == "--yolo"));
    }

    #[test]
    fn argv_complete_has_isolation_flags_and_prompt_on_stdin() {
        let inv = QwenCodeAgent.complete_invocation("qwen-max", "system text", None);
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
        assert!(inv.args.iter().any(|a| a == "--approval-mode"));
        assert!(inv.args.iter().any(|a| a == "plan"));
        assert!(inv.args.iter().any(|a| a == "--output-format"));
        assert!(inv.args.iter().any(|a| a == "json"));
        assert!(!inv.args.iter().any(|a| a == "--yolo"));
    }

    #[test]
    fn native_json_schema_invocation_under_cap() {
        let schema = json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
        });
        let inv = QwenCodeAgent
            .native_json_schema_invocation("qwen-max", "be brief", None, &schema)
            .expect("small schema must take native path");
        assert!(inv.args.windows(2).any(|w| w[0] == "--json-schema"
            && w[1] == r#"{"properties":{"name":{"type":"string"}},"type":"object"}"#));
    }

    #[test]
    fn native_json_schema_invocation_over_cap_is_none() {
        let big = json!({
            "type": "object",
            "properties": { "blob": { "type": "string", "description": "x".repeat(MAX_JSON_SCHEMA_ARG_CHARS * 3) } },
        });
        assert!(
            QwenCodeAgent
                .native_json_schema_invocation("qwen-max", "be brief", None, &big)
                .is_none(),
            "over-cap schema must decline native path"
        );
    }

    #[test]
    fn system_settings_deny_every_tool_and_are_wired_through_the_env_var() {
        let files = QwenCodeAgent.workspace_files();
        assert_eq!(files.len(), 1);
        assert_eq!(
            QwenCodeAgent.workspace_env(),
            vec![("QWEN_CODE_SYSTEM_SETTINGS_PATH", files[0].0)]
        );
        let config: Value = serde_json::from_str(&files[0].1).unwrap();
        // Written out by hand on purpose: a check driven off QWEN_TOOLS itself
        // would still pass after a tool was deleted from it.
        for tool in [
            "run_shell_command",
            "read_file",
            "read_many_files",
            "write_file",
            "edit",
            "glob",
            "web_fetch",
            "web_search",
            "google_web_search",
            "save_memory",
            "tool_search",
            "tool_call",
        ] {
            for key in [&config["permissions"]["deny"], &config["tools"]["exclude"]] {
                assert!(
                    key.as_array().unwrap().iter().any(|v| v == tool),
                    "{tool} missing from {key}"
                );
            }
        }
    }

    #[test]
    fn parse_structured_complete_prefers_structured_output() {
        let out = r#"[{"type":"result","is_error":false,"result":"Here you go: {\"name\":\"Miso\"}","structured_output":{"name":"Miso"}}]"#;
        assert_eq!(
            QwenCodeAgent.parse_structured_complete(out).unwrap(),
            r#"{"name":"Miso"}"#
        );
    }

    #[test]
    fn parse_structured_complete_falls_back_to_result() {
        let out = r#"[{"type":"result","is_error":false,"result":"{\"name\":\"Miso\"}"}]"#;
        assert_eq!(
            QwenCodeAgent.parse_structured_complete(out).unwrap(),
            r#"{"name":"Miso"}"#
        );
        let null_out = r#"[{"type":"result","is_error":false,"result":"{\"name\":\"Miso\"}","structured_output":null}]"#;
        assert_eq!(
            QwenCodeAgent.parse_structured_complete(null_out).unwrap(),
            r#"{"name":"Miso"}"#
        );
    }
}
