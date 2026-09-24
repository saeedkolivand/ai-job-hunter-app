//! Claude Code backend — Anthropic's `claude` CLI run headless.
//!
//! Streaming uses `--output-format stream-json --include-partial-messages`, which
//! emits token-level `content_block_delta` events (mirroring the Anthropic
//! Messages API), terminated by a `result` event. One-shot `complete` uses
//! `--output-format json` and reads the top-level `.result`. A structured call
//! (`complete_structured` over [`super::CliAgentClient`]) adds
//! `--json-schema '<compact schema>'` and reads the schema-validated
//! `structured_output` field of the same result envelope, falling back to
//! `.result`. Three modifications apply to EVERY invocation: the isolation flags
//! ([`isolation_args`]), the optional `--effort` level ([`push_effort`]), and the
//! tool disables. Mutating/exec tools are disabled and the harness runs in a
//! neutral cwd, so this is text-only.

use async_trait::async_trait;
use serde_json::Value;

use crate::commands::ai_provider::ProviderId;
use crate::error::{AppError, AppResult};

use super::{CliAgentBackend, CliEvent, CliInvocation, PromptDelivery};

/// Tools the agent may not use — we only want text, never filesystem/network/exec
/// side effects. Read-only tools are additionally constrained by the temp cwd.
const DISALLOWED_TOOLS: &str = "Bash,Edit,Write,MultiEdit,NotebookEdit,WebFetch,WebSearch,Task";

const MODELS: &[&str] = &["sonnet", "opus", "haiku", "fable"];

/// The reasoning-effort levels Claude Code's `--effort` flag accepts, LOWEST
/// first — the order [`CliAgentClient::effort_levels`](super::CliAgentClient)
/// exposes them in, which the renderer's effort picker and the
/// `every_providers_effort_levels_list_its_lowest_tier_first` pin rely on.
/// This list IS the allowlist: [`push_effort`] forwards a value only if it is
/// one of these, so an unknown string from user settings can never reach argv.
pub(super) const EFFORT_LEVELS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

/// Serialized-length cap (in UTF-16 code units — what Windows actually counts
/// in its 32767-character command-line limit) for the JSON schema passed via
/// `--json-schema` on argv. The schema comes from OUR code
/// (`EvidenceMap::schema()`-style `json!` literals), never user input, but it
/// still rides the command line — through the `cmd.exe /C` wrapper on Windows —
/// so it gets a bound of its own; the rest of the flags are short. Above the
/// cap, [`super::CliAgentClient::complete_structured`] falls back to the
/// prompt-discipline path instead of failing.
pub(super) const MAX_JSON_SCHEMA_ARG_CHARS: usize = 16_384;

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

    fn install_package(&self) -> Option<&'static str> {
        Some("@anthropic-ai/claude-code")
    }

    fn docs_url(&self) -> &'static str {
        "https://code.claude.com/docs/en/setup"
    }

    fn stream_invocation(&self, model: &str, system: &str, effort: Option<&str>) -> CliInvocation {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--include-partial-messages".to_string(),
            "--disallowedTools".to_string(),
            DISALLOWED_TOOLS.to_string(),
        ];
        args.extend(isolation_args());
        push_model_system(&mut args, model, system);
        push_effort(&mut args, effort);
        CliInvocation {
            args,
            prompt: PromptDelivery::Stdin,
        }
    }

    fn complete_invocation(
        &self,
        model: &str,
        system: &str,
        effort: Option<&str>,
    ) -> CliInvocation {
        CliInvocation {
            args: json_invocation(model, system, effort, None),
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
        result_text(&parse_envelope(stdout)?)
    }

    fn native_json_schema_invocation(
        &self,
        model: &str,
        system: &str,
        effort: Option<&str>,
        schema: &Value,
    ) -> Option<CliInvocation> {
        // The schema rides `--json-schema` on argv, so it must fit the argv cap
        // ([`MAX_JSON_SCHEMA_ARG_CHARS`]). Above it we decline the native path
        // and [`super::CliAgentClient::complete_structured`] falls back to the
        // prompt-discipline path instead of failing (see the trait doc).
        if !schema_is_under_argv_cap(schema) {
            return None;
        }
        Some(structured_invocation(
            model,
            system,
            effort,
            Some(&compact_json_schema(schema)),
        ))
    }

    fn parse_structured_complete(&self, stdout: &str) -> AppResult<String> {
        parse_structured_stdout(stdout)
    }
}

