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
use serde::Deserialize;
use serde_json::{json, Value};

use crate::commands::ai_provider::{timeouts, ProviderId};
use crate::error::{AppError, AppResult};

use super::{CliAgentBackend, CliEvent, CliInvocation, PromptDelivery};

/// LAST-RESORT fallback — used only when [`discover_models`](CodexAgent::discover_models)
/// (`codex debug models`) is unavailable (CLI not installed) or its attempt
/// fails/times out/returns nothing. `CliAgentClient::list_models` labels every entry
/// from this path `source: "fallback"` so it can never be mistaken for the CLI's own
/// live catalogue (issue #1185 — the CLI's real models drift release to release).
/// The old `gpt-5-codex`/`o4-mini` pair no longer exists in the CLI's own catalogue
/// (verified live via `codex debug models` against CLI 0.144.6, 2026-09) — this list
/// is only ever shown when discovery itself has already failed, so it's a last-known
/// snapshot, not a promise; keep it updated by the same live check when it goes stale.
const MODELS: &[&str] = &["gpt-5.5", "gpt-5.6-terra", "gpt-5.6-luna"];

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

    /// Live discovery via `codex debug models` — the installed CLI's own model
    /// catalog as JSON (verified against Codex CLI 0.144.6). Spawned through the
    /// same [`super::cli_command`] path every other invocation uses (Windows
    /// `.cmd`-shim handling, augmented `PATH`), bounded by
    /// [`timeouts::LIST_MODELS_TOTAL`] so a hung CLI can't stall the picker.
    /// `None` on any failure (not installed, timeout, non-zero exit, unparseable
    /// output) — the harness then falls back to [`models`](Self::models),
    /// labelled `source: "fallback"` (see [`super::resolve_models`]).
    async fn discover_models(&self) -> Option<Vec<Value>> {
        let binary = self.binary();
        let args = vec!["debug".to_string(), "models".to_string()];
        let out = tokio::time::timeout(
            timeouts::LIST_MODELS_TOTAL,
            super::cli_command(&binary, &args).output(),
        )
        .await
        .ok()?
        .ok()?;
        if !out.status.success() {
            return None;
        }
        parse_debug_models(&String::from_utf8_lossy(&out.stdout))
    }

    fn install_package(&self) -> Option<&'static str> {
        Some("@openai/codex")
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
        // Current dialect (Codex CLI 0.144+ `exec --json`, verified live plus the
        // app-server v2 protocol's shared `ThreadItem`/`Turn` schema): a flat,
        // dotted top-level `type` (`thread.started`, `turn.started`,
        // `item.completed`, `turn.completed`, `turn.failed`, `error`), with the
        // completed/updated item's own fields nested under `item`. The legacy
        // dialect below never produces a top-level `type` at all (it's always
        // nested under `msg`, or — per `inner`'s fallback — unwrapped but never
        // dotted), so this check alone distinguishes the two.
        if let Some(ty) = v.get("type").and_then(|t| t.as_str()) {
            if ty.contains('.') || ty == "error" {
                return parse_dotted_event(ty, &v);
            }
        }
        // Legacy dialect (`{"msg":{"type":"agent_message",…}}`) — kept as a
        // fallback so an older/downgraded Codex install still works.
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
            if let Some(ty) = v.get("type").and_then(|t| t.as_str()) {
                if ty.contains('.') || ty == "error" {
                    match ty {
                        "item.completed" | "item.updated" => {
                            if let Some(item) = v.get("item") {
                                match item.get("type").and_then(|t| t.as_str()) {
                                    Some("agent_message") => {
                                        last_message = text_of(item).or(last_message)
                                    }
                                    Some("error") => error = text_of(item).or(error),
                                    _ => {}
                                }
                            }
                        }
                        "turn.failed" => error = v.get("error").and_then(text_of).or(error),
                        "error" => error = text_of(&v).or(error),
                        _ => {}
                    }
                    continue;
                }
            }
            // Legacy dialect.
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

