//! Unit tests for `anthropic.rs`, split into this sibling file (R8 line-budget
//! split — mirrors the `extension_bridge/answer_assist.rs` +
//! `answer_assist_tests.rs` precedent of moving the test module itself out
//! rather than production code).
//!
//! Wired via `#[path = "anthropic_tests.rs"] mod tests;` in `anthropic.rs` —
//! that keeps this a CHILD module of `anthropic` in the module tree (same as
//! an inline `#[cfg(test)] mod tests { ... }` block), so `use super::*` below
//! still reaches every private item there, while this file's own filename
//! (ending `tests.rs`) excludes it from the architecture test's R8 LOC cap
//! (`tests/architecture.rs`'s `is_test` filename check) and from R3/R6's
//! non-test scans.

use super::*;
use crate::ipc_contracts::ai::AiGenerateRequestMessage;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A minimal raw-socket HTTP/1.1 server that writes each response's status
/// line + headers immediately, then sleeps `body_delay` BEFORE writing that
/// response's body. `wiremock::ResponseTemplate::set_delay` cannot build
/// this: its delay elapses entirely BEFORE any bytes (headers included)
/// reach the socket, so it only ever exercises `req.send()`'s own timeout —
/// never a `resp.text()`/`resp.json()` blocked on a body that's slow to
/// arrive AFTER `send()` already resolved, which is the actual gap a
/// cumulative deadline has to cover. Serves `bodies` in order, one per
/// accepted connection (`Connection: close`, so each page fetch opens a new
/// one) — `bodies[i].1` is that page's body-write delay.
async fn slow_body_server(bodies: Vec<(String, Duration)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        for (body, body_delay) in bodies {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            // Drain the request up to the blank line ending its headers —
            // contents don't matter, only connection ORDER distinguishes pages.
            let mut buf = [0u8; 4096];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") => break,
                    Ok(_) => continue,
                }
            }
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(headers.as_bytes()).await;
            let _ = socket.flush().await;
            if !body_delay.is_zero() {
                tokio::time::sleep(body_delay).await;
            }
            let _ = socket.write_all(body.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });
    format!("http://{addr}")
}

fn base_request(model: &str) -> AiGenerateRequest {
    AiGenerateRequest {
        model: model.to_string(),
        messages: vec![AiGenerateRequestMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
        }],
        locale: "en".to_string(),
        temperature: Some(0.8),
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        repeat_penalty: None,
        max_tokens: None,
        context_window: None,
        effort: None,
        intent: None,
    }
}

/// The real capability matrix for `model`, exactly as every trait method
/// computes it — tests must exercise the same `caps.supports_temperature`
/// gate the adapter actually uses, not a hand-rolled stand-in.
fn caps_for(model: &str) -> ModelCapabilities {
    AnthropicClient.capabilities(model)
}

/// Mirrors what `AnthropicClient::chat_stream` does: resolve this adapter's
/// own profile for `req.model` + `req.intent`, merged with the request's
/// explicit numeric overrides.
fn sampling_for(req: &AiGenerateRequest) -> SamplingProfile {
    AnthropicClient
        .sampling_profile(&req.model, resolve_intent(req))
        .resolve(req)
}

#[test]
fn sampling_profile_is_neutral_on_adaptive_frontier_and_unrecognized_models_only() {
    // Hard constraint: a Claude 4.7+/5 model receives no sampling params at
    // all, for every intent — `temperature`/`top_p` 400 outright on adaptive
    // models regardless of value. An unrecognized `claude-`-prefixed id
    // fails safe the same way (the same new-family fail-safe
    // `anthropic_supports_temperature` already applies).
    for model in ["claude-opus-5", "claude-opus-4-7", "claude-zephyr-6"] {
        for intent in [
            Intent::Deterministic,
            Intent::Prose,
            Intent::ProseGrounded,
            Intent::Default,
        ] {
            assert_eq!(
                AnthropicClient.sampling_profile(model, intent),
                SamplingProfile::default(),
                "{model} / {intent:?} must stay neutral"
            );
        }
    }
}

