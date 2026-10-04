use super::{support::*, *};

// ── Reported thinking tokens ─────────────────────────────────────────────────

/// The column round-trips, and — the part that matters — "not reported"
/// survives as `None` instead of collapsing to a zero that would read as "this
/// model did no reasoning".
///
/// Mutation check (executed): give the migration `DEFAULT 0` and store `0`
/// instead of `NULL`, and the second assertion fails.
#[test]
fn a_reported_thinking_count_round_trips_and_an_unreported_one_stays_absent() {
    let (_dir, store) = open_store();
    store.record(thinking_rec("openai", "o3-mini", 1_000, Some(800)));
    store.record(thinking_rec("ollama", "gpt-oss:20b", 1_000, None));

    let rows = store.list();
    let reported = rows.iter().find(|r| r.model == "o3-mini").unwrap();
    let unreported = rows.iter().find(|r| r.model == "gpt-oss:20b").unwrap();
    assert_eq!(reported.thinking_tokens, Some(800));
    assert_eq!(
        unreported.thinking_tokens, None,
        "a provider that reports no distinct count must not be recorded as zero",
    );
}

/// The advisor's read model: per (provider, model) totals over the calls that
/// ACTUALLY reported, so the ratio's denominator covers the same calls as its
/// numerator.
///
/// Mutation check (executed): drop the `WHERE thinking_tokens IS NOT NULL` and
/// the unreported model appears with a 0 ratio; sum `output_tokens` over all
/// rows instead and the denominator assertion fails.
#[test]
fn thinking_by_model_reports_only_measured_calls() {
    let (_dir, store) = open_store();
    store.record(thinking_rec("openai", "o3-mini", 100, Some(900)));
    store.record(thinking_rec("openai", "o3-mini", 100, Some(1_100)));
    // Same model, a call that reported nothing — must not dilute the average.
    store.record(thinking_rec("openai", "o3-mini", 5_000, None));
    // A provider that never reports is absent entirely, not a zero row.
    store.record(thinking_rec("anthropic", "claude-sonnet-5", 2_000, None));

    let by_model = store.thinking_by_model();
    assert_eq!(
        by_model.len(),
        1,
        "only measured models appear: {by_model:?}"
    );
    let row = &by_model[0];
    assert_eq!(row.model, "o3-mini");
    assert_eq!(row.calls, 2);
    assert_eq!(row.thinking_tokens, 2_000);
    assert_eq!(
        row.output_tokens, 200,
        "the denominator must cover exactly the calls that reported",
    );
}

/// A backup taken before the column existed still restores, and a row with a
/// reported count keeps it across the round trip.
#[test]
fn thinking_tokens_survive_export_import_and_a_legacy_bundle_still_imports() {
    let (_dir, store) = open_store();
    store.record(thinking_rec("gemini", "gemini-3.5-flash", 500, Some(400)));

    let bundle = store.export();
    let (_dir2, restored) = open_store();
    restored.import(&bundle).unwrap();
    assert_eq!(restored.list()[0].thinking_tokens, Some(400));

    let legacy = serde_json::json!([{
        "id": "spend-1", "createdAt": 1_700_000_000_000_u64, "provider": "ollama",
        "model": "llama3", "inputTokens": 5, "outputTokens": 6, "estCostUsd": 0.0,
    }]);
    restored
        .import(&legacy)
        .expect("a pre-column bundle still restores");
    assert_eq!(restored.list()[0].thinking_tokens, None);
}
