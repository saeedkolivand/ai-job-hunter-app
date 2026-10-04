use super::{support::salary, *};

// ── role_is_missing (enrich's early-return guard) ────────────────────────

#[test]
fn role_is_missing_flags_empty_and_whitespace_only() {
    assert!(role_is_missing(""));
    assert!(role_is_missing("   "));
    assert!(role_is_missing("\t\n  "));
}

#[test]
fn role_is_missing_accepts_a_real_role() {
    assert!(!role_is_missing("Backend Engineer"));
    assert!(!role_is_missing("  Backend Engineer  "));
}

// ── truncate_input ────────────────────────────────────────────────────────

#[test]
fn truncate_input_caps_at_max_input_chars() {
    let long = "a".repeat(500);
    let truncated = truncate_input(&long);
    assert_eq!(truncated.chars().count(), MAX_INPUT_CHARS);
}

#[test]
fn truncate_input_is_a_no_op_under_the_cap() {
    assert_eq!(truncate_input("Backend Engineer"), "Backend Engineer");
}

#[test]
fn truncate_input_never_splits_a_multi_byte_character() {
    // A multi-byte (CJK) string longer than the cap must still produce
    // valid UTF-8 — `.chars().take(n)` is boundary-safe by construction,
    // unlike a byte-index slice.
    let long: String = "日".repeat(500);
    let truncated = truncate_input(&long);
    assert_eq!(truncated.chars().count(), MAX_INPUT_CHARS);
    assert!(truncated.chars().all(|c| c == '日'));
}

// ── cache_key (case-folding) ─────────────────────────────────────────────

#[test]
fn cache_key_case_folds_so_differently_cased_inputs_collide() {
    assert_eq!(
        cache_key("Backend Engineer", "Acme", "Berlin", "EUR"),
        cache_key("backend engineer", "ACME", "berlin", "eur")
    );
}

#[test]
fn cache_key_preserves_the_pipe_delimited_shape() {
    assert_eq!(
        cache_key("Backend Engineer", "Acme", "Berlin", "EUR"),
        "backend engineer|acme|berlin|eur"
    );
}

#[test]
fn cache_key_differs_by_currency_so_cross_country_postings_never_collide() {
    // The bug this fixes: a DE (EUR) and a US (USD) "Remote" posting share
    // role/company/location, so without currency in the key they'd land
    // on the same cache row — and an unknown-currency ("") read could then
    // surface whatever currency a different, known-currency job last
    // wrote there.
    let de = cache_key("Backend Engineer", "Acme", "Remote", "EUR");
    let us = cache_key("Backend Engineer", "Acme", "Remote", "USD");
    let unknown = cache_key("Backend Engineer", "Acme", "Remote", "");
    assert_ne!(de, us);
    assert_ne!(de, unknown);
    assert_ne!(us, unknown);
}

#[test]
fn a_no_info_response_never_produces_a_cacheable_value() {
    // `enrich` only calls `cache.set` with the output of a successful
    // `parse_and_validate` — a `{}` ("no reliable data") response yields
    // `None` here, so the caller has nothing to cache (never a stale miss
    // stuck for the 7-day TTL).
    assert_eq!(parse_and_validate("{}"), None);
}

// ── extract_json_object ──────────────────────────────────────────────────

#[test]
fn extract_json_object_finds_a_bare_object() {
    assert_eq!(
        extract_json_object(r#"{"min":1,"max":2,"currency":"USD"}"#),
        Some(r#"{"min":1,"max":2,"currency":"USD"}"#)
    );
}

#[test]
fn extract_json_object_ignores_surrounding_prose_and_fences() {
    let text = "Sure, here you go:\n```json\n{\"min\":1,\"max\":2,\"currency\":\"USD\"}\n```";
    assert_eq!(
        extract_json_object(text),
        Some(r#"{"min":1,"max":2,"currency":"USD"}"#)
    );
}

#[test]
fn extract_json_object_none_without_braces() {
    assert_eq!(extract_json_object("no data available"), None);
}

// ── parse_and_validate ───────────────────────────────────────────────────

#[test]
fn valid_json_parses_to_some() {
    let range = parse_and_validate(r#"{"min":65000,"max":80000,"currency":"eur"}"#).unwrap();
    assert_eq!(
        range,
        SalaryRange {
            min: 65000,
            max: 80000,
            currency: "EUR".to_string()
        }
    );
}

#[test]
fn empty_object_no_info_is_none() {
    assert_eq!(parse_and_validate("{}"), None);
}

#[test]
fn malformed_json_is_none() {
    assert_eq!(parse_and_validate("not json at all"), None);
}

#[test]
fn negative_or_zero_values_are_rejected() {
    // serde_json has no negative-as-u64, so a negative min fails `as_u64`.
    assert_eq!(
        parse_and_validate(r#"{"min":-5,"max":80000,"currency":"USD"}"#),
        None
    );
    assert_eq!(
        parse_and_validate(r#"{"min":0,"max":80000,"currency":"USD"}"#),
        None
    );
    assert_eq!(
        parse_and_validate(r#"{"min":50000,"max":0,"currency":"USD"}"#),
        None
    );
}

#[test]
fn min_greater_than_max_is_rejected() {
    assert_eq!(
        parse_and_validate(r#"{"min":90000,"max":80000,"currency":"USD"}"#),
        None
    );
}

#[test]
fn absurdly_large_values_are_rejected() {
    assert_eq!(
        parse_and_validate(r#"{"min":1,"max":999999999999,"currency":"USD"}"#),
        None
    );
}

#[test]
fn bad_currency_shapes_are_rejected() {
    for currency in ["", "U", "US", "TOOLONG", "12A", "eu-r"] {
        let text = format!(r#"{{"min":1,"max":2,"currency":"{currency}"}}"#);
        assert_eq!(parse_and_validate(&text), None, "currency={currency:?}");
    }
}

#[test]
fn four_letter_currency_codes_are_accepted() {
    let range = parse_and_validate(r#"{"min":1,"max":2,"currency":"USDX"}"#).unwrap();
    assert_eq!(range.currency, "USDX");
}

#[test]
fn missing_fields_are_none() {
    assert_eq!(parse_and_validate(r#"{"min":1,"max":2}"#), None);
    assert_eq!(parse_and_validate(r#"{"currency":"USD"}"#), None);
}

// ── currency grounding (the bug fix): reconcile_expected_currency ──────────
//
// Fail-safe, not relabel: a mismatched currency is dropped (`None`), never
// overridden onto the (wrong-currency) numbers.

#[test]
fn reconcile_expected_currency_drops_a_mismatched_range() {
    let range = salary(1, 2, "USD");
    assert_eq!(reconcile_expected_currency(range, "EUR"), None);
}

#[test]
fn reconcile_expected_currency_is_a_no_op_when_the_expected_currency_is_unknown() {
    // Unknown-country guard: an empty expected currency must never drop or
    // mutate the parsed range — today's unconstrained behavior.
    let range = salary(1, 2, "USD");
    assert_eq!(reconcile_expected_currency(range.clone(), ""), Some(range));
}

#[test]
fn reconcile_expected_currency_is_a_no_op_when_it_already_matches_case_insensitively() {
    let range = salary(1, 2, "eur");
    // Already-matching (modulo case) is kept exactly as parsed.
    assert_eq!(
        reconcile_expected_currency(range.clone(), "eur"),
        Some(range)
    );
}
