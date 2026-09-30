//! `model_entry` / `parse_rfc3339_millis` (list_models projection),
//! `advance_cursor` / `pagination_step` / `bounded` (cursor pagination), and
//! `cosine` (raw vector similarity) — pure, cross-adapter infra with no
//! provider-specific logic.

use serde_json::json;

use super::super::*;

// ── model_entry / parse_rfc3339_millis (list_models projection) ────────────

#[test]
fn model_entry_carries_only_name_when_every_other_field_is_none() {
    // Byte-identical to the pre-widening shape — a stored model
    // preference matches on `name` alone, so this must never gain a
    // fabricated field just because it CAN.
    assert_eq!(
        model_entry("claude-sonnet-5", None, None, None),
        json!({ "name": "claude-sonnet-5" })
    );
}

#[test]
fn model_entry_includes_only_the_fields_that_are_some() {
    assert_eq!(
        model_entry("gpt-5.6", Some("GPT-5.6"), None, Some(200_000)),
        json!({ "name": "gpt-5.6", "displayName": "GPT-5.6", "contextLength": 200_000 })
    );
}

#[test]
fn parse_rfc3339_millis_converts_a_known_reference_timestamp() {
    // 2024-01-01T00:00:00Z is the well-known 1704067200 unix-epoch-SECONDS
    // reference point — asserted here as the expected epoch-MILLISECONDS
    // value this codebase's `createdAt` convention uses.
    assert_eq!(
        parse_rfc3339_millis("2024-01-01T00:00:00Z"),
        Some(1_704_067_200_000)
    );
}

#[test]
fn parse_rfc3339_millis_handles_a_non_utc_offset() {
    // Ollama's `modified_at` may carry a non-UTC offset (e.g. `-07:00`) —
    // the epoch value is offset-independent, so 07:00 UTC-7 is the same
    // instant as 00:00 UTC the same reference day plus 7 hours... concretely:
    // 2024-01-01T00:00:00-07:00 == 2024-01-01T07:00:00Z.
    assert_eq!(
        parse_rfc3339_millis("2024-01-01T00:00:00-07:00"),
        Some(1_704_067_200_000 + 7 * 3_600_000)
    );
}

#[test]
fn parse_rfc3339_millis_is_none_on_a_malformed_timestamp() {
    // Never a fabricated/zero timestamp — a parse failure degrades
    // exactly like the field being absent.
    assert_eq!(parse_rfc3339_millis("not a timestamp"), None);
    assert_eq!(parse_rfc3339_millis(""), None);
}

// ── advance_cursor / pagination_step (shared by every paginated adapter) ───

#[test]
fn advance_cursor_is_done_only_when_there_is_no_cursor_at_all() {
    assert_eq!(advance_cursor::<String>(&None, None), CursorProgress::Done);
    assert_eq!(
        advance_cursor(&Some("id1".to_string()), None),
        CursorProgress::Done
    );
}

#[test]
fn advance_cursor_is_stalled_not_done_on_a_non_advancing_cursor() {
    // The exact regression this guards against: a provider handing back the
    // SAME cursor it was just called with is NEITHER a clean end-of-pages
    // (there's a cursor — more data is claimed) NOR safe to loop on forever.
    // Folding this into `Done` is silent truncation; it must be its own
    // outcome so the caller can reject instead of returning `Ok`.
    assert_eq!(
        advance_cursor(&Some("id1".to_string()), Some("id1".to_string())),
        CursorProgress::Stalled
    );
}

#[test]
fn advance_cursor_continues_on_a_genuinely_new_cursor() {
    assert_eq!(
        advance_cursor(&None, Some("id1".to_string())),
        CursorProgress::Continue("id1".to_string())
    );
    assert_eq!(
        advance_cursor(&Some("id1".to_string()), Some("id2".to_string())),
        CursorProgress::Continue("id2".to_string())
    );
}

