use super::*;

/// Assert `model` matches the `RATES` row `prefix` — the matched PREFIX, not the
/// price, since several rows share one (see `rate_for`).
#[track_caller]
fn assert_row(model: &str, prefix: &str) {
    assert_eq!(rate_for(model).map(|(p, _, _)| *p), Some(prefix));
}

// ── Pure rate/cost fns ──────────────────────────────────────────────────────

#[test]
fn estimate_cost_matches_known_model_rate() {
    // 1M input + 1M output tokens on gpt-4o-mini == the table's per-1M rates.
    let cost = estimate_cost("gpt-4o-mini", 1_000_000, 1_000_000);
    assert!(
        (cost - (0.15 + 0.60)).abs() < 1e-9,
        "expected 0.75, got {cost}"
    );
}

#[test]
fn estimate_cost_prefers_the_more_specific_prefix() {
    // "gpt-4o-mini" must NOT match the shorter "gpt-4o" prefix's higher rate.
    let mini = estimate_cost("gpt-4o-mini-2024-07-18", 1_000_000, 0);
    let full = estimate_cost("gpt-4o-2024-08-06", 1_000_000, 0);
    assert!(
        (mini - 0.15).abs() < 1e-9,
        "mini should be 0.15, got {mini}"
    );
    assert!(
        (full - 2.50).abs() < 1e-9,
        "full should be 2.50, got {full}"
    );
    assert!(mini < full, "mini must be cheaper than full gpt-4o");
}

#[test]
fn estimate_cost_falls_back_to_default_rate_for_unknown_model() {
    let cost = estimate_cost(
        "some-brand-new-model-nobody-has-heard-of",
        1_000_000,
        1_000_000,
    );
    assert!(
        (cost - (DEFAULT_RATE.0 + DEFAULT_RATE.1)).abs() < 1e-9,
        "unrecognized model must use DEFAULT_RATE, got {cost}"
    );
    assert!(
        cost > 0.0,
        "an unknown PAID model must never silently cost $0"
    );
}

#[test]
fn estimate_cost_zero_tokens_is_zero_cost() {
    assert_eq!(estimate_cost("gpt-4o", 0, 0), 0.0);
}

#[test]
fn estimate_cost_strips_a_leading_models_prefix() {
    // Gemini ids can arrive as "models/gemini-2.5-flash" — must match the
    // same rate as the bare id, not fall through to DEFAULT_RATE.
    let prefixed = estimate_cost("models/gemini-2.5-flash", 1_000_000, 0);
    let bare = estimate_cost("gemini-2.5-flash", 1_000_000, 0);
    assert!((prefixed - bare).abs() < 1e-9);
    assert!(
        (prefixed - 0.30).abs() < 1e-9,
        "expected the gemini-2.5-flash rate, got {prefixed} (looks like DEFAULT_RATE)"
    );
}

#[test]
fn estimate_cost_matches_the_claude_5_family_rates() {
    // Each Claude 5 model must hit its own row, not DEFAULT_RATE and not a
    // same-tier 4.x row it happens to share a numeric substring with.
    let fable = estimate_cost("claude-fable-5", 1_000_000, 1_000_000);
    let opus5 = estimate_cost("claude-opus-5-20260201", 1_000_000, 1_000_000);
    assert!((fable - (10.00 + 50.00)).abs() < 1e-9, "got {fable}");
    assert!((opus5 - (5.00 + 25.00)).abs() < 1e-9, "got {opus5}");
    // "claude-sonnet-5"'s $3/$15 list price is numerically identical to
    // DEFAULT_RATE, so a price-only assertion here is vacuous (it would pass
    // even if sonnet-5 fell straight through to DEFAULT_RATE without
    // matching any row). Assert the matched PREFIX instead.
    assert_eq!(
        rate_for("claude-sonnet-5").map(|(p, _, _)| *p),
        Some("claude-sonnet-5"),
        "claude-sonnet-5 must hit its own row, not fall through to DEFAULT_RATE"
    );
}