/// Isolate the app's generation call from the user's OWN Claude Code
/// configuration. Without these, every call loads the user's settings, hooks,
/// plugins and MCP servers into the app's prompts — verified to leak user
/// configuration into generation and to roughly double startup cost.
///
/// `--setting-sources=` uses the single-token form (NOT `--setting-sources`
/// with a separate empty-string value): on Windows `claude` is an npm `.cmd`
/// shim launched through `cmd.exe /C`, whose quote handling can drop or reshape
/// a bare `""` argv entry; the `=` form cannot be reshaped by cmd.exe, and the
/// CLI parses it identically.
///
/// Explicitly NOT `--bare`: it forces `ANTHROPIC_API_KEY` auth and breaks
/// subscription logins — do not "simplify" to it.
fn isolation_args() -> Vec<String> {
    vec![
        "--strict-mcp-config".to_string(),
        "--setting-sources=".to_string(),
        "--disable-slash-commands".to_string(),
    ]
}

/// The one-shot `-p --output-format json` headless args, shared by
/// `complete_invocation` and [`structured_invocation`] (which differ only in
/// whether a `--json-schema` rides along).
fn json_invocation(
    model: &str,
    system: &str,
    effort: Option<&str>,
    json_schema: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        "--output-format".to_string(),
        "json".to_string(),
        "--disallowedTools".to_string(),
        DISALLOWED_TOOLS.to_string(),
    ];
    args.extend(isolation_args());
    push_model_system(&mut args, model, system);
    push_effort(&mut args, effort);
    if let Some(schema) = json_schema {
        args.push("--json-schema".to_string());
        args.push(schema.to_string());
    }
    args
}

/// The structured-call invocation (`--json-schema <compact schema>`), called by
/// [`super::CliAgentClient::complete_structured`] with the already-compacted
/// schema when it is present and under [`MAX_JSON_SCHEMA_ARG_CHARS`].
pub(super) fn structured_invocation(
    model: &str,
    system: &str,
    effort: Option<&str>,
    json_schema: Option<&str>,
) -> CliInvocation {
    CliInvocation {
        args: json_invocation(model, system, effort, json_schema),
        prompt: PromptDelivery::Stdin,
    }
}

/// Push `--effort <level>` when `effort` is one of [`EFFORT_LEVELS`] — the list
/// IS the guard, so anything else (an unknown string, shell metacharacters, a
/// blank) is dropped and the CLI falls back to its default effort. Never forward
/// an unknown string to argv: the allowlist is the CVE-2024-24576 defense here
/// (only fixed, trusted tokens can reach `cmd.exe` on Windows).
fn push_effort(args: &mut Vec<String>, effort: Option<&str>) {
    if let Some(level) = effort.and_then(|e| EFFORT_LEVELS.iter().find(|l| **l == e.trim())) {
        args.push("--effort".to_string());
        args.push((*level).to_string());
    }
}

/// Compact (no-whitespace) serialization of `schema` for `--json-schema` argv
/// transport. The schema is our own code (`EvidenceMap::schema()`-style
/// `json!` literals — see [`super::CliAgentClient::complete_structured`]), so
/// this serialization carries nothing untrusted; the length cap
/// ([`MAX_JSON_SCHEMA_ARG_CHARS`], via [`schema_is_under_argv_cap`]) is the
/// remaining argv bound.
pub(super) fn compact_json_schema(schema: &Value) -> String {
    schema.to_string()
}

/// Whether `schema`'s compact form fits the `--json-schema` argv cap, measured
/// in UTF-16 code units (windows' 32767-character command-line limit counts
/// those, not bytes or chars). The over-cap path in
/// [`super::CliAgentClient::complete_structured`] falls back to the
/// prompt-discipline path.
pub(super) fn schema_is_under_argv_cap(schema: &Value) -> bool {
    compact_json_schema(schema).encode_utf16().count() <= MAX_JSON_SCHEMA_ARG_CHARS
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

/// Parse the headless JSON result envelope
/// (`{"type":"result","is_error":false,…}`) into its `Value`; non-zero
/// `is_error` (or invalid JSON) surfaces exactly like `parse_complete` did
/// before the structured path split the helpers out.
fn parse_envelope(stdout: &str) -> AppResult<Value> {
    let v: Value = serde_json::from_str(stdout.trim())
        .map_err(|e| AppError::Provider(format!("Claude Code: invalid JSON output: {e}")))?;
    if v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false) {
        let msg = v
            .get("result")
            .and_then(|r| r.as_str())
            .unwrap_or("Claude Code reported an error");
        return Err(AppError::Provider(format!("Claude Code: {msg}")));
    }
    Ok(v)
}

