use tempfile::TempDir;

use super::*;
use crate::spend::{ProviderTotals, SpendTotals};

// ── spend_totals_json (ai_spend_summary today/windowTotals shape, #1161) ──

#[test]
fn spend_totals_json_carries_the_exact_totals_given() {
    let totals = SpendTotals {
        input_tokens: 12_431,
        output_tokens: 3_204,
        est_cost_usd: 0.42,
    };

    let out = spend_totals_json(totals);
    assert_eq!(out["inputTokens"], 12_431);
    assert_eq!(out["outputTokens"], 3_204);
    assert_eq!(out["estCostUsd"], 0.42);
}

#[test]
fn spend_summary_value_labels_today_and_window_totals_from_their_own_input() {
    // Covers `spend_summary_value`'s payload SHAPING only (it just labels
    // whatever two `SpendTotals` it's handed) — it does NOT exercise the
    // call site that decides what those two values ARE. That's
    // `ai_spend_summary_call_site_keeps_today_and_window_totals_distinct`
    // below (issue #1161's C1-r2-RBA-2): this test alone would stay green
    // even if the call site collapsed both to `totals_since(window_start)`.
    let today = SpendTotals {
        input_tokens: 100,
        output_tokens: 50,
        est_cost_usd: 0.10,
    };
    let window_totals = SpendTotals {
        input_tokens: 9_000,
        output_tokens: 4_000,
        est_cost_usd: 12.0,
    };

    let out = spend_summary_value(today, window_totals, vec![], vec![], json!({}));
    assert_eq!(out["today"]["inputTokens"], 100);
    assert_eq!(out["windowTotals"]["inputTokens"], 9_000);
    assert_ne!(out["today"], out["windowTotals"]);
}

#[test]
fn spend_summary_value_passes_through_provider_thinking_and_window_untouched() {
    // The test above only pins `today`/`windowTotals`. `spend_summary_value`
    // also has to forward `perProvider`/`thinkingByModel`/`window` verbatim
    // and stamp the `thinkingByModelWindow: "allTime"` discriminator the
    // renderer switches on (`AiSpendSummary.thinkingByModelWindow`,
    // mock-client.test.ts's "fully-populated AiSpendSummary shape" test
    // pins the same keys against the TS mock) — none of that is covered
    // above, so dropping any one of those four `json!` keys stays green
    // there while failing here.
    let per_provider = vec![json!({"provider": "openai", "inputTokens": 1})];
    let thinking_by_model = vec![json!({"model": "o3-mini"})];
    let window = json!({"days": 7, "from": 1, "to": 2});

    let out = spend_summary_value(
        SpendTotals::default(),
        SpendTotals::default(),
        per_provider,
        thinking_by_model,
        window,
    );

    assert_eq!(out["perProvider"][0]["provider"], "openai");
    assert_eq!(out["thinkingByModel"][0]["model"], "o3-mini");
    assert_eq!(out["window"]["days"], 7);
    assert_eq!(out["thinkingByModelWindow"], "allTime");
}

#[test]
fn spend_summary_from_store_maps_thinking_by_model_fields_by_name() {
    // The `thinkingByModel` JSON mapping (provider/model/calls/
    // thinkingTokens/outputTokens) moved here verbatim from the old
    // inline `ai_spend_summary` body (issue #1161) but was never
    // JSON-shape tested before the move either — this pins the wire
    // field names a renderer reads by string key (`AiSpendModelThinking`),
    // so a rename/typo is silent to the type system but fails here.
    use crate::spend::{SpendRecord, SpendStore};

    let dir = TempDir::new().unwrap();
    let store = SpendStore::open(&dir.path().to_path_buf()).unwrap();
    store.record(SpendRecord {
        provider: "openai".to_string(),
        model: "o3-mini".to_string(),
        input_tokens: 100,
        output_tokens: 50,
        thinking_tokens: Some(900),
        run_id: None,
        base_url: None,
    });

    let out = spend_summary_from_store(&store, 1);
    let row = &out["thinkingByModel"][0];
    assert_eq!(row["provider"], "openai");
    assert_eq!(row["model"], "o3-mini");
    assert_eq!(row["calls"], 1);
    assert_eq!(row["thinkingTokens"], 900);
    assert_eq!(row["outputTokens"], 50);
}