#[test]
fn sampling_profile_declares_real_values_for_a_legacy_model_per_intent() {
    // A model `anthropic_supports_temperature` accepts (legacy pre-thinking)
    // declares REAL values reproducing this app's pre-fix shipped numbers —
    // omitting is not a safe fallback on a provider that accepts the field
    // (see `commands/ai_provider/mod.rs`'s module doc for why).
    let model = "claude-3-5-sonnet-20241022";
    assert_eq!(
        AnthropicClient.sampling_profile(model, Intent::Deterministic),
        SamplingProfile {
            temperature: Some(DETERMINISTIC_TEMPERATURE),
            ..SamplingProfile::default()
        }
    );
    assert_eq!(
        AnthropicClient.sampling_profile(model, Intent::Prose),
        SamplingProfile {
            temperature: Some(PROSE_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            ..SamplingProfile::default()
        }
    );
    assert_eq!(
        AnthropicClient.sampling_profile(model, Intent::ProseGrounded),
        SamplingProfile {
            temperature: Some(PROSE_GROUNDED_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            ..SamplingProfile::default()
        }
    );
    // `Default` (no declared intent) resolves the same as `Deterministic`.
    assert_eq!(
        AnthropicClient.sampling_profile(model, Intent::Default),
        AnthropicClient.sampling_profile(model, Intent::Deterministic)
    );
}

#[test]
fn wire_body_carries_the_declared_deterministic_temperature_with_no_explicit_override() {
    // End-to-end: the profile's own temperature reaches the wire when the
    // request carries no explicit override — this is exactly the path that
    // was DEAD before `chat_stream` was wired through `sampling_profile`
    // (previously `build_chat_stream_body` read `req.temperature` directly,
    // defaulting to a bare 0.7 that no test ever exercised). Mutation check:
    // change `DETERMINISTIC_TEMPERATURE` or delete the `Intent::Deterministic`
    // arm in `AnthropicClient::sampling_profile` and this must fail.
    let mut req = base_request("claude-3-5-sonnet-20241022");
    req.temperature = None;
    req.intent = Some("deterministic".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["temperature"], json!(DETERMINISTIC_TEMPERATURE));
}

#[test]
fn wire_body_prose_grounded_never_sends_a_top_p_higher_register_than_declared_and_no_penalty_field_exists(
) {
    // Anthropic has NO frequency/presence penalty parameter at all — so
    // `Intent::ProseGrounded`'s distinguishing feature elsewhere (omitting
    // presence_penalty) has nothing to omit here. What DOES carry over is
    // the lower, more-traceable temperature vs. `Intent::Prose`.
    let mut req = base_request("claude-3-5-sonnet-20241022");
    req.temperature = None;
    req.intent = Some("prose_grounded".to_string());
    let grounded = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(grounded["temperature"], json!(PROSE_GROUNDED_TEMPERATURE));
    assert_eq!(grounded["top_p"], json!(PROSE_TOP_P));
    assert!(!grounded
        .as_object()
        .unwrap()
        .contains_key("presence_penalty"));

    req.intent = Some("prose".to_string());
    let prose = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(prose["temperature"], json!(PROSE_TEMPERATURE));
}

#[test]
fn chat_stream_body_serializes_top_p_when_set_on_a_non_thinking_model() {
    let mut req = base_request("claude-3-5-sonnet-20241022");
    req.top_p = Some(0.95);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["top_p"], json!(0.95));
    assert!(body.get("thinking").is_none());
}

#[test]
fn chat_stream_body_omits_top_p_when_none() {
    let req = base_request("claude-3-5-sonnet-20241022");
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("top_p").is_none());
}

#[test]
fn chat_stream_body_skips_top_p_when_extended_thinking_is_enabled() {
    // The API rejects `top_p` alongside `thinking` — must never be sent
    // together, even if the caller (an application-answer/cover-letter
    // prose call) supplied top_p. `temperature` is omitted entirely on the
    // classic-thinking path too (Anthropic forces it to 1.0 internally;
    // omitting IS that default — see `build_chat_stream_body`'s doc comment).
    let mut req = base_request("claude-opus-4-20250514");
    req.top_p = Some(0.95);
    req.max_tokens = Some(4096); // >= 2048 → thinking budget kicks in
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("thinking").is_some(), "thinking should be enabled");
    assert!(
        body.get("temperature").is_none(),
        "temperature must be omitted, not forced to 1.0, when thinking is enabled"
    );
    assert!(
        body.get("top_p").is_none(),
        "top_p must be omitted when thinking is enabled"
    );
}

#[test]
fn thinking_gate_enables_only_extended_thinking_models() {
    for m in [
        "claude-3-7-sonnet-20250219",
        "claude-3.7-sonnet",
        "claude-opus-4-20250514",
        "claude-sonnet-4-5",
        "claude-haiku-4",
    ] {
        assert!(anthropic_supports_thinking(m), "{m} should enable thinking");
    }
    // Pre-3.7 models 400 on a `thinking` block — must stay off.
    for m in [
        "claude-3-haiku-20240307",
        "claude-3-5-sonnet-20241022",
        "claude-3-opus-20240229",
        "claude-2.1",
    ] {
        assert!(
            !anthropic_supports_thinking(m),
            "{m} must not request thinking (it 400s)"
        );
    }
}

#[test]
fn thinking_gates_normalize_dot_form_version_ids() {
    // A dot-form id ("claude-opus-4.7") must normalize the same as its
    // dash-form equivalent — otherwise it misses the "opus-4-7" needle,
    // falls through to the classic "claude-opus-4" gate, and 400s (Opus
    // 4.7 is adaptive-only; it rejects the classic `thinking.enabled` shape).
    assert!(anthropic_uses_adaptive_thinking("claude-opus-4.7"));
    assert!(anthropic_uses_adaptive_thinking("claude-opus-4.8"));
    assert!(!anthropic_supports_thinking("claude-opus-4.7"));
    assert!(!anthropic_supports_thinking("claude-opus-4.8"));
}

#[test]
fn thinking_gate_excludes_the_claude_5_family() {
    // The 5 family (Opus 5, Sonnet 5, Fable 5) replaced classic
    // budget-token thinking with adaptive thinking — sending the classic
    // `thinking` block to them would 400, so the gate must stay off.
    for m in [
        "claude-opus-5",
        "claude-sonnet-5",
        "claude-fable-5",
        "claude-fable-5-20260201",
    ] {
        assert!(
            !anthropic_supports_thinking(m),
            "{m} must not request classic thinking (it 400s; uses adaptive thinking instead)"
        );
    }
}

#[test]
fn thinking_gate_excludes_opus_4_7_and_4_8_despite_matching_claude_opus_4() {
    // Opus 4.7/4.8 are adaptive-ONLY per Anthropic's per-model table
    // ("Extended thinking: No") — they must not fall into the classic
    // gate just because they match the "claude-opus-4" substring.
    for m in ["claude-opus-4-7", "claude-opus-4-8"] {
        assert!(
            !anthropic_supports_thinking(m),
            "{m} is adaptive-only; must not receive classic thinking"
        );
    }
}

