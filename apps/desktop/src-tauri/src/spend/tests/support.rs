//! Fixtures shared by the `spend` store tests.

use tempfile::TempDir;

use super::*;

pub(super) fn open_store() -> (TempDir, SpendStore) {
    let dir = TempDir::new().unwrap();
    let store = SpendStore::open(&dir.path().to_path_buf()).unwrap();
    (dir, store)
}

pub(super) fn rec(provider: &str, model: &str, input: u32, output: u32) -> SpendRecord {
    SpendRecord {
        provider: provider.to_string(),
        model: model.to_string(),
        input_tokens: input,
        output_tokens: output,
        thinking_tokens: None,
        run_id: None,
        base_url: None,
    }
}

pub(super) fn thinking_rec(
    provider: &str,
    model: &str,
    output: u32,
    thinking: Option<u32>,
) -> SpendRecord {
    SpendRecord {
        thinking_tokens: thinking,
        ..rec(provider, model, 10, output)
    }
}

/// Directly inserts a row `days_ago` days in the past — `record()` always
/// stamps `now_ms()`, so a window test that needs a call OUTSIDE today has to
/// write the row itself, same idiom as `documents::tests`'s TTL-eviction tests.
pub(super) fn insert_backdated(
    store: &SpendStore,
    days_ago: u64,
    provider: &str,
    model: &str,
    tokens: u32,
) {
    let ts = now_ms().saturating_sub(days_ago * 86_400_000);
    let cost = estimate_cost(model, tokens, tokens);
    let conn = store.conn.lock();
    conn.execute(
        "INSERT INTO ai_spend
         (id, created_at, provider, model, input_tokens, output_tokens, thinking_tokens,
          est_cost_usd, run_id)
         VALUES (?1,?2,?3,?4,?5,?6,NULL,?7,NULL)",
        params![
            format!("spend-test-{}", uuid::Uuid::new_v4()),
            ts_to_db(ts),
            provider,
            model,
            tokens,
            tokens,
            cost,
        ],
    )
    .unwrap();
}
