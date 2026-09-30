//! Interpreting `codex exec --json` stdout: legacy (`{"msg":{...}}`) and
//! current (dotted top-level `type`) event dialects, both streaming
//! (`parse_stream_line`) and one-shot (`parse_complete`).

use super::super::*;

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
    let line =
        r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"pong"}}"#;
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

/// PR #1187 review: a `content`/`summary` array element can also be an
/// object shaped `{"type":"reasoning_text","text":"…"}` (the CLI's own
/// `ResponseItem` wire shape), not just a bare string. `reasoning_text` must
/// pull `text` out of it rather than silently dropping the entry (which
/// would drop the whole Thinking event when it's the only element).
#[test]
fn dotted_item_completed_reasoning_content_object_entry_becomes_thinking() {
    let line = r#"{"type":"item.completed","item":{"id":"item_0","type":"reasoning","content":[{"type":"reasoning_text","text":"weighing options"}]}}"#;
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