#[test]
fn adaptive_gate_matches_opus_4_7_4_8_and_the_5_family() {
    for m in [
        "claude-opus-4-7",
        "claude-opus-4-8",
        "claude-opus-5",
        "claude-opus-5-20260201",
        "claude-sonnet-5",
        "claude-fable-5",
        "claude-fable-5-20260201",
        "claude-mythos-5",
    ] {
        assert!(
            anthropic_uses_adaptive_thinking(m),
            "{m} should use adaptive thinking"
        );
    }
    // Pre-5/pre-4.7 models must never be misclassified as adaptive — in
    // particular "claude-sonnet-4-5" (Sonnet 4.5) must not match on a bare
    // "-5" substring.
    for m in [
        "claude-opus-4-20250514",
        "claude-sonnet-4-5",
        "claude-3-5-sonnet-20241022",
        "claude-3-haiku-20240307",
    ] {
        assert!(
            !anthropic_uses_adaptive_thinking(m),
            "{m} must not be classified as adaptive"
        );
    }
}

#[test]
fn unrecognized_claude_family_fails_safe_to_no_temperature() {
    // A future Anthropic family not yet in either needle list (e.g. a
    // hypothetical "Zephyr" release) must default to NO temperature, not
    // a blanket "true" — sending a non-default temperature 400s on every
    // adaptive model, while omitting it is always accepted. Defaulting
    // an unrecognized "claude-"-prefixed id to the safe direction is what
    // restores the zero-code-change promise: a brand-new Claude model
    // works safely even before this adapter learns its needle.
    assert!(!anthropic_supports_temperature("claude-zephyr-6"));
    assert!(!caps_for("claude-zephyr-6").supports_temperature);

    let mut req = base_request("claude-zephyr-6");
    req.max_tokens = Some(4096);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("temperature").is_none(),
        "an unrecognized claude- family must not receive a non-default temperature"
    );
    assert!(body.get("thinking").is_none());
}

#[test]
fn unrecognized_claude_family_fails_safe_even_behind_a_vendor_prefix() {
    // A vendor-prefixed id (as seen through an OpenRouter-style gateway) must
    // classify identically to its bare form — before stripping the prefix in
    // `normalize_model_id`, "anthropic/claude-zephyr-6" failed the
    // `starts_with("claude-")` check (it starts with "anthropic/" instead),
    // silently disarming the new-family fail-safe and sending a non-default
    // temperature that would 400 if this actually were a new adaptive family.
    assert!(!anthropic_supports_temperature("anthropic/claude-zephyr-6"));
    assert!(!caps_for("anthropic/claude-zephyr-6").supports_temperature);

    let mut req = base_request("anthropic/claude-zephyr-6");
    req.max_tokens = Some(4096);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("temperature").is_none(),
        "a vendor-prefixed unrecognized claude- family must still fail safe"
    );
}

#[test]
fn version_needles_are_boundary_aware_not_raw_substring() {
    // "claude-opus-4-70" contains "opus-4-7" as a raw substring but is NOT
    // Opus 4.7 (a different, unclassified point release) — a boundary-aware
    // match must not misclassify it as the adaptive-only opus-4-7/4-8 shape.
    assert!(!anthropic_uses_adaptive_thinking("claude-opus-4-70"));
    // It's still a plain 4.x id, so it's fine (and correct) for it to keep
    // matching the broader classic "claude-opus-4" gate.
    assert!(anthropic_supports_thinking("claude-opus-4-70"));

    // Same class of bug for "opus-5": "claude-opus-50" must not be treated
    // as the Opus 5 (adaptive) family.
    assert!(!anthropic_uses_adaptive_thinking("claude-opus-50"));
}

#[test]
fn legacy_pre_thinking_models_keep_temperature_despite_matching_neither_gate() {
    // These also match neither `anthropic_supports_thinking` nor
    // `anthropic_uses_adaptive_thinking` (they predate thinking entirely)
    // — the new-family fail-safe above must NOT catch them too, or every
    // long-shipped Claude 3.x/2.x call silently loses its temperature.
    for m in [
        "claude-3-haiku-20240307",
        "claude-3-5-sonnet-20241022",
        "claude-3-opus-20240229",
        "claude-2.1",
    ] {
        assert!(
            anthropic_supports_temperature(m),
            "{m} is a known legacy model and must keep normal temperature support"
        );
    }
    let req = base_request("claude-3-5-sonnet-20241022");
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["temperature"], json!(0.8));
}

#[test]
fn chat_stream_body_sends_adaptive_thinking_with_summarized_display_for_claude_5_family() {
    // End-to-end: the request body builder must attach the adaptive
    // thinking block (opting into visible "summarized" display, which
    // defaults to "omitted"/empty otherwise) and inflate max_tokens the
    // same way the classic-thinking path does.
    for m in [
        "claude-opus-5",
        "claude-sonnet-5",
        "claude-fable-5",
        "claude-fable-5-20260201",
    ] {
        let mut req = base_request(m);
        req.max_tokens = Some(4096);
        let body = build_chat_stream_body(&req, sampling_for(&req));
        assert_eq!(
            body["thinking"],
            json!({ "type": "adaptive", "display": "summarized" }),
            "{m} must opt into summarized display"
        );
        assert_eq!(
            body["max_tokens"],
            json!(4096 + 4096 / 2),
            "{m}: thinking tokens count toward max_tokens on adaptive models too"
        );
        // Anthropic 400s on ANY non-default temperature/top_p for every
        // adaptive-thinking model — both must be entirely omitted.
        assert!(
            body.get("temperature").is_none(),
            "{m} must not send temperature"
        );
        assert!(body.get("top_p").is_none(), "{m} must not send top_p");
    }
}

