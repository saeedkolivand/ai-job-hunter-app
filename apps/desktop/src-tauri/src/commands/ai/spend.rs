//! AI-spend visibility (issue #1161) — the pure/`AppHandle`-free half of
//! `ai_spend_summary` (the `#[tauri::command]` itself stays in `mod.rs` — only its
//! helpers move), split out
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
mod tests;