#[test]
fn ai_spend_summary_call_site_keeps_today_and_window_totals_distinct() {
    // Regression for C1-r2-RBA-2: the tautology above only checks that
    // `spend_summary_value` labels its two inputs correctly — it never calls
    // the actual `ai_spend_summary` call site, so reverting mod.rs back to
    // `today = store.totals_since(window_start)` (C1-r1-RBA-1's original
    // defect) would leave the whole suite green. This drives the real call
    // site (`spend_summary_from_store`) against a real on-disk `SpendStore`
    // seeded with a row outside "today" AND a row inside "today", so a
    // collapse back onto one query fails on EITHER side (C1-r3-RBA-3): a
    // `today` that is always zero (e.g. a call site that never queries it)
    // would fail the second assertion below just as loudly as a `today`
    // that wrongly includes the 5-day-old row.
    use crate::data_store::DataStore;
    use crate::spend::SpendStore;

    let dir = TempDir::new().unwrap();
    let store = SpendStore::open(&dir.path().to_path_buf()).unwrap();

    let five_days_ago = crate::db::now_ms() - 5 * 86_400_000;
    let today_ms = crate::db::now_ms();
    store
        .import(&serde_json::json!([
            {
                "id": "spend-c1-r2-rba-2-old",
                "createdAt": five_days_ago,
                "provider": "openai",
                "model": "gpt-test",
                "inputTokens": 500,
                "outputTokens": 200,
                "estCostUsd": 3.0,
            },
            {
                "id": "spend-c1-r3-rba-3-today",
                "createdAt": today_ms,
                "provider": "openai",
                "model": "gpt-test",
                "inputTokens": 70,
                "outputTokens": 30,
                "estCostUsd": 0.5,
            },
        ]))
        .unwrap();

    let out = spend_summary_from_store(&store, 7);
    assert_eq!(
        out["today"]["inputTokens"], 70,
        "today's row must be counted, and the 5-day-old row must not be"
    );
    assert_eq!(
        out["windowTotals"]["inputTokens"], 570,
        "the 7-day window must include both rows"
    );
    assert_ne!(out["today"], out["windowTotals"]);
}

#[test]
fn ai_spend_summary_call_site_keeps_a_quiet_provider_in_per_provider() {
    // Regression for #1159 T1: the four `per_provider_with_zero_rows`
    // tests below feed hand-built `Vec<ProviderTotals>` literals, so they
    // can never see which `since_ms` the REAL call site asks the store
    // for — mutating `spend_summary_from_store`'s
    // `store.by_provider_since(0)` back to `store.by_provider_since(window_start)`
    // (the pre-#1161 defect, where a quiet provider silently vanishes
    // instead of getting a zero row) left the whole suite green before
    // this test existed. This drives the real call site against a real
    // on-disk `SpendStore` seeded with a provider active only 30 days ago
    // (outside a 7-day window) plus a different provider active today.
    use crate::data_store::DataStore;
    use crate::spend::SpendStore;

    let dir = TempDir::new().unwrap();
    let store = SpendStore::open(&dir.path().to_path_buf()).unwrap();

    let thirty_days_ago = crate::db::now_ms() - 30 * 86_400_000;
    let today_ms = crate::db::now_ms();
    store
        .import(&serde_json::json!([
            {
                "id": "spend-t1-quiet-provider",
                "createdAt": thirty_days_ago,
                "provider": "anthropic",
                "model": "claude-test",
                "inputTokens": 400,
                "outputTokens": 150,
                "estCostUsd": 2.5,
            },
            {
                "id": "spend-t1-active-provider",
                "createdAt": today_ms,
                "provider": "openai",
                "model": "gpt-test",
                "inputTokens": 70,
                "outputTokens": 30,
                "estCostUsd": 0.5,
            },
        ]))
        .unwrap();

    let out = spend_summary_from_store(&store, 7);
    let per_provider = out["perProvider"].as_array().unwrap();
    assert_eq!(
        per_provider.len(),
        2,
        "a provider quiet in the window must still be listed, not dropped"
    );
    let anthropic_row = per_provider
        .iter()
        .find(|row| row["provider"] == "anthropic")
        .expect("the 30-day-old provider must still appear");
    assert_eq!(anthropic_row["inputTokens"], 0);
    assert_eq!(anthropic_row["reason"], "no spend in window");
    let openai_row = per_provider
        .iter()
        .find(|row| row["provider"] == "openai")
        .expect("the active-today provider must appear");
    assert_eq!(openai_row["inputTokens"], 70);
    assert!(openai_row.get("reason").is_none());
}

#[test]
fn zero_summary_reuses_the_same_window_json_as_the_live_path() {
    // #1159 T2: the store-unavailable branch used to hand-rebuild
    // `window_json` inline in mod.rs instead of reusing `window_json`
    // here, so the two constructions could silently drift apart. Pin
    // `zero_summary`'s six top-level keys and its `window` shape.
    let out = zero_summary(7);
    assert_eq!(out["today"]["inputTokens"], 0);
    assert_eq!(out["today"]["outputTokens"], 0);
    assert_eq!(out["today"]["estCostUsd"], 0.0);
    assert_eq!(out["windowTotals"]["inputTokens"], 0);
    assert_eq!(out["perProvider"].as_array().unwrap().len(), 0);
    assert_eq!(out["thinkingByModel"].as_array().unwrap().len(), 0);
    assert_eq!(out["window"]["days"], 7);
    assert!(out["window"]["from"].as_u64().unwrap() <= out["window"]["to"].as_u64().unwrap());
    assert_eq!(out["thinkingByModelWindow"], "allTime");
}