#[test]
fn chat_stream_body_inflates_max_tokens_for_adaptive_models_below_the_classic_2048_gate() {
    // Regression for the extension bridge's answer-assist flow, which
    // calls with `max_tokens: 1000` (below the classic path's 2048
    // heuristic gate). Adaptive thinking is on by default regardless of
    // the caller's cap, so the inflation must NOT be gated the same way —
    // otherwise the user is billed summarized-thinking tokens out of an
    // un-inflated 1000-token budget and drafts come back short/empty.
    let mut req = base_request("claude-sonnet-5");
    req.max_tokens = Some(1000);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["thinking"],
        json!({ "type": "adaptive", "display": "summarized" })
    );
    assert_eq!(
        body["max_tokens"],
        json!(1000 + 1024),
        "a small cap must get the ~1024-token headroom floor, not a proportional \
         1000/2=500 that leaves too little room for thinking + a visible draft"
    );
}

#[test]
fn chat_stream_body_keeps_the_classic_2048_gate_for_classic_models() {
    // The classic path's inflation/`thinking` key must stay gated on
    // `max_tokens >= 2048` — only the ADAPTIVE path lost that gate.
    let mut req = base_request("claude-opus-4-20250514");
    req.max_tokens = Some(1000);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("thinking").is_none(),
        "classic thinking must stay off below the 2048 gate"
    );
    assert_eq!(
        body["max_tokens"],
        json!(1000),
        "no inflation for a classic model below the 2048 gate"
    );
}

#[test]
fn chat_stream_body_sends_adaptive_thinking_for_opus_4_7_and_4_8() {
    for m in ["claude-opus-4-7", "claude-opus-4-8"] {
        let mut req = base_request(m);
        req.max_tokens = Some(4096);
        let body = build_chat_stream_body(&req, sampling_for(&req));
        assert_eq!(
            body["thinking"],
            json!({ "type": "adaptive", "display": "summarized" }),
            "{m} is adaptive-only — must never get the classic enabled+budget shape"
        );
        assert!(body.get("temperature").is_none());
    }
}

#[test]
fn chat_stream_body_omits_top_p_for_adaptive_models_even_when_caller_supplies_it() {
    let mut req = base_request("claude-sonnet-5");
    req.top_p = Some(0.95);
    req.max_tokens = Some(4096);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("top_p").is_none(),
        "adaptive models 400 on a non-default top_p"
    );
}

#[test]
fn chat_stream_body_sends_no_thinking_key_for_unknown_models() {
    // Unknown/other models get nothing extra: no thinking block, no
    // max_tokens inflation, and the plain temperature/top_p path.
    let mut req = base_request("some-future-claude-model");
    req.max_tokens = Some(8192);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("thinking").is_none());
    assert_eq!(
        body["max_tokens"],
        json!(8192),
        "no inflation for an unknown model"
    );
    assert_eq!(body["temperature"], json!(0.8));
}

#[test]
fn build_complete_body_omits_temperature_and_inflates_max_tokens_for_fable_5() {
    let body = build_complete_body("claude-fable-5", "", "hi", Some(0.8));
    assert!(
        body.get("temperature").is_none(),
        "adaptive models 400 on a non-default temperature"
    );
    assert_eq!(
        body["max_tokens"],
        json!(4096 + 4096 / 2),
        "thinking is on by default and counts toward max_tokens even with no thinking key sent"
    );
    // No thinking-view display concern on this single-shot completion path.
    assert!(body.get("thinking").is_none());
}

#[test]
fn build_complete_body_keeps_temperature_for_a_classic_model() {
    let body = build_complete_body("claude-opus-4-20250514", "sys", "hi", Some(0.3));
    assert_eq!(body["temperature"], json!(0.3));
    assert_eq!(
        body["max_tokens"],
        json!(4096),
        "no inflation for a classic model here"
    );
    assert_eq!(body["system"], json!("sys"));
}

#[test]
fn build_web_search_body_omits_temperature_and_inflates_max_tokens_for_fable_5() {
    let body = build_web_search_body("claude-fable-5", "sys", "hi");
    assert!(
        body.get("temperature").is_none(),
        "adaptive models 400 on a non-default temperature"
    );
    assert_eq!(
        body["max_tokens"],
        json!(1024 + 1024),
        "the ~1024-token headroom floor applies even to this small hardcoded 1024 cap \
         (a proportional 1024/2=512 would be below the useful-thinking floor)"
    );
}

#[test]
fn build_web_search_body_keeps_its_hardcoded_temperature_for_a_classic_model() {
    let body = build_web_search_body("claude-opus-4-20250514", "sys", "hi");
    assert_eq!(body["temperature"], json!(0.2));
    assert_eq!(body["max_tokens"], json!(1024));
}

#[test]
fn build_tools_body_omits_temperature_and_inflates_max_tokens_for_fable_5() {
    let body = build_tools_body("claude-fable-5", "", vec![], vec![], Some(0.8));
    assert!(
        body.get("temperature").is_none(),
        "adaptive models 400 on a non-default temperature"
    );
    assert_eq!(body["max_tokens"], json!(4096 + 4096 / 2));
}

#[test]
fn build_tools_body_keeps_temperature_for_a_classic_model() {
    let body = build_tools_body("claude-opus-4-20250514", "", vec![], vec![], Some(0.4));
    assert_eq!(body["temperature"], json!(0.4));
    assert_eq!(body["max_tokens"], json!(4096));
}

#[test]
fn parse_frames_splits_thinking_and_text_deltas() {
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from(
        "event: content_block_delta\n\
         data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hmm\"}}\n\
         event: content_block_delta\n\
         data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n",
    );
    let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
    assert_eq!(
        pieces,
        vec![StreamPiece::thinking("hmm"), StreamPiece::text("hi")]
    );
}

#[test]
fn parse_frames_done_on_message_stop_event() {
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from("event: message_stop\ndata: {\"type\":\"message_stop\"}\n");
    assert_eq!(
        parse_anthropic_frames(&mut buf, &mut last, &mut usage),
        vec![StreamPiece::done("")]
    );
}