#[test]
fn estimate_cost_claude_5_and_4_prefixes_do_not_shadow_each_other() {
    // "claude-opus-5"/"claude-sonnet-5" must never fall through to the
    // "claude-opus-4"/"claude-sonnet-4" rows (or vice versa) — the digit
    // makes the prefixes distinct, but a future reordering could still break
    // this, so pin it.
    let opus5 = estimate_cost("claude-opus-5", 1_000_000, 0);
    let opus4 = estimate_cost("claude-opus-4-20250514", 1_000_000, 0);
    assert!((opus5 - 5.00).abs() < 1e-9, "got {opus5}");
    assert!((opus4 - 15.00).abs() < 1e-9, "got {opus4}");

    // "claude-sonnet-5" and "claude-sonnet-4-5" (Sonnet 4.5) happen to share
    // the same $3/$15 price — and DEFAULT_RATE is *also* $3/$15 — so a
    // price-only assertion here would pass even if sonnet-5 fell straight
    // through to DEFAULT_RATE without matching any row at all. Assert the
    // matched PREFIX instead.
    assert_eq!(
        rate_for("claude-sonnet-5").map(|(p, _, _)| *p),
        Some("claude-sonnet-5"),
        "claude-sonnet-5 must hit its own row, not fall through to DEFAULT_RATE"
    );
    assert_row("claude-sonnet-4-5", "claude-sonnet-4");
}

#[test]
fn rate_for_gives_opus_4_5_its_own_row_instead_of_the_shorter_opus_4_prefix() {
    // Before the "claude-opus-4-5" row existed, an Opus 4.5 model id matched
    // the shorter "claude-opus-4" prefix and was billed at its $15/$75 rate —
    // a 3x overestimate of Opus 4.5's actual $5/$25 pricing.
    assert_row("claude-opus-4-5-20260101", "claude-opus-4-5");
    let cost = estimate_cost("claude-opus-4-5-20260101", 1_000_000, 1_000_000);
    assert!((cost - (5.00 + 25.00)).abs() < 1e-9, "got {cost}");
    // Same shadowing bug, same fix, for Opus 4.7 (VERIFIED $5/$25 pricing,
    // platform.claude.com legacy models table) — before its own row existed
    // it also fell through to the $15/$75 "claude-opus-4" row.
    assert_row("claude-opus-4-7-20260101", "claude-opus-4-7");
    let opus47_cost = estimate_cost("claude-opus-4-7-20260101", 1_000_000, 1_000_000);
    assert!(
        (opus47_cost - (5.00 + 25.00)).abs() < 1e-9,
        "got {opus47_cost}"
    );
    // Plain Opus 4 (no point-release suffix) must still hit the original,
    // more expensive row.
    assert_row("claude-opus-4-20250514", "claude-opus-4");
}

#[test]
fn rate_for_gives_gpt_4_1106_its_own_row_instead_of_the_normalized_gpt_4_1_prefix() {
    // Post dot/dash normalization, "gpt-4.1" becomes the prefix "gpt-4-1",
    // which is ALSO a prefix of "gpt-4-1106-preview" — without its own row,
    // GPT-4 Turbo's 1106 snapshot silently matched the $2/$8 gpt-4.1 rate
    // instead of its actual $10/$30 list price.
    assert_row("gpt-4-1106-preview", "gpt-4-1106");
    assert_row("gpt-4-1106-vision-preview", "gpt-4-1106");
    let cost = estimate_cost("gpt-4-1106-preview", 1_000_000, 1_000_000);
    assert!((cost - (10.00 + 30.00)).abs() < 1e-9, "got {cost}");
    // Bare "gpt-4.1" itself must still hit its own row, unaffected.
    assert_row("gpt-4.1-2025-04-14", "gpt-4.1");
}

#[test]
fn rate_for_matches_a_dot_form_id_against_a_dash_form_row() {
    // "claude-opus-4.7" (dot form) must hit the "claude-opus-4-7" row, not
    // fall through the shorter "claude-opus-4" row (or DEFAULT_RATE).
    assert_row("claude-opus-4.7", "claude-opus-4-7");
    let cost = estimate_cost("claude-opus-4.7", 1_000_000, 1_000_000);
    assert!((cost - (5.00 + 25.00)).abs() < 1e-9, "got {cost}");
    // The dot/dash equivalence must NOT corrupt OpenAI's own literal-dot
    // version rows (`gpt-4.1-mini`) — normalizing both sides identically
    // keeps this match intact.
    assert_row("gpt-4.1-mini-2025-04-14", "gpt-4.1-mini");
}

