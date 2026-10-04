use super::{support::*, *};

// ── Window scoping (issue #1161) ─────────────────────────────────────────────

#[test]
fn window_start_ms_of_one_day_is_exactly_today_start() {
    assert_eq!(window_start_ms(1), today_start_ms());
    assert_eq!(window_start_ms(0), today_start_ms(), "0 days means 1 day");
}

#[test]
fn window_start_ms_of_seven_days_reaches_six_full_days_back() {
    assert_eq!(
        window_start_ms(7),
        today_start_ms() - 6 * 86_400_000,
        "day 1 of the window is today itself, so 7 days reaches back 6 more"
    );
}

#[test]
fn totals_since_and_by_provider_since_only_count_rows_inside_the_window() {
    let (_dir, store) = open_store();

    // Today: counts in every window.
    store.record(rec("openai", "gpt-4o-mini", 1_000, 1_000));
    // 3 days ago: inside a 7-day window, outside a 1-day window.
    insert_backdated(&store, 3, "anthropic", "claude-sonnet-5", 2_000);
    // 30 days ago: outside both windows below.
    insert_backdated(&store, 30, "gemini", "gemini-2.5-flash", 5_000);

    let one_day = store.totals_since(window_start_ms(1));
    assert_eq!(one_day.input_tokens, 1_000, "only today's row counts");

    let seven_day = store.totals_since(window_start_ms(7));
    assert_eq!(
        seven_day.input_tokens, 3_000,
        "today's + the 3-day-old row count; the 30-day-old row does not"
    );

    let by_provider_7d = store.by_provider_since(window_start_ms(7));
    assert_eq!(by_provider_7d.len(), 2, "only openai and anthropic appear");
    assert!(by_provider_7d.iter().any(|p| p.provider == "openai"));
    assert!(by_provider_7d.iter().any(|p| p.provider == "anthropic"));
    assert!(!by_provider_7d.iter().any(|p| p.provider == "gemini"));

    // since_ms = 0 (all-time) picks up every provider ever recorded.
    let all_time = store.by_provider_since(0);
    assert_eq!(all_time.len(), 3);
}

#[test]
fn zero_row_reason_labels_each_kind_of_quiet_provider() {
    for (provider, input_tokens, output_tokens, est_cost_usd, expected) in [
        // A free provider is local regardless of all-time cost.
        ("ollama", 10_000, 5_000, 0.0, "local — always $0"),
        // A non-free provider (e.g. openai-compatible always pointed at a local
        // server) that moved real tokens but never actually cost anything.
        ("openai-compatible", 8_000, 4_000, 0.0, "not priced"),
        // A genuinely quiet paid provider.
        ("openai", 100_000, 50_000, 1.23, "no spend in window"),
        // A provider row that (in principle) has zero tokens and zero cost
        // all-time must not be misread as "not priced" — that reason is reserved
        // for a provider that moved REAL tokens without ever costing anything.
        ("openai", 0, 0, 0.0, "no spend in window"),
    ] {
        let hist = ProviderTotals {
            provider: provider.to_string(),
            input_tokens,
            output_tokens,
            est_cost_usd,
        };
        assert_eq!(
            zero_row_reason(&hist),
            expected,
            "{provider} {input_tokens}"
        );
    }
}