#[test]
fn parse_frames_done_when_event_line_split_across_chunks() {
    // The `event:` line arrives in one chunk, the `data:` in the next — the
    // caller carries `last_event`, so message_stop is still detected.
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from("event: message_stop\n");
    assert!(parse_anthropic_frames(&mut buf, &mut last, &mut usage).is_empty());
    assert_eq!(last, "message_stop");
    buf.push_str("data: {}\n");
    assert_eq!(
        parse_anthropic_frames(&mut buf, &mut last, &mut usage),
        vec![StreamPiece::done("")]
    );
}

#[test]
fn parse_frames_leaves_partial_trailing_line_buffered() {
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from("data: {\"type\":\"content_block_de");
    assert!(parse_anthropic_frames(&mut buf, &mut last, &mut usage).is_empty());
    assert_eq!(buf, "data: {\"type\":\"content_block_de");
}

#[test]
fn parse_frames_drains_consumed_lines_keeping_partial_tail() {
    // The in-place `drain(..consumed)` must drop exactly the fully-parsed lines
    // (incl. a multi-byte char before the newline) and keep the partial tail —
    // the offset arithmetic stays on char boundaries.
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from(
        "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"café\"}}\n\
         data: {\"type\":\"content_block_de",
    );
    let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
    assert_eq!(pieces, vec![StreamPiece::text("café")]);
    // Only the unterminated trailing line survives the drain.
    assert_eq!(buf, "data: {\"type\":\"content_block_de");
}

#[test]
fn parse_frames_combines_message_start_input_and_message_delta_output_tokens() {
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from(
        "event: message_start\n\
         data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":25,\"output_tokens\":1}}}\n",
    );
    let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
    assert_eq!(
        pieces,
        vec![StreamPiece::usage(Usage {
            input_tokens: 25,
            output_tokens: 0
        })]
    );

    buf.push_str(
        "event: message_delta\n\
         data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":91}}\n",
    );
    let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
    assert_eq!(
        pieces,
        vec![StreamPiece::usage(Usage {
            input_tokens: 25,
            output_tokens: 91
        })]
    );
}

#[test]
fn parse_usage_reads_a_non_streaming_response() {
    let data = json!({ "usage": { "input_tokens": 12, "output_tokens": 34 } });
    let usage = parse_anthropic_usage(&data);
    assert_eq!(usage.input_tokens, 12);
    assert_eq!(usage.output_tokens, 34);
}

#[test]
fn parse_usage_defaults_to_zero_when_absent() {
    let usage = parse_anthropic_usage(&json!({}));
    assert_eq!(usage, Usage::default());
}

#[test]
fn join_text_blocks_concatenates_only_text_blocks() {
    // Web-search responses interleave tool blocks among the text blocks.
    let data = json!({
        "content": [
            { "type": "text", "text": "Acme is a " },
            { "type": "server_tool_use", "name": "web_search", "input": { "query": "Acme" } },
            { "type": "web_search_tool_result", "content": [{ "url": "x", "title": "y" }] },
            { "type": "text", "text": "widget maker." }
        ]
    });
    assert_eq!(join_text_blocks(&data), "Acme is a widget maker.");
}

#[test]
fn join_text_blocks_empty_on_missing_or_error() {
    assert_eq!(join_text_blocks(&json!({})), "");
    assert_eq!(join_text_blocks(&json!({ "content": [] })), "");
}

#[test]
fn parse_turn_extracts_text_and_tool_use_blocks() {
    // Assistant text interleaved with a `tool_use` block; stop_reason=tool_use.
    let data = json!({
        "content": [
            { "type": "text", "text": "Let me look that up." },
            { "type": "tool_use", "id": "toolu_1", "name": "research_company",
              "input": { "company": "Acme", "jobAd": "..." } }
        ],
        "stop_reason": "tool_use"
    });
    let turn = parse_anthropic_turn(&data);
    assert_eq!(turn.text, "Let me look that up.");
    assert_eq!(turn.stop, StopReason::ToolUse);
    assert_eq!(
        turn.tool_calls,
        vec![ToolCall {
            id: "toolu_1".to_string(),
            name: "research_company".to_string(),
            args: json!({ "company": "Acme", "jobAd": "..." }),
        }]
    );
}

#[test]
fn parse_turn_no_tool_calls_is_a_plain_end_turn() {
    let data = json!({
        "content": [{ "type": "text", "text": "All done." }],
        "stop_reason": "end_turn"
    });
    let turn = parse_anthropic_turn(&data);
    assert_eq!(turn.text, "All done.");
    assert!(turn.tool_calls.is_empty());
    assert_eq!(turn.stop, StopReason::End);
}

#[test]
fn parse_turn_maps_max_tokens_and_missing_input() {
    // `max_tokens` → Length; a tool_use with no `input` still parses (args = {}).
    let data = json!({
        "content": [{ "type": "tool_use", "id": "t", "name": "match_resume" }],
        "stop_reason": "max_tokens"
    });
    let turn = parse_anthropic_turn(&data);
    assert_eq!(turn.stop, StopReason::Length);
    assert_eq!(turn.tool_calls.len(), 1);
    assert_eq!(turn.tool_calls[0].args, json!({}));
}

#[test]
fn effort_gate_matches_the_documented_model_list() {
    for m in [
        "claude-opus-4-5",
        "claude-opus-4.5",
        "claude-opus-4-6",
        "claude-opus-4-7",
        "claude-opus-4-8",
        "claude-sonnet-4-6",
        "claude-sonnet-5",
        "claude-opus-5",
        "claude-fable-5",
        "claude-mythos-5",
        "claude-mythos-preview",
    ] {
        assert!(anthropic_supports_effort(m), "{m} should support effort");
    }
    // Models NOT in Anthropic's documented effort list — including thinking-
    // capable ones (classic 3.7/4.x, and Sonnet 4.5 which is adjacent to but
    // distinct from the documented Sonnet 4.6) — must stay off.
    for m in [
        "claude-3-7-sonnet-20250219",
        "claude-sonnet-4",
        "claude-sonnet-4-5",
        "claude-haiku-4-5",
        "claude-3-5-sonnet-20241022",
    ] {
        assert!(!anthropic_supports_effort(m), "{m} must not support effort");
    }
}

