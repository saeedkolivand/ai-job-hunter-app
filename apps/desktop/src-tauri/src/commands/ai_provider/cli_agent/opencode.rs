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
use std::path::Path;
use std::process::Stdio;

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
    ///
    /// stdout is captured through a **file** in the agent's private workspace, not a
    /// pipe: opencode v2's Windows build writes this command's stdout asynchronously
    /// and the process exits before it flushes — measured on 2.0.16, this command's
    /// stdout arrives as 0 lines through a pipe vs. ~400 through a file. A file has
    /// no such race, so it is the one code path on every OS.
    ///
    /// One retry (no sleep loop) if the first attempt exits successfully but the
    /// file reads back blank: seen live on a cold `opencode.exe` start — 0 models on
    /// the very first invocation this session, ~400 on every one after — so without
    /// this a new user's very first picker load is exactly the bug this fixes.
    async fn discover_models(&self) -> Option<Vec<Value>> {
        let binary = self.binary();
        let args = vec!["models".to_string()];
        let provider_id = self.id().as_str();
        retry_once_on_none(|| run_models_once(&binary, &args, provider_id)).await
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

/// Read the model list a completed `opencode models` run wrote to `path`, then
/// parse it. Split out from [`OpencodeAgent::discover_models`] so the file read
/// is unit-testable without spawning the real CLI.
fn read_models_file(path: &Path) -> Option<Vec<Value>> {
    parse_models_output(&std::fs::read_to_string(path).ok()?)
}

/// One `opencode models` attempt: a fresh private workspace (see
/// [`OpencodeAgent::discover_models`]), stdout captured to a file there, bounded by
/// [`super::super::timeouts::LIST_MODELS_TOTAL`], then parsed. `None` covers a
/// failed spawn, a non-zero exit, and a blank/unparsable file alike — the caller
/// can't (and needn't) tell them apart.
async fn run_models_once(binary: &str, args: &[String], provider_id: &str) -> Option<Vec<Value>> {
    let workspace =
        super::workspace::prepare_workspace(&crate::platform::config::data_dir(), provider_id, &[])
            .ok()?;
    let out_path = workspace.path().join("models.out");
    let out_file = std::fs::File::create(&out_path).ok()?;

    let mut cmd = super::cli_command(binary, args);
    cmd.stdout(out_file).stderr(Stdio::null());

    let status = tokio::time::timeout(super::super::timeouts::LIST_MODELS_TOTAL, cmd.status())
        .await
        .ok()?
        .ok()?;
    if !status.success() {
        return None;
    }
    read_models_file(&out_path)
}

/// Call `attempt` once; if it returns `None`, call it exactly one more time (no
/// sleep loop) and return that result. Pulled out of [`OpencodeAgent::discover_models`]
/// so the retry decision is unit-testable with a fake `attempt` — no real CLI spawn.
async fn retry_once_on_none<T, Fut: std::future::Future<Output = Option<T>>>(
    mut attempt: impl FnMut() -> Fut,
) -> Option<T> {
    match attempt().await {
        Some(v) => Some(v),
        None => attempt().await,
    }
}

#[cfg(test)]
mod tests;