#[test]
fn rate_for_gives_gemini_3_pro_preview_its_own_row_not_the_default_fallback() {
    // No longer curated-list selectable — the model is SHUT DOWN and
    // `provider-meta.ts` now ships `gemini-3.6-flash` instead — but the row
    // is kept for historical spend records (see the doc comment in
    // `spend/mod.rs`), so it must still resolve to its own real rate rather
    // than falling through to DEFAULT_RATE (3.00/15.00).
    assert_row("gemini-3-pro-preview", "gemini-3-pro-preview");
    let cost = estimate_cost("gemini-3-pro-preview", 1_000_000, 1_000_000);
    assert!((cost - (2.00 + 12.00)).abs() < 1e-9, "got {cost}");
    // Its `.1` sibling shares the "gemini-3-" prefix after dot/dash
    // normalization but must resolve to ITS OWN row, not this one (or
    // DEFAULT_RATE) — a bare `assert_ne!` only proves no WRONG collision,
    // not that the right row exists at all.
    assert_row("gemini-3.1-pro-preview", "gemini-3.1-pro-preview");
    let sibling_cost = estimate_cost("gemini-3.1-pro-preview", 1_000_000, 1_000_000);
    assert!(
        (sibling_cost - (2.00 + 12.00)).abs() < 1e-9,
        "got {sibling_cost}"
    );
}

#[test]
fn rate_for_gives_gemini_3_6_flash_its_own_row_not_the_default_fallback() {
    // Curated-list default (`provider-meta.ts`, replacing the shut-down
    // `gemini-3-pro-preview`) — must resolve to its own real rate, not
    // DEFAULT_RATE (3.00/15.00).
    assert_row("gemini-3.6-flash", "gemini-3.6-flash");
    let cost = estimate_cost("gemini-3.6-flash", 1_000_000, 1_000_000);
    assert!((cost - (1.50 + 7.50)).abs() < 1e-9, "got {cost}");
}

#[test]
fn rate_for_gives_gemini_3_5_flash_and_lite_their_own_rows_not_the_default_fallback() {
    // Both curated-list entries (`provider-meta.ts`) — `-lite` must resolve
    // to its OWN row, not fall through to the bare `-flash` row it shares a
    // dot/dash-normalized prefix with.
    assert_row("gemini-3.5-flash-lite", "gemini-3.5-flash-lite");
    let lite_cost = estimate_cost("gemini-3.5-flash-lite", 1_000_000, 1_000_000);
    assert!((lite_cost - (0.30 + 2.50)).abs() < 1e-9, "got {lite_cost}");

    assert_row("gemini-3.5-flash", "gemini-3.5-flash");
    let flash_cost = estimate_cost("gemini-3.5-flash", 1_000_000, 1_000_000);
    assert!(
        (flash_cost - (1.50 + 9.00)).abs() < 1e-9,
        "got {flash_cost}"
    );
}

#[test]
fn rate_for_gives_ollama_cloud_gpt_oss_ids_their_own_rows_not_default_rate() {
    // The exact defect this closes: Ollama Cloud's `gpt-oss:120b` (colon-tag
    // form — NOT dash-form) previously fell straight through to DEFAULT_RATE
    // ($3/$15), massively overbilling a cheap open-weight model.
    assert_row("gpt-oss:120b", "gpt-oss:120b");
    let cost_120b = estimate_cost("gpt-oss:120b", 1_000_000, 1_000_000);
    assert!((cost_120b - (0.037 + 0.17)).abs() < 1e-9, "got {cost_120b}");
    assert!(
        cost_120b < DEFAULT_RATE.0 + DEFAULT_RATE.1,
        "gpt-oss:120b must be far cheaper than the DEFAULT_RATE it used to fall through to"
    );

    assert_row("gpt-oss:20b", "gpt-oss:20b");
    let cost_20b = estimate_cost("gpt-oss:20b", 1_000_000, 1_000_000);
    assert!((cost_20b - (0.03 + 0.13)).abs() < 1e-9, "got {cost_20b}");
}

#[test]
fn rate_for_falls_back_to_the_bare_gpt_oss_row_for_an_unlisted_spelling() {
    // A dash-form id (another gateway hosting the same open model) or any
    // other unlisted size must still hit the family catch-all, not DEFAULT_RATE.
    assert_row("gpt-oss-120b", "gpt-oss");
    // An actually-unlisted size — `-safeguard` has its own row below.
    let cost = estimate_cost("gpt-oss-400b", 1_000_000, 1_000_000);
    assert!((cost - (0.037 + 0.17)).abs() < 1e-9, "got {cost}");
}

