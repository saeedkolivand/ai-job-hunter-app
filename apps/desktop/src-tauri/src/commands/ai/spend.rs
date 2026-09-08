//! AI-spend visibility (issue #1161) — the pure/`AppHandle`-free half of
//! `ai_spend_summary` (the `#[tauri::command]` itself stays in `mod.rs`, same
//! as every other command in this module — only its helpers move), split out
//! purely for R8 (the 1400-LOC hard cap). Same shape as
//! `commands::match_resume`'s `constraints` split.

use serde_json::{json, Value};

/// The real body of [`super::ai_spend_summary`] once a
/// [`crate::spend::SpendStore`] is in hand — pulled out of the
/// `#[tauri::command]` fn (which needs a live `AppHandle` this crate has no
/// mock harness for) so the call site itself is unit-testable against a real
/// on-disk store, not just hand-built [`crate::spend::SpendTotals`] literals
/// fed straight to [`spend_summary_value`] (issue #1161's C1-r2-RBA-2: that
/// shape-only test cannot catch a call site that collapses `today` and
/// `windowTotals` back onto the same query).
pub(super) fn spend_summary_from_store(store: &crate::spend::SpendStore, days: u32) -> Value {
    let window_start = crate::spend::window_start_ms(days);
    let window_json = window_json(days);
    let today = store.today_totals();
    let window_totals = store.totals_since(window_start);
    // Every provider that has EVER recorded a call (since_ms = 0), so a
    // provider with no activity in THIS window still appears — as a zero row
    // with a reason — rather than silently vanishing from the list.
    let per_provider = per_provider_with_zero_rows(
        store.by_provider_since(0),
        store.by_provider_since(window_start),
    );
    // Observed reasoning overhead per model, over all history — the honest
    // input to "which model should run which stage". EMPTY until a provider
    // that reports a distinct thinking count has actually been used (OpenAI's
    // reasoning models, Gemini's thinking models); Anthropic and Ollama fold
    // thinking into their output count and so contribute nothing here rather
    // than a zero that would read as "this model does not reason".
    let thinking_by_model: Vec<Value> = store
        .thinking_by_model()
        .into_iter()
        .map(|m| {
            json!({
                "provider": m.provider,
                "model": m.model,
                "calls": m.calls,
                "thinkingTokens": m.thinking_tokens,
                "outputTokens": m.output_tokens,
            })
        })
        .collect();
    spend_summary_value(
        today,
        window_totals,
        per_provider,
        thinking_by_model,
        window_json,
    )
}

/// Builds the `window` payload key (`{days, from, to}`) shared by
/// [`spend_summary_from_store`] and [`zero_summary`] (issue #1159 T2) — a
/// single construction so the real path and the store-unavailable
/// degradation path can't silently drift apart.
fn window_json(days: u32) -> Value {
    json!({
        "days": days,
        "from": crate::spend::window_start_ms(days),
        "to": crate::db::now_ms(),
    })
}

/// The degraded [`super::ai_spend_summary`] payload for when no
/// [`crate::spend::SpendStore`] is available in app state (failed to open at
/// startup) — all-zero totals and empty lists, but a real `window` built
/// from the SAME [`window_json`] the live path uses (issue #1159 T2: this
/// branch used to hand-rebuild that JSON inline, with nothing pinning the
/// two constructions together).
pub(super) fn zero_summary(days: u32) -> Value {
    let zero = crate::spend::SpendTotals::default();
    spend_summary_value(zero, zero, vec![], vec![], window_json(days))
}

/// Resolves `ai_spend_summary`'s `days` argument (issue #1161): unset means
/// "today only" (`1`, the pre-#1161 default), and the result is clamped to
/// [`crate::spend::SPEND_WINDOW_MAX_DAYS`] so an unbounded value can never
/// force a full-table scan or report a `window.days` the store didn't
/// actually query for.
pub(super) fn resolve_window_days(days: Option<u32>) -> u32 {
    days.unwrap_or(1)
        .clamp(1, crate::spend::SPEND_WINDOW_MAX_DAYS)
}

