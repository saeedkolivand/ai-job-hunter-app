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
/// string arrays (per the app-server v2 `ReasoningThreadItem` schema, which shares
/// its item shapes with `exec --json`) rather than a scalar text field — join
/// whichever is present and non-empty. Unverified against a live reasoning-bearing
/// run (issue #1185 review); the call site falls back to [`text_of`]'s scalar
/// `message`/`text`/`delta` fields if this returns `None`, so a wrong guess here
/// degrades to the same lookup every other item type uses instead of silently
/// dropping the Thinking indicator.
fn reasoning_text(item: &Value) -> Option<String> {
    ["content", "summary"].iter().find_map(|key| {
        let joined = item
            .get(*key)?
            .as_array()?
            .iter()
            .filter_map(|v| v.as_str())
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

    // ── Current dialect (`exec --json`, Codex CLI 0.144+) ─────────────────────
    // Fixtures below are real lines captured from a live `codex exec --json
    // --skip-git-repo-check "Reply with the single word pong"` run against the
    // installed CLI (0.144.6) — see issue #1185 — plus one synthetic success
    // fixture (the account had no successful run available at capture time; its
    // shape is the app-server v2 protocol's `AgentMessageThreadItem`/`Turn`
    // schema, which shares its item/turn model with `exec --json`).

    #[test]
    fn dotted_item_completed_agent_message_becomes_delta() {
        let line = r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"pong"}}"#;
        assert_eq!(
            CodexAgent.parse_stream_line(line),
            Some(CliEvent::Delta("pong".to_string()))
        );
    }

    #[test]
    fn dotted_item_completed_reasoning_joins_content_becomes_thinking() {
        let line = r#"{"type":"item.completed","item":{"id":"item_0","type":"reasoning","content":["weighing ","options"]}}"#;
        assert_eq!(
            CodexAgent.parse_stream_line(line),
            Some(CliEvent::Thinking("weighing options".to_string()))
        );
    }

    /// Hedge for issue #1185's review: if a real Codex build carries reasoning
    /// text under a scalar field (`text`/`message`/`delta`) instead of the
    /// `content`/`summary` string arrays `reasoning_text` expects, the Thinking
    /// indicator must still surface rather than silently going dark.
    #[test]
    fn dotted_item_completed_reasoning_falls_back_to_scalar_text_field() {
        let line = r#"{"type":"item.completed","item":{"id":"item_0","type":"reasoning","text":"weighing options"}}"#;
        assert_eq!(
            CodexAgent.parse_stream_line(line),
            Some(CliEvent::Thinking("weighing options".to_string()))
        );
    }

    /// `item.updated` fires repeatedly while an item is still in progress and its
    /// `item.text` is not confirmed to be an incremental chunk rather than a
    /// running snapshot — mapping it to `Delta` would risk re-concatenating the
    /// whole message-so-far into `answer` on every tick (issue #1185 review). It
    /// must be ignored; only the terminal `item.completed` carries the real text.
    #[test]
    fn dotted_item_updated_agent_message_is_ignored() {
        let line =
            r#"{"type":"item.updated","item":{"id":"item_1","type":"agent_message","text":"pon"}}"#;
        assert_eq!(CodexAgent.parse_stream_line(line), None);
    }

    /// `reasoning_text` tries `content` first, `summary` second — a real build
    /// that only populates `summary` must still surface Thinking text, not fall
    /// through to `text_of`'s scalar lookup (which would find nothing here and
    /// silently drop the item).
    #[test]
    fn dotted_item_completed_reasoning_falls_back_to_summary_array() {
        let line = r#"{"type":"item.completed","item":{"id":"item_0","type":"reasoning","summary":["short ","recap"]}}"#;
        assert_eq!(
            CodexAgent.parse_stream_line(line),
            Some(CliEvent::Thinking("short recap".to_string()))
        );
    }

    /// Tool-call / file-change / other non-chat item kinds under `item.completed`
    /// must stay invisible to the UI — only `agent_message`/`reasoning`/`error`
    /// map to an event.
    #[test]
    fn dotted_item_completed_unknown_item_type_is_ignored() {
        // `text` is present on purpose — even a matching scalar field must not leak
        // through as chat output for a kind the UI doesn't render.
        let line = r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"ls","text":"ls -la"}}"#;
        assert_eq!(CodexAgent.parse_stream_line(line), None);
    }

    #[test]
    fn dotted_turn_completed_is_done() {
        let line = r#"{"type":"turn.completed","threadId":"t1","turn":{}}"#;
        assert_eq!(CodexAgent.parse_stream_line(line), Some(CliEvent::Done));
    }

    /// Real capture: an unsupported model surfaces as an `item.completed` whose
    /// item is itself `type: "error"` (a shape the app-server v2 `ThreadItem`
    /// schema doesn't even define — `exec --json`-specific).
    #[test]
    fn dotted_item_completed_error_item_becomes_error() {
        let line = r#"{"type":"item.completed","item":{"id":"item_0","type":"error","message":"Model metadata for `gpt-5.4-mini` not found."}}"#;
        assert_eq!(
            CodexAgent.parse_stream_line(line),
            Some(CliEvent::Error(
                "Model metadata for `gpt-5.4-mini` not found.".to_string()
            ))
        );
    }

    /// Real capture: `turn.failed` nests its message under `error`.
    #[test]
    fn dotted_turn_failed_becomes_error() {
        let line = r#"{"type":"turn.failed","error":{"message":"You've hit your usage limit."}}"#;
        assert_eq!(
            CodexAgent.parse_stream_line(line),
            Some(CliEvent::Error("You've hit your usage limit.".to_string()))
        );
    }

    /// Real capture: a top-level `error` event (distinct from `turn.failed`).
    #[test]
    fn dotted_top_level_error_becomes_error() {
        let line = r#"{"type":"error","message":"You've hit your usage limit. Upgrade to Plus…"}"#;
        assert_eq!(
            CodexAgent.parse_stream_line(line),
            Some(CliEvent::Error(
                "You've hit your usage limit. Upgrade to Plus…".to_string()
            ))
        );
    }

    /// Real captures: `thread.started`/`turn.started` are recognized (dotted
    /// type) but carry nothing the UI needs — distinct from an unrecognized line.
    #[test]
    fn dotted_thread_and_turn_started_are_ignored() {
        assert_eq!(
            CodexAgent.parse_stream_line(r#"{"type":"thread.started","thread_id":"01a"}"#),
            None
        );
        assert_eq!(
            CodexAgent.parse_stream_line(r#"{"type":"turn.started"}"#),
            None
        );
    }

    #[test]
    fn dotted_parse_complete_returns_the_last_agent_message() {
        let out = "{\"type\":\"thread.started\",\"thread_id\":\"t\"}\n\
                   {\"type\":\"item.completed\",\"item\":{\"id\":\"i0\",\"type\":\"agent_message\",\"text\":\"first\"}}\n\
                   {\"type\":\"item.completed\",\"item\":{\"id\":\"i1\",\"type\":\"agent_message\",\"text\":\"final answer\"}}\n\
                   {\"type\":\"turn.completed\",\"threadId\":\"t\",\"turn\":{}}\n";
        assert_eq!(CodexAgent.parse_complete(out).unwrap(), "final answer");
    }

    /// Real-shaped repro of the issue: a run that never produces an agent
    /// message surfaces the turn-failure text, not the generic "no response".
    #[test]
    fn dotted_parse_complete_surfaces_turn_failed_when_there_is_no_agent_message() {
        let out = "{\"type\":\"thread.started\",\"thread_id\":\"t\"}\n\
                   {\"type\":\"turn.started\"}\n\
                   {\"type\":\"error\",\"message\":\"You've hit your usage limit.\"}\n\
                   {\"type\":\"turn.failed\",\"error\":{\"message\":\"You've hit your usage limit.\"}}\n";
        let err = CodexAgent.parse_complete(out).unwrap_err();
        assert!(format!("{err}").contains("usage limit"));
    }

    /// Unlike streaming (`item.updated` is ignored — see
    /// `dotted_item_updated_agent_message_is_ignored`), `parse_complete` reads the
    /// whole output back after the process exits, so an `item.updated` snapshot
    /// with no later `item.completed` for that item is the only text available and
    /// must still be captured — otherwise a turn that ends mid-item would report
    /// "no response" despite the CLI having produced text.
    #[test]
    fn dotted_parse_complete_captures_agent_message_from_item_updated_alone() {
        let out = "{\"type\":\"thread.started\",\"thread_id\":\"t\"}\n\
                   {\"type\":\"item.updated\",\"item\":{\"id\":\"i0\",\"type\":\"agent_message\",\"text\":\"partial so far\"}}\n";
        assert_eq!(CodexAgent.parse_complete(out).unwrap(), "partial so far");
    }

    /// Same non-chat item kinds ignored in streaming (see
    /// `dotted_item_completed_unknown_item_type_is_ignored`) must also leave no
    /// trace in the aggregated output — the `_ => {}` arm doesn't accidentally
    /// stringify a tool call into `last_message`.
    #[test]
    fn dotted_parse_complete_ignores_unknown_item_types() {
        let out = "{\"type\":\"item.completed\",\"item\":{\"id\":\"i0\",\"type\":\"command_execution\",\"command\":\"ls\",\"text\":\"ls -la\"}}\n";
        let err = CodexAgent.parse_complete(out).unwrap_err();
        assert!(format!("{err}").contains("no response in output"));
    }

    /// Neither dialect yields anything — the honest "no response" error, not a
    /// silent empty success.
    #[test]
    fn parse_complete_reports_no_response_when_output_has_no_message_or_error() {
        let out = "{\"type\":\"thread.started\",\"thread_id\":\"t\"}\n\
                   {\"type\":\"turn.started\"}\n";
        let err = CodexAgent.parse_complete(out).unwrap_err();
        assert!(format!("{err}").contains("no response in output"));
    }

    // ── `codex debug models` (live discovery) ──────────────────────────────────

    /// Trimmed real shape from `codex debug models` (Codex CLI 0.144.6) — full
    /// entries also carry a `base_instructions` block, omitted here.
    const DEBUG_MODELS_JSON: &str = r#"{"models":[
        {"slug":"gpt-reserve","display_name":"GPT-Reserve","visibility":"hide"},
        {"slug":"gpt-5.6-terra","display_name":"GPT-5.6-Terra","visibility":"list"},
        {"slug":"gpt-5.6-luna","display_name":"GPT-5.6-Luna","visibility":"list"},
        {"slug":"gpt-5.5","display_name":"GPT-5.5","visibility":"list"},
        {"slug":"codex-auto-review","display_name":"Codex Auto Review","visibility":"hide"}
    ]}"#;

    #[test]
    fn parse_debug_models_keeps_only_list_visibility_entries() {
        let entries = parse_debug_models(DEBUG_MODELS_JSON).unwrap();
        assert_eq!(
            entries,
            vec![
                json!({ "name": "gpt-5.6-terra", "displayName": "GPT-5.6-Terra" }),
                json!({ "name": "gpt-5.6-luna", "displayName": "GPT-5.6-Luna" }),
                json!({ "name": "gpt-5.5", "displayName": "GPT-5.5" }),
            ]
        );
    }

    #[test]
    fn parse_debug_models_none_on_malformed_json() {
        assert_eq!(parse_debug_models("not json"), None);
        assert_eq!(parse_debug_models(r#"{"nope":true}"#), None);
    }

    /// Every real entry happened to be `"hide"` (or the catalog is genuinely
    /// empty) — `None`, same as a parse failure, so the caller falls back.
    #[test]
    fn parse_debug_models_none_when_nothing_is_listable() {
        let out = r#"{"models":[{"slug":"x","display_name":"X","visibility":"hide"}]}"#;
        assert_eq!(parse_debug_models(out), None);
    }

    /// One row missing a required field (`DebugModel` has no default for
    /// `display_name`) must be skipped, not abort the whole parse — the
    /// `filter_map(...ok())` behind it silently drops just that row.
    #[test]
    fn parse_debug_models_skips_a_malformed_row_but_keeps_the_rest() {
        let out = r#"{"models":[
            {"slug":"broken","visibility":"list"},
            {"slug":"gpt-5.5","display_name":"GPT-5.5","visibility":"list"}
        ]}"#;
        assert_eq!(
            parse_debug_models(out).unwrap(),
            vec![json!({ "name": "gpt-5.5", "displayName": "GPT-5.5" })]
        );
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