#[test]
fn effort_gate_needles_are_boundary_aware_not_raw_substring() {
    // The classic prefix-collision trap this file already guards elsewhere
    // (`version_needles_are_boundary_aware_not_raw_substring`): a longer
    // version number must not falsely match a shorter documented needle.
    assert!(!anthropic_supports_effort("claude-sonnet-50"));
    assert!(!anthropic_supports_effort("claude-opus-4-50"));
    // A bare "mythos" with no recognized suffix, or an unlisted future
    // Mythos version, must not guess `true` — only the two currently
    // documented names ("Mythos 5", "Mythos Preview") match.
    assert!(!anthropic_supports_effort("claude-mythos"));
    assert!(!anthropic_supports_effort("claude-mythos-6"));
}

#[test]
fn effort_levels_mirror_the_effort_gate() {
    assert_eq!(
        AnthropicClient.effort_levels("claude-opus-5"),
        vec!["low", "medium", "high", "max", "xhigh"]
    );
    assert!(AnthropicClient
        .effort_levels("claude-3-5-sonnet-20241022")
        .is_empty());
}

#[test]
fn effort_levels_are_looked_up_per_model_tier_not_binary() {
    // Full 5-level tier.
    for m in [
        "claude-fable-5",
        "claude-mythos-5",
        "claude-opus-5",
        "claude-opus-4-8",
        "claude-opus-4-7",
        "claude-sonnet-5",
    ] {
        assert_eq!(
            anthropic_effort_levels(m),
            vec!["low", "medium", "high", "max", "xhigh"],
            "{m} should support the full 5-level set"
        );
    }
    // `max` but no `xhigh`.
    for m in [
        "claude-mythos-preview",
        "claude-opus-4-6",
        "claude-sonnet-4-6",
    ] {
        assert_eq!(
            anthropic_effort_levels(m),
            vec!["low", "medium", "high", "max"],
            "{m} should support max but not xhigh"
        );
    }
    // Neither `max` nor `xhigh` — the one extended-thinking-only exception.
    assert_eq!(
        anthropic_effort_levels("claude-opus-4-5"),
        vec!["low", "medium", "high"]
    );
    // Not an effort-capable model at all -> no levels.
    assert!(anthropic_effort_levels("claude-3-5-sonnet-20241022").is_empty());
}

#[test]
fn capabilities_supports_reasoning_mirrors_the_effort_gate() {
    assert_eq!(
        caps_for("claude-opus-5").supports_reasoning,
        anthropic_supports_effort("claude-opus-5")
    );
    assert_eq!(
        caps_for("claude-3-5-sonnet-20241022").supports_reasoning,
        anthropic_supports_effort("claude-3-5-sonnet-20241022")
    );
}

#[test]
fn chat_stream_body_sends_output_config_effort_for_an_effort_capable_model() {
    let mut req = base_request("claude-opus-5");
    req.effort = Some("low".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["output_config"], json!({ "effort": "low" }));
}

#[test]
fn chat_stream_body_omits_output_config_for_a_non_effort_capable_model() {
    let mut req = base_request("claude-3-5-sonnet-20241022");
    req.effort = Some("low".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("output_config").is_none());
}

#[test]
fn chat_stream_body_omits_output_config_when_effort_not_set() {
    let req = base_request("claude-opus-5");
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("output_config").is_none());
}

#[test]
fn chat_stream_body_omits_output_config_effort_invalid_for_the_current_model_tier() {
    // The reported model-switch scenario: `effort` is stored PER PROVIDER
    // (`preferences-store.ts`), not per model. "xhigh" is valid on Sonnet 5
    // but NOT on Sonnet 4.6 (max but no xhigh) — both are effort-capable, so
    // gating on `anthropic_supports_effort` alone would ship an invalid
    // level and 400. Must omit `output_config` entirely rather than send a
    // level the CURRENT model rejects.
    let mut req = base_request("claude-sonnet-4-6");
    req.effort = Some("xhigh".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("output_config").is_none(),
        "xhigh is invalid for claude-sonnet-4-6 (max but no xhigh) — must not be sent"
    );
}

#[test]
fn chat_stream_body_omits_output_config_effort_invalid_for_opus_4_5() {
    // Opus 4.5 is the one effort-capable model with NEITHER max nor xhigh.
    let mut req = base_request("claude-opus-4-5");
    req.effort = Some("max".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("output_config").is_none(),
        "max is invalid for claude-opus-4-5 (low/medium/high only) — must not be sent"
    );
}

#[test]
fn chat_stream_body_sends_xhigh_only_on_a_model_that_supports_it() {
    let mut req = base_request("claude-sonnet-5");
    req.effort = Some("xhigh".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["output_config"], json!({ "effort": "xhigh" }));
}

// ── list_models ──────────────────────────────────────────────────────────────

#[test]
fn require_anthropic_key_errors_on_missing_or_blank_key() {
    assert!(matches!(
        require_anthropic_key(None),
        Err(AppError::Config(_))
    ));
    assert!(matches!(
        require_anthropic_key(Some("   ".to_string())),
        Err(AppError::Config(_))
    ));
}

#[test]
fn require_anthropic_key_accepts_a_real_key() {
    assert_eq!(
        require_anthropic_key(Some("sk-ant-real".to_string())).unwrap(),
        "sk-ant-real"
    );
}