/// Assembles the [`super::ai_spend_summary`] payload — pulled out of the
/// `#[tauri::command]` fn so it is unit testable without a live `AppHandle`
/// (this crate has no mock harness for one). `today` and `window_totals` are
/// two DIFFERENT [`crate::spend::SpendTotals`] values (`today_totals()` vs
/// `totals_since(window_start)`, issue #1161's C1-r1-RBA-1) — they only carry
/// the same number when `days == 1`, where the two windows coincide.
pub(super) fn spend_summary_value(
    today: crate::spend::SpendTotals,
    window_totals: crate::spend::SpendTotals,
    per_provider: Vec<Value>,
    thinking_by_model: Vec<Value>,
    window_json: Value,
) -> Value {
    json!({
        "today": spend_totals_json(today),
        "windowTotals": spend_totals_json(window_totals),
        "perProvider": per_provider,
        "thinkingByModel": thinking_by_model,
        "window": window_json,
        "thinkingByModelWindow": "allTime",
    })
}

/// The `{inputTokens, outputTokens, estCostUsd}` shape used for both `today`
/// and `windowTotals` in [`spend_summary_value`].
fn spend_totals_json(t: crate::spend::SpendTotals) -> Value {
    json!({
        "inputTokens": t.input_tokens,
        "outputTokens": t.output_tokens,
        "estCostUsd": t.est_cost_usd,
    })
}

/// `ai_spend_summary`'s `perProvider` merge: every provider in `all_time`
/// (the full ledger vocabulary) shows its `windowed` totals when present,
/// else a zero row with [`crate::spend::zero_row_reason`]. Pulled out as a
/// pure function so the merge is unit-testable without a live `AppHandle`
/// (this crate has no mock harness for one).
///
/// Row order (issue #1159 T4): active providers come first, in `windowed`'s
/// own order — `SpendStore::by_provider_since`'s "highest estimated cost
/// first", scoped to the REQUESTED window — followed by the quiet/zero-row
/// providers in `all_time` order. Before this, the merge iterated `all_time`
/// (ordered by ALL-HISTORY cost) and substituted windowed values in place,
/// so a provider that dominated last month could outrank this window's
/// actual top spender in the Settings list.
fn per_provider_with_zero_rows(
    all_time: Vec<crate::spend::ProviderTotals>,
    windowed: Vec<crate::spend::ProviderTotals>,
) -> Vec<Value> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<Value> = windowed
        .into_iter()
        .map(|p| {
            seen.insert(p.provider.clone());
            json!({
                "provider": p.provider,
                "inputTokens": p.input_tokens,
                "outputTokens": p.output_tokens,
                "estCostUsd": p.est_cost_usd,
            })
        })
        .collect();
    out.extend(
        all_time
            .into_iter()
            .filter(|hist| !seen.contains(&hist.provider))
            .map(|hist| {
                json!({
                    "provider": hist.provider,
                    "inputTokens": 0,
                    "outputTokens": 0,
                    "estCostUsd": 0.0,
                    "reason": crate::spend::zero_row_reason(&hist),
                })
            }),
    );
    out
}

#[cfg(test)]
mod test {
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
        // Historically active (all_time), but nothing in THIS window.
        let all_time = vec![totals("anthropic", 20_000, 8_000, 2.0)];
        let windowed = vec![]; // nothing in the window

        let out = per_provider_with_zero_rows(all_time, windowed);
        assert_eq!(out.len(), 1, "the provider must still appear");
        assert_eq!(out[0]["provider"], "anthropic");
        assert_eq!(out[0]["inputTokens"], 0);
        assert_eq!(out[0]["outputTokens"], 0);
        assert_eq!(out[0]["estCostUsd"], 0.0);
        assert_eq!(out[0]["reason"], "no spend in window");
    }

    #[test]
    fn a_free_provider_absent_from_the_window_is_labelled_local() {
        let all_time = vec![totals("ollama", 50_000, 20_000, 0.0)];
        let out = per_provider_with_zero_rows(all_time, vec![]);
        assert_eq!(out[0]["reason"], "local — always $0");
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
}
