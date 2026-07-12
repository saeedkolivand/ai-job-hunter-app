//! AI-spend visibility: REAL per-call token usage (as reported by each
//! provider's own response — never estimated), persisted per call, converted
//! to an ESTIMATED dollar cost via a static list-price rate table.
//!
//! Clones the `ai_generations/mod.rs` SQLite-store pattern exactly: one
//! `SpendStore { conn: Mutex<Connection> }`, opened via `crate::db::open` +
//! `run_migrations`, implementing [`DataStore`] (key `"spend"`) so export/
//! import + factory reset work for free via the existing registries.
//!
//! Tokens are exact — sourced from each provider adapter's own usage fields
//! (see `commands::ai_provider::stream` and `pipeline::Completer::complete`,
//! the two shared chokepoints that call [`record_usage`]). The dollar figure
//! is a best-effort list-price conversion: a BYO-key user has no billing API
//! we could query, so this can never be billing-accurate — it's a ballpark,
//! not an invoice.

use parking_lot::Mutex;
use std::path::PathBuf;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::data_store::DataStore;
use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::AppResult;

/// One persisted call's real usage + estimated cost (the `DataStore::export`
/// shape).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpendRow {
    pub id: String,
    pub created_at: u64,
    pub provider: String,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub est_cost_usd: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub run_id: Option<String>,
}

/// Input to [`SpendStore::record`] — the real usage a provider adapter
/// reports at one of the two shared chokepoints.
pub struct SpendRecord {
    pub provider: String,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub run_id: Option<String>,
}

/// Aggregate totals over a set of calls.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SpendTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub est_cost_usd: f64,
}

/// Per-provider totals — one row of [`SpendStore::by_provider_today`].
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderTotals {
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub est_cost_usd: f64,
}

pub struct SpendStore {
    conn: Mutex<Connection>,
}