#[test]
fn require_anthropic_key_trims_the_returned_key_not_just_the_checked_one() {
    // A pasted key with a trailing space/newline must reach the `x-api-key`
    // header TRIMMED — checking `k.trim().is_empty()` but returning the
    // padded `k` is the exact bug: a trailing space just 401s, an embedded
    // `\n` makes the header value invalid and the request never builds at all.
    assert_eq!(
        require_anthropic_key(Some(" sk-ant-real \n".to_string())).unwrap(),
        "sk-ant-real"
    );
}

#[test]
fn parse_model_page_keeps_only_claude_ids() {
    let body = json!({
        "data": [
            { "id": "claude-sonnet-5" },
            { "id": "claude-opus-4-5" },
            { "id": "some-other-model" },
        ]
    });
    let (page, cursor) = parse_model_page(&body).unwrap();
    let names: Vec<String> = page
        .into_iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["claude-sonnet-5", "claude-opus-4-5"]);
    assert_eq!(cursor, None);
}

#[test]
fn parse_model_page_populates_display_name_created_at_and_context_length_when_present() {
    let body = json!({
        "data": [{
            "type": "model",
            "id": "claude-sonnet-5-20251101",
            "display_name": "Claude Sonnet 5",
            "created_at": "2024-01-01T00:00:00Z",
            "max_input_tokens": 200_000,
        }]
    });
    let (page, _) = parse_model_page(&body).unwrap();
    assert_eq!(
        page,
        vec![json!({
            "name": "claude-sonnet-5-20251101",
            "displayName": "Claude Sonnet 5",
            "createdAt": 1_704_067_200_000i64,
            "contextLength": 200_000,
        })]
    );
}

#[test]
fn parse_model_page_omits_optional_fields_the_provider_does_not_return_and_keeps_name_unchanged() {
    // `name` must stay byte-identical to the pre-widening shape — a stored
    // model preference matches against it.
    let body = json!({ "data": [{ "id": "claude-sonnet-5" }] });
    let (page, _) = parse_model_page(&body).unwrap();
    assert_eq!(page, vec![json!({ "name": "claude-sonnet-5" })]);
}

#[test]
fn parse_model_page_ok_empty_on_genuinely_empty_catalogue() {
    let body = json!({ "data": [] });
    let (page, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(page, Vec::<Value>::new());
    assert_eq!(cursor, None);
}

#[test]
fn parse_model_page_errors_when_data_field_is_missing() {
    let body = json!({ "unexpected": "shape" });
    assert!(matches!(
        parse_model_page(&body),
        Err(AppError::Provider(_))
    ));
}

#[test]
fn parse_model_page_carries_the_cursor_only_when_has_more_is_true() {
    let body = json!({
        "data": [{ "id": "claude-sonnet-5" }],
        "has_more": true,
        "last_id": "claude-sonnet-5",
    });
    let (_, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(cursor, Some("claude-sonnet-5".to_string()));
}

#[test]
fn parse_model_page_omits_the_cursor_when_has_more_is_false_even_with_a_last_id() {
    // A `last_id` can still be present on the final page — the cursor must be
    // driven by `has_more`, never by `last_id`'s mere presence, or pagination
    // would loop forever re-requesting the same last page.
    let body = json!({
        "data": [{ "id": "claude-sonnet-5" }],
        "has_more": false,
        "last_id": "claude-sonnet-5",
    });
    let (_, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(cursor, None);
}

#[test]
fn parse_model_page_errors_when_has_more_is_true_but_last_id_is_missing() {
    // `has_more: true` with no cursor to continue from is a malformed
    // response, not a clean end-of-pages — silently stopping would return a
    // truncated catalogue as `Ok`, exactly the bug pagination exists to fix.
    let body = json!({
        "data": [{ "id": "claude-sonnet-5" }],
        "has_more": true,
    });
    assert!(matches!(
        parse_model_page(&body),
        Err(AppError::Provider(_))
    ));
}

#[test]
fn parse_model_page_errors_when_has_more_is_true_but_last_id_is_blank() {
    let body = json!({
        "data": [{ "id": "claude-sonnet-5" }],
        "has_more": true,
        "last_id": "   ",
    });
    assert!(matches!(
        parse_model_page(&body),
        Err(AppError::Provider(_))
    ));
}

// `advance_cursor`/`PaginationStep`/`pagination_step` are shared, generic
// helpers now — see `ai_provider::mod`'s test module for their coverage.
// Duplicating them here per-adapter is exactly the "a rule implemented
// twice that silently stops agreeing" defect class this codebase keeps
// paying for; one copy, one set of tests.

// ── list_models_transport (wiremock — the pagination HTTP loop itself,
// not just pagination_step's pure decision) ─────────────────────────────────

#[tokio::test]
async fn list_models_transport_propagates_the_cursor_into_the_next_requests_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param_is_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-page1-last",
        })))
        .mount(&server)
        .await;
    // Only matches when `after_id` carries EXACTLY page 1's `last_id` —
    // proves the cursor is wired from the parsed response into the next
    // request's query, not just decided in the abstract by `pagination_step`.
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param("after_id", "claude-page1-last"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-opus-5" }],
            "has_more": false,
        })))
        .mount(&server)
        .await;

    let models = AnthropicClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect("both pages must be fetched, the cursor propagated between them");
    assert_eq!(
        models,
        vec![
            json!({ "name": "claude-sonnet-5" }),
            json!({ "name": "claude-opus-5" }),
        ]
    );
}

#[tokio::test]
async fn list_models_transport_propagates_a_mid_pagination_status_error_instead_of_the_pages_already_collected(
) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param_is_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-page1-last",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param("after_id", "claude-page1-last"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = AnthropicClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err(
            "a failure on page 2 must reject the whole fetch, never return page 1's models alone",
        );
    // 500 classifies as Network via `friendly_api_error` — not the point of
    // this test (see the classification tests), but asserted so a future
    // change that silently swallows page 2's error can't slip through.
    assert!(matches!(err, AppError::Network(_)));
}