/// Map one current-dialect (dotted-`type`) event to a [`CliEvent`] — split out from
/// [`CodexAgent::parse_stream_line`] purely so the dialect-detection guard there stays
/// readable. `ty` is already known dotted or `"error"`.
fn parse_dotted_event(ty: &str, v: &Value) -> Option<CliEvent> {
    match ty {
        // `item.updated` is deliberately IGNORED, not mapped to the same events as
        // `item.completed`: it fires repeatedly while an item is still in progress
        // and there's no confirmed evidence its `item.text` is an incremental
        // chunk rather than a running snapshot (unlike the token-level SSE deltas
        // the cloud providers emit). Treating a snapshot as a delta would
        // re-concatenate the whole message-so-far on every tick, garbling the
        // streamed output — so only the terminal `item.completed` (always the
        // full, final text) is surfaced. Message-granularity streaming (one
        // event per finished item) is exactly what the module doc promises.
        "item.completed" => {
            let item = v.get("item")?;
            match item.get("type").and_then(|t| t.as_str())? {
                "agent_message" => text_of(item).map(CliEvent::Delta),
                // Reasoning items carry `content`/`summary` string arrays, not the
                // scalar `text`/`message` field every other item type uses.
                "reasoning" => reasoning_text(item)
                    .or_else(|| text_of(item))
                    .map(CliEvent::Thinking),
                "error" => Some(CliEvent::Error(
                    text_of(item).unwrap_or_else(|| "Codex reported an error".to_string()),
                )),
                // Tool-call / file-change / other item kinds — not chat output.
                _ => None,
            }
        }
        "turn.completed" => Some(CliEvent::Done),
        "turn.failed" => Some(CliEvent::Error(
            v.get("error")
                .and_then(text_of)
                .unwrap_or_else(|| "Codex turn failed".to_string()),
        )),
        "error" => Some(CliEvent::Error(
            text_of(v).unwrap_or_else(|| "Codex reported an error".to_string()),
        )),
        // `thread.started` / `turn.started` / anything else new — recognized as
        // the current dialect, but nothing the UI needs to see.
        _ => None,
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
        // Isolation: don't load the user's global/project config or AGENTS.md rule
        // files, don't persist session state, and disable every agentic feature flag
        // that could pull in extra context or run extra machinery (hooks, plugins,
        // apps, memories, goals, multi-agent, workspace deps). `project_doc_max_bytes=0`
        // drops the auto-loaded project doc. Isolation is defense-in-depth: JD text
        // must not mix with whatever the user's own codex setup injects.
        //
        // WHY `--ignore-user-config` specifically: verified that `-c mcp_servers={}`
        // MERGES with the user's config.toml rather than clearing it, so without this
        // flag every generation call would start every MCP server in the user's
        // config.toml (on the owner's machine that includes this app's own binary in
        // `agent mcp` mode). Known trade-off: a user whose config.toml sets a custom
        // `model_provider` (Azure, OSS) loses it for app calls — acceptable, because
        // the app passes `--model` explicitly, so the model choice is unaffected.
        //
        // We use the `-c key=value` config-override form for feature flags — NOT
        // `--disable <flag>`: `--disable` hard-errors on an unknown feature name on
        // older/newer CLI builds, which would break every call (out-of-band CLI
        // drift is the exact failure mode `friendly_cli_error` maps to a friendly
        // "unexpected argument" message, but we'd rather never hit it).
        //
        // ponytail: the GLOBAL `$CODEX_HOME/AGENTS.md` memory file is still loaded
        // even with `--ignore-user-config` + `--ignore-rules` (upstream reads it
        // through a separate path those flags don't gate). The only fix is a
        // temporary `CODEX_HOME` with the auth file linked in, which means handling
        // credentials — out of scope. Upgrade path: a per-install `CODEX_HOME`
        // scaffold if this ever becomes a real leak.
        //
        // We do NOT pass `--bare`: it forces `ANTHROPIC_API_KEY`-style env auth and
        // breaks the user's ChatGPT subscription login.
        "--ignore-user-config".to_string(),
        "--ignore-rules".to_string(),
        "--ephemeral".to_string(),
        "-c".to_string(),
        "features.hooks=false".to_string(),
        "-c".to_string(),
        "features.plugins=false".to_string(),
        "-c".to_string(),
        "features.remote_plugin=false".to_string(),
        "-c".to_string(),
        "features.apps=false".to_string(),
        "-c".to_string(),
        "features.memories=false".to_string(),
        "-c".to_string(),
        "features.goals=false".to_string(),
        "-c".to_string(),
        "features.multi_agent=false".to_string(),
        "-c".to_string(),
        "features.workspace_dependencies=false".to_string(),
        "-c".to_string(),
        "project_doc_max_bytes=0".to_string(),
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

/// Reasoning items in the current dialect are believed to carry `content`/`summary`
/// arrays (per the app-server v2 `ReasoningThreadItem` schema, which shares its item
/// shapes with `exec --json`) rather than a scalar text field — join whichever is
/// present and non-empty. Each array element is either a plain string, or an object
/// shaped `{"type":"reasoning_text","text":"…"}` (confirmed against the CLI's own
/// `ResponseItem` schema — PR #1187 review); either shape yields its text. The call
/// site falls back to [`text_of`]'s scalar `message`/`text`/`delta` fields if this
/// returns `None`, so a wrong guess here degrades to the same lookup every other item
/// type uses instead of silently dropping the Thinking indicator.
fn reasoning_text(item: &Value) -> Option<String> {
    ["content", "summary"].iter().find_map(|key| {
        let joined = item
            .get(*key)?
            .as_array()?
            .iter()
            .filter_map(|v| {
                v.as_str()
                    .or_else(|| v.get("text").and_then(|t| t.as_str()))
                    .or_else(|| v.get("content").and_then(|t| t.as_str()))
            })
            .collect::<String>();
        (!joined.is_empty()).then_some(joined)
    })
}

/// One row of `codex debug models`'s raw catalog JSON (`{"models":[…]}`). Only the
/// fields the picker needs — the real catalog additionally carries a per-model
/// `base_instructions` prompt block (hundreds of KB combined), deliberately never
/// deserialized here.
#[derive(Deserialize)]
struct DebugModel {
    slug: String,
    display_name: String,
    visibility: String,
}

/// Parse `codex debug models`'s stdout into `ProviderModelInfo`-shaped entries —
/// pure, so it's covered by a fixture without spawning the real CLI (this crate has
/// no `tauri::test` mock-app harness, and hermetic tests must not assume a system
/// binary is present or absent). Only `visibility: "list"` rows are user-selectable
/// models — `"hide"` rows (`codex-auto-review`, an internal reserve model, …) are
/// Codex mechanics, never a model to hand a user prompt to. `None` on anything
/// unparseable or an empty result, so the caller falls back to the curated list
/// exactly like a failed spawn would.
fn parse_debug_models(stdout: &str) -> Option<Vec<Value>> {
    let parsed: Value = serde_json::from_str(stdout).ok()?;
    let rows = parsed.get("models")?.as_array()?;
    let entries: Vec<Value> = rows
        .iter()
        .filter_map(|m| serde_json::from_value::<DebugModel>(m.clone()).ok())
        .filter(|m| m.visibility == "list")
        .map(|m| json!({ "name": m.slug, "displayName": m.display_name }))
        .collect();
    (!entries.is_empty()).then_some(entries)
}

#[cfg(test)]
mod tests;