impl SpendStore {
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_ai_spend",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS ai_spend (
                    id            TEXT PRIMARY KEY,
                    created_at    INTEGER NOT NULL,
                    provider      TEXT NOT NULL,
                    model         TEXT NOT NULL,
                    input_tokens  INTEGER NOT NULL DEFAULT 0,
                    output_tokens INTEGER NOT NULL DEFAULT 0,
                    est_cost_usd  REAL NOT NULL DEFAULT 0,
                    run_id        TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_ai_spend_created_at ON ai_spend(created_at);
                CREATE INDEX IF NOT EXISTS idx_ai_spend_provider ON ai_spend(provider);",
            )
        },
    }];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("ai_spend.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM ai_spend", []).ok();
    }

    /// Persist one call's real usage, computing its estimated cost from the
    /// static rate table. Local/CLI-agent providers ([`is_free_provider`])
    /// are always $0 regardless of token volume — never fabricated.
    pub fn record(&self, rec: SpendRecord) {
        let est_cost_usd = if is_free_provider(&rec.provider) {
            0.0
        } else {
            estimate_cost(&rec.model, rec.input_tokens, rec.output_tokens)
        };
        let id = format!("spend-{}-{}", now_ms(), &Uuid::new_v4().to_string()[..8]);
        let conn = self.conn.lock();
        let _ = conn.execute(
            "INSERT INTO ai_spend
             (id, created_at, provider, model, input_tokens, output_tokens, est_cost_usd, run_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                id,
                ts_to_db(now_ms()),
                rec.provider,
                rec.model,
                rec.input_tokens,
                rec.output_tokens,
                est_cost_usd,
                rec.run_id,
            ],
        );
    }

    /// Every persisted row, newest first — the `DataStore::export` payload.
    pub fn list(&self) -> Vec<SpendRow> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT id, created_at, provider, model, input_tokens, output_tokens, est_cost_usd, run_id
             FROM ai_spend ORDER BY created_at DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], row_to_spend_row)
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    /// Real token totals + estimated cost across every provider, since the
    /// start of the current UTC day.
    pub fn today_totals(&self) -> SpendTotals {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0), COALESCE(SUM(est_cost_usd),0)
             FROM ai_spend WHERE created_at >= ?1",
            params![ts_to_db(today_start_ms())],
            |row| {
                Ok(SpendTotals {
                    input_tokens: row.get::<_, i64>(0)? as u64,
                    output_tokens: row.get::<_, i64>(1)? as u64,
                    est_cost_usd: row.get(2)?,
                })
            },
        )
        .unwrap_or_default()
    }

    /// Real token totals + estimated cost per provider, since the start of
    /// the current UTC day. Ordered by estimated cost, highest first.
    pub fn by_provider_today(&self) -> Vec<ProviderTotals> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT provider, COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0), COALESCE(SUM(est_cost_usd),0)
             FROM ai_spend WHERE created_at >= ?1
             GROUP BY provider ORDER BY SUM(est_cost_usd) DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(params![ts_to_db(today_start_ms())], |row| {
                Ok(ProviderTotals {
                    provider: row.get(0)?,
                    input_tokens: row.get::<_, i64>(1)? as u64,
                    output_tokens: row.get::<_, i64>(2)? as u64,
                    est_cost_usd: row.get(3)?,
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }
}

fn row_to_spend_row(row: &rusqlite::Row) -> rusqlite::Result<SpendRow> {
    Ok(SpendRow {
        id: row.get(0)?,
        created_at: ts_from_db(row.get::<_, i64>(1)?),
        provider: row.get(2)?,
        model: row.get(3)?,
        input_tokens: row.get(4)?,
        output_tokens: row.get(5)?,
        est_cost_usd: row.get(6)?,
        run_id: row.get(7)?,
    })
}

/// Epoch-ms of the start of the current UTC day — the "today" boundary for
/// [`SpendStore::today_totals`]/[`SpendStore::by_provider_today`]. Mirrors
/// `limits::utc_day()`'s day-bucket convention.
fn today_start_ms() -> u64 {
    (now_ms() / 86_400_000) * 86_400_000
}

impl DataStore for SpendStore {
    fn key(&self) -> &'static str {
        "spend"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::json!(self.list())
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        let items = data.as_array().ok_or("spend: expected an array")?;
        let rows: Vec<SpendRow> = items
            .iter()
            .map(|item| serde_json::from_value(item.clone()).map_err(crate::error::AppError::from))
            .collect::<AppResult<_>>()?;

        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        tx.execute("DELETE FROM ai_spend", [])?;
        for row in &rows {
            tx.execute(
                "INSERT INTO ai_spend
                 (id, created_at, provider, model, input_tokens, output_tokens, est_cost_usd, run_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    row.id,
                    ts_to_db(row.created_at),
                    row.provider,
                    row.model,
                    row.input_tokens,
                    row.output_tokens,
                    row.est_cost_usd,
                    row.run_id,
                ],
            )?;
        }
        tx.commit()?;
        Ok(rows.len())
    }
}

// ── App-managed convenience ─────────────────────────────────────────────────

/// Record one AI call's REAL token usage against today's spend via the
/// managed [`SpendStore`], if one is present. Best-effort: spend tracking
/// never blocks or fails a generation — a missing store (e.g. it failed to
/// open at startup) is silently skipped, exactly like the other
/// `try_state`-gated convenience writers in this crate (see
/// `commands::notifications::push_and_notify`).
pub fn record_usage(
    app: &AppHandle,
    provider: &str,
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
) {
    if let Some(store) = app.try_state::<SpendStore>() {
        store.record(SpendRecord {
            provider: provider.to_string(),
            model: model.to_string(),
            input_tokens,
            output_tokens,
            run_id: None,
        });
    }
}

// ── Static rate table ───────────────────────────────────────────────────────

/// Providers with no metered API — local inference (Ollama) or a CLI agent
/// authenticated via the user's own tool login (Claude Code/Codex/Gemini
/// CLI/Antigravity). Always $0 real cost regardless of token volume — never
/// estimated. Deliberately excludes `ollama-cloud` (a paid hosted service).
fn is_free_provider(provider: &str) -> bool {
    matches!(
        provider,
        "ollama" | "claude-code" | "codex" | "gemini-cli" | "antigravity"
    )
}

/// `(model-name-PREFIX, input $/1M tokens, output $/1M tokens)`. Matched by
/// prefix (case-insensitive) so date-suffixed snapshots (`gpt-4o-2024-08-06`)
/// still match their family. **Order matters**: a more specific prefix (e.g.
/// `gpt-4o-mini`) must precede a shorter prefix it also satisfies (`gpt-4o`),
/// since the first match wins. Approximate list prices as of 2026 — a
/// best-effort ballpark, not a billing-accurate source (see module docs).
const RATES: &[(&str, f64, f64)] = &[
    // OpenAI
    ("gpt-4o-mini", 0.15, 0.60),
    ("gpt-4o", 2.50, 10.00),
    ("gpt-4.1-mini", 0.40, 1.60),
    ("gpt-4.1-nano", 0.10, 0.40),
    ("gpt-4.1", 2.00, 8.00),
    ("gpt-4-turbo", 10.00, 30.00),
    ("gpt-4", 30.00, 60.00),
    ("gpt-3.5", 0.50, 1.50),
    ("o1-mini", 1.10, 4.40),
    ("o1", 15.00, 60.00),
    ("o3-mini", 1.10, 4.40),
    ("o3", 2.00, 8.00),
    ("o4-mini", 1.10, 4.40),
    // Anthropic
    ("claude-3-5-haiku", 0.80, 4.00),
    ("claude-3-haiku", 0.25, 1.25),
    ("claude-haiku-4", 1.00, 5.00),
    ("claude-opus-4", 15.00, 75.00),
    ("claude-3-opus", 15.00, 75.00),
    ("claude-sonnet-4", 3.00, 15.00),
    ("claude-3-7-sonnet", 3.00, 15.00),
    ("claude-3-5-sonnet", 3.00, 15.00),
    ("claude-3-sonnet", 3.00, 15.00),
    // Gemini
    ("gemini-2.5-pro", 1.25, 10.00),
    ("gemini-2.5-flash-lite", 0.10, 0.40),
    ("gemini-2.5-flash", 0.30, 2.50),
    ("gemini-2.0-flash-lite", 0.075, 0.30),
    ("gemini-2.0-flash", 0.10, 0.40),
    ("gemini-1.5-flash", 0.075, 0.30),
    ("gemini-1.5-pro", 1.25, 5.00),
];

/// Conservative default for a cloud model this table doesn't recognize (a
/// mid-tier price point) — so an unknown-but-paid model never silently shows
/// $0, which would look like a free/local call. New models therefore need no
/// code change to show *some* estimate; the table can be tightened later.
const DEFAULT_RATE: (f64, f64) = (3.00, 15.00);

/// Estimated USD cost for one call, from the static [`RATES`] table (or
/// [`DEFAULT_RATE`] for an unrecognized model). Pure — callers gate local/
/// CLI-agent providers to $0 via [`is_free_provider`] before calling this, so
/// this function never needs to know about providers at all.
pub fn estimate_cost(model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
    let m = model.to_ascii_lowercase();
    let (in_rate, out_rate) = RATES
        .iter()
        .find(|(prefix, _, _)| m.starts_with(prefix))
        .map(|(_, i, o)| (*i, *o))
        .unwrap_or(DEFAULT_RATE);
    (f64::from(input_tokens) / 1_000_000.0) * in_rate
        + (f64::from(output_tokens) / 1_000_000.0) * out_rate
}

#[cfg(test)]
mod test;