#[test]
fn rate_for_prices_gpt_oss_safeguard_above_the_family_catch_all() {
    // Regression: `gpt-oss-safeguard-20b` costs MORE than either base size
    // ($0.075/$0.30 per 1M), so the bare `gpt-oss` catch-all under-costs it by
    // ~2x. Its own row must out-rank the catch-all.
    assert_row("gpt-oss-safeguard-20b", "gpt-oss-safeguard");
    let cost = estimate_cost("gpt-oss-safeguard-20b", 1_000_000, 1_000_000);
    assert!((cost - (0.075 + 0.30)).abs() < 1e-9, "got {cost}");
    // Strictly more expensive than the catch-all it used to fall through to.
    assert!(cost > estimate_cost("gpt-oss-400b", 1_000_000, 1_000_000));
}

#[test]
fn estimate_cost_strips_a_vendor_prefix_before_matching() {
    // An id seen through an OpenRouter-style gateway (`anthropic/claude-fable-5`)
    // must hit the bare model's row, not silently fall through to DEFAULT_RATE.
    assert_row("anthropic/claude-fable-5", "claude-fable-5");
    let prefixed = estimate_cost("anthropic/claude-fable-5", 1_000_000, 1_000_000);
    let bare = estimate_cost("claude-fable-5", 1_000_000, 1_000_000);
    assert!((prefixed - bare).abs() < 1e-9);
    assert!((prefixed - (10.00 + 50.00)).abs() < 1e-9, "got {prefixed}");
}

#[test]
fn is_free_provider_covers_local_and_cli_agents_only() {
    for p in [
        "ollama",
        "claude-code",
        "codex",
        "gemini-cli",
        "antigravity",
    ] {
        assert!(is_free_provider(p), "{p} must be free");
    }
    for p in [
        "ollama-cloud",
        "openai",
        "anthropic",
        "gemini",
        "openai-compatible",
    ] {
        assert!(!is_free_provider(p), "{p} must NOT be free");
    }
}

#[test]
fn is_localhost_url_recognizes_common_local_forms() {
    for url in [
        "http://localhost:1234/v1",
        "http://127.0.0.1:1234/v1",
        "http://0.0.0.0:8080",
        "https://localhost/v1",
        "localhost:11434",
        "http://[::1]:1234/v1",
        // IPv6 loopback WITHOUT a port: the last colon is an internal one, so
        // splitting on it used to cut the host down to "[:" and bill a free
        // local call at the unknown-model DEFAULT_RATE.
        "http://[::1]/v1",
        "http://[::1]",
        "[::1]",
        // The bare (unbracketed) spelling the match arm below also accepts.
        "::1",
        // Fully-expanded and IPv4-mapped IPv6 loopback spellings, bracketed
        // (with/without a port) and bare — all the same free local call.
        "http://[0:0:0:0:0:0:0:1]:1234/v1",
        "http://[0:0:0:0:0:0:0:1]",
        "0:0:0:0:0:0:0:1",
        "http://[::ffff:127.0.0.1]:8080/v1",
        "http://[::ffff:127.0.0.1]",
        "::ffff:127.0.0.1",
        // A `userinfo@` prefix must not defeat the host match.
        "http://user@[::1]/v1",
        "http://user:pass@[::1]:1234/v1",
        "http://user@127.0.0.1:1234/v1",
    ] {
        assert!(is_localhost_url(url), "{url} should be recognized as local");
    }
    for url in [
        "https://openrouter.ai/api/v1",
        "https://api.groq.com/openai/v1",
        "http://my-localhost-lookalike.example.com/v1",
    ] {
        assert!(!is_localhost_url(url), "{url} must NOT be treated as local");
    }
}

#[test]
fn is_free_call_is_free_only_for_local_providers_and_openai_compatible_localhost() {
    for (provider, base_url, free) in [
        ("openai-compatible", Some("http://localhost:1234/v1"), true),
        ("openai-compatible", Some("http://127.0.0.1:8080"), true),
        // A port-less IPv6 loopback is just as free as its ported form.
        ("openai-compatible", Some("http://[::1]/v1"), true),
        // Remote OpenAI-compatible gateways still charge.
        (
            "openai-compatible",
            Some("https://openrouter.ai/api/v1"),
            false,
        ),
        ("openai-compatible", None, false),
        // A localhost-looking base_url must never make a genuinely paid cloud
        // provider id look free.
        ("openai", Some("http://localhost:1234"), false),
        ("ollama", Some("https://not-actually-checked"), true),
    ] {
        assert_eq!(
            is_free_call(provider, base_url),
            free,
            "{provider} {base_url:?}"
        );
    }
}