#[test]
fn resolve_window_days_defaults_to_one_and_clamps_to_the_max() {
    assert_eq!(resolve_window_days(None), 1, "unset means today only");
    assert_eq!(
        resolve_window_days(Some(0)),
        1,
        "zero-day window is not valid"
    );
    assert_eq!(
        resolve_window_days(Some(7)),
        7,
        "in-range values pass through"
    );
    assert_eq!(
        resolve_window_days(Some(500)),
        crate::spend::SPEND_WINDOW_MAX_DAYS,
        "an oversized request must clamp, not report an unbounded window"
    );
    assert_eq!(
        resolve_window_days(Some(u32::MAX)),
        crate::spend::SPEND_WINDOW_MAX_DAYS,
        "u32::MAX must clamp too, never resolve to an all-time window"
    );
}

// ── per_provider_with_zero_rows (ai_spend_summary window merge, #1161) ──

fn totals(provider: &str, input: u64, output: u64, cost: f64) -> ProviderTotals {
    ProviderTotals {
        provider: provider.to_string(),
        input_tokens: input,
        output_tokens: output,
        est_cost_usd: cost,
    }
}

#[test]
fn a_provider_active_in_the_window_reports_its_windowed_totals_with_no_reason() {
    let all_time = vec![totals("openai", 10_000, 5_000, 1.5)];
    let windowed = vec![totals("openai", 10_000, 5_000, 1.5)];

    let out = per_provider_with_zero_rows(all_time, windowed);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0]["provider"], "openai");
    assert_eq!(out[0]["inputTokens"], 10_000);
    assert!(
        out[0].get("reason").is_none(),
        "an active row must not carry a reason"
    );
}

#[test]
fn a_provider_absent_from_the_window_becomes_a_zero_row_with_a_reason() {
    // Historically active (all_time), but nothing in THIS window: a paid
    // provider reads "no spend in window", a free (local) one is labelled local.
    for (provider, input, output, cost, reason) in [
        ("anthropic", 20_000, 8_000, 2.0, "no spend in window"),
        ("ollama", 50_000, 20_000, 0.0, "local — always $0"),
    ] {
        let all_time = vec![totals(provider, input, output, cost)];
        let windowed = vec![]; // nothing in the window

        let out = per_provider_with_zero_rows(all_time, windowed);
        assert_eq!(out.len(), 1, "the provider must still appear");
        assert_eq!(out[0]["provider"], provider);
        assert_eq!(out[0]["inputTokens"], 0);
        assert_eq!(out[0]["outputTokens"], 0);
        assert_eq!(out[0]["estCostUsd"], 0.0);
        assert_eq!(out[0]["reason"], reason);
    }
}

#[test]
fn the_merge_orders_active_rows_by_windowed_cost_not_all_time_cost() {
    // #1159 T4: "openai" dominates ALL-TIME cost but is quiet THIS
    // window; "anthropic" is the reverse (small all-time, top spender
    // this window). The emitted order must follow `windowed` (this
    // window's actual ranking), not `all_time` — a provider that
    // dominated last month must not outrank this window's real top
    // spender in the Settings list.
    let all_time = vec![
        totals("openai", 100_000, 50_000, 500.0), // all-time #1, but...
        totals("anthropic", 5_000, 2_000, 10.0),  // all-time #2
    ];
    let windowed = vec![
        totals("anthropic", 5_000, 2_000, 10.0), // ...this window's #1
                                                 // "openai" absent: quiet this window
    ];

    let out = per_provider_with_zero_rows(all_time, windowed);
    assert_eq!(out.len(), 2);
    assert_eq!(
        out[0]["provider"], "anthropic",
        "this window's top spender must lead, even though openai dominates all-time"
    );
    assert!(
        out[0].get("reason").is_none(),
        "the active row must not carry a reason"
    );
    assert_eq!(out[1]["provider"], "openai");
    assert_eq!(out[1]["reason"], "no spend in window");
}

#[test]
fn every_all_time_provider_survives_the_merge_even_with_an_empty_window() {
    let all_time = vec![
        totals("openai", 1, 1, 0.01),
        totals("anthropic", 2, 2, 0.02),
        totals("ollama", 3, 3, 0.0),
    ];
    let out = per_provider_with_zero_rows(all_time, vec![]);
    assert_eq!(
        out.len(),
        3,
        "no provider must be dropped by an empty window"
    );
}