/// The assistant text carried by `.result` — the answer every envelope carries
/// (on the structured path `.result` holds the same data as JSON text, which is
/// why [`parse_structured_stdout`] prefers `structured_output` when present).
fn result_text(v: &Value) -> AppResult<String> {
    v.get("result")
        .and_then(|r| r.as_str())
        .map(String::from)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Provider("Claude Code: empty result".to_string()))
}

/// Parse a `--json-schema` result envelope — the pure half of
/// [`CliAgentBackend::parse_structured_complete`]: the schema-validated answer
/// lives in the top-level `structured_output` field (a VALUE — the validated
/// one), so it wins over `.result` (which carries the same data as JSON text)
/// whenever it is present and non-null. It is serialized back to a compact JSON
/// string for the `complete_structured` caller. Absent/null `structured_output`
/// falls back to `.result`; `is_error: true` errors exactly like
/// [`parse_envelope`] does for `parse_complete`.
fn parse_structured_stdout(stdout: &str) -> AppResult<String> {
    let v = parse_envelope(stdout)?;
    match v.get("structured_output") {
        None | Some(Value::Null) => result_text(&v),
        Some(value) => serde_json::to_string(value).map_err(|e| {
            AppError::Provider(format!("Claude Code: invalid structured output: {e}"))
        }),
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

    // ── Spec tests 1 & 2: isolation flags + effort on BOTH invocations ──────

    /// Both invocations isolate the call from the user's own configuration:
    /// `--strict-mcp-config`, the single-token `--setting-sources=` (see
    /// [`isolation_args`] for why not the two-arg empty-string form), and
    /// `--disable-slash-commands` — and never `--bare` (it would force
    /// `ANTHROPIC_API_KEY` auth and break subscription logins).
    #[test]
    fn both_invocations_carry_isolation_flags_and_never_bare() {
        for inv in [
            ClaudeCodeAgent.stream_invocation("sonnet", "be brief", None),
            ClaudeCodeAgent.complete_invocation("sonnet", "be brief", None),
        ] {
            assert!(inv.args.iter().any(|a| a == "--strict-mcp-config"));
            assert!(inv.args.iter().any(|a| a == "--setting-sources="));
            assert!(inv.args.iter().any(|a| a == "--disable-slash-commands"));
            assert!(!inv.args.iter().any(|a| a == "--bare"));
            assert_eq!(inv.prompt, PromptDelivery::Stdin);
        }
    }

    /// `--effort <level>` is pushed for a known level and absent for `None`,
    /// an unknown string, or a blank — the allowlist drops anything not in
    /// [`EFFORT_LEVELS`], so an unknown value never reaches argv.
    #[test]
    fn effort_is_pushed_only_for_known_levels() {
        for inv in [
            ClaudeCodeAgent.stream_invocation("sonnet", "", Some("low")),
            ClaudeCodeAgent.complete_invocation("sonnet", "", Some("low")),
        ] {
            assert!(inv
                .args
                .windows(2)
                .any(|w| w[0] == "--effort" && w[1] == "low"));
        }
        for effort in [None, Some("bogus"), Some("  "), Some("high & calc")] {
            for inv in [
                ClaudeCodeAgent.stream_invocation("sonnet", "", effort),
                ClaudeCodeAgent.complete_invocation("sonnet", "", effort),
            ] {
                assert!(
                    !inv.args.iter().any(|a| a == "--effort"),
                    "effort {effort:?} must not reach argv"
                );
            }
        }
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

    // ── Spec tests 4 & 5: the structured (`--json-schema`) path ─────────────

    /// The spec fixture: the schema-validated `structured_output` wins over
    /// `.result` (which carries the same data as JSON text). Exercised through
    /// the trait override — the exact call `run_structured_complete` makes.
    #[test]
    fn parse_structured_complete_prefers_structured_output() {
        // `result` deliberately differs (prose around the JSON, as a model may
        // write it): with identical payloads a parser that ignored
        // `structured_output` and fell back to `result` would pass too.
        let out = r#"{"type":"result","is_error":false,"result":"Here you go: {\"name\":\"Miso\"}","structured_output":{"name":"Miso"},"num_turns":3}"#;
        assert_eq!(
            ClaudeCodeAgent.parse_structured_complete(out).unwrap(),
            r#"{"name":"Miso"}"#
        );
    }

    /// Absent OR null `structured_output` falls back to `.result`.
    #[test]
    fn parse_structured_complete_falls_back_to_result_when_structured_output_missing() {
        let absent = r#"{"type":"result","is_error":false,"result":"{\"name\":\"Miso\"}"}"#;
        assert_eq!(
            ClaudeCodeAgent.parse_structured_complete(absent).unwrap(),
            r#"{"name":"Miso"}"#
        );
        let null = r#"{"type":"result","is_error":false,"result":"{\"name\":\"Miso\"}","structured_output":null}"#;
        assert_eq!(
            ClaudeCodeAgent.parse_structured_complete(null).unwrap(),
            r#"{"name":"Miso"}"#
        );
    }

    /// `is_error: true` errors exactly like `parse_complete` does today.
    #[test]
    fn parse_structured_complete_errors_on_is_error() {
        let out = r#"{"type":"result","is_error":true,"result":"boom"}"#;
        let err = ClaudeCodeAgent.parse_structured_complete(out).unwrap_err();
        assert!(format!("{err}").contains("boom"));
    }

    /// `structured_invocation` carries `--json-schema` (with the compact
    /// schema) and `--effort` when given, and omits the schema flag entirely
    /// when none is given — effort/schema/isolate all ride the same base.
    #[test]
    fn structured_invocation_carries_the_schema_and_effort() {
        let schema = r#"{"type":"object","properties":{"name":{"type":"string"}}}"#;
        let inv = structured_invocation("sonnet", "be brief", Some("xhigh"), Some(schema));
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "--json-schema" && w[1] == schema));
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "--effort" && w[1] == "xhigh"));
        assert!(inv.args.iter().any(|a| a == "--setting-sources="));

        let none = structured_invocation("sonnet", "", None, None);
        assert!(!none.args.iter().any(|a| a == "--json-schema"));
        assert!(!none.args.iter().any(|a| a == "--effort"));
    }

    /// The trait opt-in (`native_json_schema_invocation`) is how the cap reaches
    /// the caller: a schema under the cap yields `Some` with `--json-schema` +
    /// the compact schema on argv; one over the cap yields `None`, which
    /// `CliAgentClient::complete_structured` turns into the prompt-discipline
    /// fallback (spec test 5: "falls back when the schema exceeds the cap").
    #[test]
    fn native_schema_invocation_is_some_under_the_cap_and_none_above_it() {
        let small = serde_json::json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
        });
        let inv = ClaudeCodeAgent
            .native_json_schema_invocation("sonnet", "be brief", Some("max"), &small)
            .expect("a small schema must take the native path");
        assert!(inv.args.windows(2).any(|w| w[0] == "--json-schema"
            && w[1] == r#"{"properties":{"name":{"type":"string"}},"type":"object"}"#));
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "--effort" && w[1] == "max"));

        let big = serde_json::json!({
            "type": "object",
            "properties": { "blob": { "type": "string", "description": "x".repeat(MAX_JSON_SCHEMA_ARG_CHARS * 3) } },
        });
        assert!(
            ClaudeCodeAgent
                .native_json_schema_invocation("sonnet", "be brief", None, &big)
                .is_none(),
            "an over-cap schema must decline the native path (prompt_only fallback)"
        );
    }

    /// The cap is the caller's fallback trigger: a schema under it is compact
    /// and fits, one over it (a pathological nesting) is detected by
    /// [`schema_is_under_argv_cap`] so the caller can degrade.
    #[test]
    fn schema_cap_detects_oversized_schemas() {
        let small = serde_json::json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
        });
        assert_eq!(
            compact_json_schema(&small),
            r#"{"properties":{"name":{"type":"string"}},"type":"object"}"#
        );
        assert!(schema_is_under_argv_cap(&small));

        // A schema of ~50k string characters (well over the cap) must be
        // detected, not silently shipped onto argv.
        let big = serde_json::json!({
            "type": "object",
            "properties": { "blob": { "type": "string", "description": "x".repeat(MAX_JSON_SCHEMA_ARG_CHARS * 3) } },
        });
        assert!(!schema_is_under_argv_cap(&big));
    }
}