#[test]
fn pagination_step_errors_incomplete_at_the_final_page_with_an_advancing_cursor() {
    // The exact boundary this exists for: page index `max_pages - 1` is
    // the LAST iteration a `max_pages`-bounded `for` loop runs — a
    // genuinely new cursor there means there's more catalogue the fetch
    // won't cover, and that must reject, not silently return `Ok`.
    assert_eq!(
        pagination_step(49, 50, &Some("id48".to_string()), Some("id49".to_string())),
        PaginationStep::Incomplete
    );
}

#[test]
fn pagination_step_continues_before_the_final_page() {
    assert_eq!(
        pagination_step(0, 50, &None, Some("id1".to_string())),
        PaginationStep::Continue("id1".to_string())
    );
    assert_eq!(
        pagination_step(48, 50, &Some("id47".to_string()), Some("id48".to_string())),
        PaginationStep::Continue("id48".to_string())
    );
}

#[test]
fn pagination_step_is_done_when_there_is_no_next_page_even_at_the_final_index() {
    // A clean end-of-catalogue on the LAST allowed page is not
    // incomplete — only a still-advancing cursor at that boundary is.
    assert_eq!(
        pagination_step(49, 50, &Some("id48".to_string()), None),
        PaginationStep::Done
    );
}

#[test]
fn pagination_step_is_stalled_not_done_on_a_non_advancing_cursor() {
    // Reserving `Done` strictly for "no cursor at all" — a repeated cursor
    // must surface as `Stalled` (an error at the transport layer), never be
    // silently treated as a clean stop, at ANY page index (not just the
    // budget boundary — this is the same regression as
    // `advance_cursor_is_stalled_not_done_on_a_non_advancing_cursor`,
    // exercised through the full `pagination_step` a transport actually
    // calls).
    assert_eq!(
        pagination_step(0, 50, &Some("id1".to_string()), Some("id1".to_string())),
        PaginationStep::Stalled
    );
    assert_eq!(
        pagination_step(49, 50, &Some("id48".to_string()), Some("id48".to_string())),
        PaginationStep::Stalled
    );
}

#[tokio::test]
async fn bounded_maps_an_expired_deadline_to_a_named_network_error() {
    // The third member of the pagination trio had no test at all. It is the
    // one that decides what a stalled page fetch LOOKS like: a timeout has to
    // arrive as `AppError::Network` (retryable, and rendered as a connectivity
    // problem), never as a `Provider`/`Unknown` error the caller would report
    // as "the provider rejected the request", and it has to name the provider
    // so the message says WHICH catalogue fetch gave up. Mutation check: swap
    // the variant in `bounded`'s `map_err`, or drop `{provider}` from the
    // message, and this fails.
    let expired = tokio::time::Instant::now();
    let error = bounded(expired, "anthropic", std::future::pending::<()>())
        .await
        .expect_err("an already-expired deadline must not resolve");
    assert!(
        matches!(error, crate::error::AppError::Network(_)),
        "a cumulative-deadline timeout must be a Network error, got {error:?}"
    );
    assert!(
        error.to_string().contains("anthropic"),
        "the message must name the provider: {error}"
    );

    // …and the wrapper is not simply always-failing: a future that resolves
    // inside the deadline passes its value straight through.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    assert_eq!(
        bounded(deadline, "gemini", std::future::ready(7))
            .await
            .expect("resolves before the deadline"),
        7
    );
}

// ── cosine ───────────────────────────────────────────────────────────────

#[test]
fn cosine_identical_vectors_is_one() {
    let a = vec![1.0, 2.0, 3.0];
    assert!((cosine(&a, &a) - 1.0).abs() < 0.001);
}

#[test]
fn cosine_orthogonal_vectors_is_zero() {
    assert!((cosine(&[1.0, 0.0], &[0.0, 1.0]) - 0.0).abs() < 0.001);
}

#[test]
fn cosine_edge_cases_return_zero() {
    // Empty vectors, mismatched lengths, and zero vectors all yield 0.0.
    assert_eq!(cosine(&[], &[]), 0.0);
    assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0);
    assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
}