#[tokio::test]
async fn list_models_transport_errors_when_page_two_is_malformed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param_is_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-page1-last",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param("after_id", "claude-page1-last"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "unexpected": "shape" })))
        .mount(&server)
        .await;

    let err = AnthropicClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err("a malformed page 2 body must reject the whole fetch");
    assert!(matches!(err, AppError::Provider(_)));
}

#[tokio::test]
async fn list_models_transport_preserves_provider_context_on_a_non_json_page_two_body() {
    // Regression pin: the `.await??` rewrite in 70738c63 dropped the
    // "{name}: parse: " context and flipped this from `Provider` to `Parse`
    // (`error.rs`'s `From<serde_json::Error>`) — a change to the renderer-
    // visible error the sweep never intended. Unlike the "malformed shape"
    // test above (valid JSON, wrong fields — caught by `parse_model_page`),
    // this body isn't JSON at all, so it fails inside `read_json_capped`
    // itself, exercising the exact line that regressed.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param_is_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-page1-last",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param("after_id", "claude-page1-last"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json at all"))
        .mount(&server)
        .await;

    let err = AnthropicClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err("a non-JSON page 2 body must reject the whole fetch");
    assert!(matches!(err, AppError::Provider(_)));
    assert_eq!(
        err.to_string(),
        "anthropic: parse: response body did not match the expected schema"
    );
}

#[tokio::test]
async fn list_models_transport_errors_when_the_provider_reports_a_stalled_cursor() {
    // `has_more: true` with the SAME `last_id` twice: the provider claims
    // more data exists but gives no way to reach it. The exact regression
    // this whole finding is about — silently treating this as `Done` and
    // returning the one page collected as `Ok` — must not happen at the
    // transport level either, not just in the pure `pagination_step` unit
    // tests.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param_is_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-stuck",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param("after_id", "claude-stuck"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-stuck",
        })))
        .mount(&server)
        .await;

    let err = AnthropicClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err("a stalled cursor must reject, never silently return the partial catalogue");
    assert!(matches!(err, AppError::Provider(_)));
}

#[tokio::test]
async fn list_models_transport_errors_when_the_cumulative_deadline_fires_across_multiple_pages() {
    // Page 1 responds instantly (well within budget) and reports a cursor.
    // Page 2 writes its HEADERS instantly too — `send()` resolves fast — but
    // stalls its BODY well past the REMAINING budget. A deadline that only
    // wraps `send()` (the pre-fix bug) would let this succeed late instead
    // of erroring; only wrapping the body reads too catches it. Verified
    // (before applying that fix) that this test genuinely fails against the
    // unfixed transport — it doesn't error at all, it just returns `Ok`
    // after the full body delay, which is the exact silent-non-enforcement
    // bug this test exists to catch.
    let page1 = json!({
        "data": [{ "id": "claude-sonnet-5" }],
        "has_more": true,
        "last_id": "claude-page1-last",
    })
    .to_string();
    let page2 = json!({
        "data": [{ "id": "claude-opus-5" }],
        "has_more": false,
    })
    .to_string();
    let base = slow_body_server(vec![
        (page1, Duration::ZERO),
        (page2, Duration::from_millis(100)),
    ])
    .await;

    let err = AnthropicClient
        .list_models_transport(&base, "dummy-key", Duration::from_millis(40))
        .await
        .expect_err("page 2's body delay must exceed the REMAINING cumulative budget after page 1");
    assert!(matches!(err, AppError::Network(_)));
}

// ── Structured-output model gate ──────────────────────────────────────────────

#[test]
fn structured_outputs_gate_covers_the_4_5_generation_and_later_plus_opus_4_1() {
    for model in [
        "claude-opus-4-1-20250805",
        "claude-opus-4-5",
        "claude-sonnet-4-5-20250929",
        "claude-haiku-4-5",
        "claude-sonnet-4-6",
        "claude-opus-4-8",
        "claude-opus-5",
        "claude-fable-5",
        // Vendor-prefixed (OpenRouter-style) ids normalize the same way every
        // other gate in this file does.
        "anthropic/claude-sonnet-5",
        // Dot-form version separators collapse to dashes too.
        "claude-opus-4.5",
    ] {
        assert!(
            super::anthropic_supports_structured_outputs(model),
            "{model} must be recognized as structured-output capable"
        );
    }
}

#[test]
fn structured_outputs_gate_rejects_pre_4_5_models_and_anything_unrecognized() {
    for model in [
        // Claude 3.x and the 4.0 models never got structured outputs.
        "claude-3-opus-20240229",
        "claude-3-5-sonnet-20241022",
        "claude-3-7-sonnet-20250219",
        "claude-opus-4-20250514",
        "claude-sonnet-4-20250514",
        // An unrecognized/future id defaults to unsupported — the fallback
        // always works, a wrongly-claimed capability does not.
        "claude-something-new",
        "",
        // Boundary check: `opus-4-50` must not match the `opus-4-5` needle.
        "claude-opus-4-50",
    ] {
        assert!(
            !super::anthropic_supports_structured_outputs(model),
            "{model} must NOT be claimed as structured-output capable"
        );
    }
}

#[test]
fn capabilities_reports_json_mode_from_the_structured_output_gate() {
    // The declared capability is the gate, not a hardcoded blanket value —
    // mutation check: pin `supports_json_mode` back to `false` and this fails.
    assert!(
        AnthropicClient
            .capabilities("claude-sonnet-4-5")
            .supports_json_mode
    );
    assert!(
        !AnthropicClient
            .capabilities("claude-3-5-sonnet-20241022")
            .supports_json_mode
    );
}
