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
mod tests;
