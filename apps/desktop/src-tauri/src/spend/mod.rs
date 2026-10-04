//! AI-spend visibility: REAL per-call token usage (as reported by each
//! provider's own response — never estimated), persisted per call, converted
//! to an ESTIMATED dollar cost via a static list-price rate table.
//!
//! Clones the `ai_generations/mod.rs` SQLite-store pattern exactly: one
//! `SpendStore { conn: Mutex<Connection> }`, opened via `crate::db::open` +
//! `run_migrations`, implementing [`DataStore`] (key `"spend"`) so export/
//! import + factory reset work for free via the existing registries.
//!
//! Tokens are exact — sourced from each provider adapter's own usage fields.
//! **Covered** (every standard token-billed call, at its shared chokepoint,
//! so a new caller inherits tracking with no code change of its own):
//! - `commands::ai_provider::stream::stream_response` — every streamed
//!   generation (`ai_generate`/`generate_pipeline`'s UI-facing path).
//! - `pipeline::Completer::complete` — every non-streaming completion
//!   (autopilot notes, the résumé/cover pipeline's research-brief synthesis).
//! - `pipeline::Completer::chat_with_tools` — every agent tool-calling turn
//!   (the "Prep this application" loop's `agent_run` command; one run fanned
//!   out into several turns, plausibly the biggest paid-token consumer while
//!   it shipped). `agent_run` was deleted in PR-5 along with the rest of the
//!   agent module, so this chokepoint currently has no live caller — flagged
//!   for the AI-provider owner as a separate cleanup, not touched by PR-5.
//! - `commands::ai_provider::embed_text` — every embedding call (`ai_embed`,
//!   match-score resolution, `ai_reembed_all`'s batch re-index).
//!
//! **Explicitly NOT covered** — `AiProvider::research`/`research_salary`/
//! `research_answer`: these run through each provider's native **web-search**
//! tool (OpenAI Responses API `web_search`, Anthropic's server-side
//! `web_search_20250305` tool, Gemini's `google_search` grounding tool, or the
//! Ollama Web Search API), which is priced per-search / by the account's web-
//! search plan, not the standard per-token chat rate this module's `RATES`
//! table models. Recording them here under a token-based estimate would be a
//! confident-looking number that is actively WRONG, not just imprecise — so
//! this module makes no attempt to track them; they are honestly invisible to
//! today's spend summary rather than silently mis-costed.
//!
//! The dollar figure for what IS tracked is a best-effort list-price
//! conversion: a BYO-key user has no billing API we could query, so this can
//! never be billing-accurate — it's a ballpark, not an invoice.

use parking_lot::Mutex;
use std::path::PathBuf;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data_store::DataStore;
use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::AppResult;
use crate::observability::sanitize_reason;

mod rates;

pub use self::rates::estimate_cost;
use self::rates::{is_free_call, is_free_provider};

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
    /// Reasoning tokens, when the provider reported them as a distinct number
    /// — see [`crate::commands::ai_provider::Usage::thinking_tokens`]. `None`
    /// means "not reported", which is what most rows will be; it is NOT zero,
    /// and a reader averaging over rows must skip it rather than count it.
    ///
    /// A SUBSET of `output_tokens`, so `est_cost_usd` neither uses nor
    /// double-counts it.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub thinking_tokens: Option<u32>,
    pub est_cost_usd: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub run_id: Option<String>,
}

/// Input to [`SpendStore::record`] — the real usage a provider adapter
/// reports at one of the shared chokepoints.
pub struct SpendRecord {
    pub provider: String,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// See [`SpendRow::thinking_tokens`] — `None` unless the provider reported
    /// a distinct count.
    pub thinking_tokens: Option<u32>,
    pub run_id: Option<String>,
    /// The resolved base URL for an `openai-compatible` call, when known —
    /// used ONLY to decide the free/paid cost gate ([`is_free_call`]); never
    /// persisted (not part of the `ai_spend` schema). `None` for every
    /// non-`openai-compatible` provider, which decides purely on `provider`.
    pub base_url: Option<String>,
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

/// One model's OBSERVED reasoning overhead — the aggregate behind
/// [`SpendStore::thinking_by_model`].
///
/// Exists so a "which model should run which stage" advisor can answer from
/// what this machine actually measured instead of from a hard-coded table of
/// model reputations: a model that spends 20 tokens thinking per token of
/// answer is a bad fit for a cheap mechanical stage, and that is a fact about
/// the user's model, not about the model's name.
///
/// Rows with no reported count are EXCLUDED (see
/// [`SpendRow::thinking_tokens`]), so `calls` is how many calls actually
/// reported — the sample size the caller needs to decide whether the ratio
/// means anything yet. A model that reports nothing is simply absent, which is
/// the honest answer to "how much does it think".
#[derive(Debug, Clone, PartialEq)]
pub struct ModelThinking {
    pub provider: String,
    pub model: String,
    /// How many calls reported a distinct thinking count.
    pub calls: u64,
    pub thinking_tokens: u64,
    /// Total `output_tokens` over the SAME calls — the denominator, so a caller
    /// computing a ratio compares like with like.
    pub output_tokens: u64,
}

pub struct SpendStore {
    conn: Mutex<Connection>,
}

impl SpendStore {
    /// APPEND-ONLY: position-indexed off `PRAGMA user_version`, so an existing
    /// element must never be edited or reordered.
    const MIGRATIONS: &'static [Migration] = &[
        Migration {
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
        },
        Migration {
            name: "add_ai_spend_thinking_tokens",
            up: |conn| {
                // NULL, not `DEFAULT 0`: every row written before this column
                // existed has an UNKNOWN thinking count, and a zero default
                // would make the whole history read as "no model ever
                // reasoned". The read model below filters on NOT NULL for the
                // same reason.
                conn.execute_batch("ALTER TABLE ai_spend ADD COLUMN thinking_tokens INTEGER;")
            },
        },
    ];

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
    /// static rate table. Free calls ([`is_free_call`] — local/CLI-agent
    /// providers, or an `openai-compatible` endpoint pointed at localhost) are
    /// always $0 regardless of token volume — never fabricated.
    pub fn record(&self, rec: SpendRecord) {
        let est_cost_usd = if is_free_call(&rec.provider, rec.base_url.as_deref()) {
            0.0
        } else {
            estimate_cost(&rec.model, rec.input_tokens, rec.output_tokens)
        };
        let now = now_ms();
        let id = format!("spend-{now}-{}", &Uuid::new_v4().to_string()[..8]);
        let conn = self.conn.lock();
        // Non-propagating by design — a store failure must never break/fail
        // an AI call — but a silently-dropped row would undercut "spend
        // visibility", so at least log it.
        if let Err(e) = conn.execute(
            "INSERT INTO ai_spend
             (id, created_at, provider, model, input_tokens, output_tokens, thinking_tokens,
              est_cost_usd, run_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                id,
                ts_to_db(now),
                rec.provider,
                rec.model,
                rec.input_tokens,
                rec.output_tokens,
                rec.thinking_tokens,
                est_cost_usd,
                rec.run_id,
            ],
        ) {
            log::warn!(
                "spend: failed to record row: {}",
                sanitize_reason(&e.to_string())
            );
        }
    }

    /// Every persisted row, newest first — the `DataStore::export` payload.
    pub fn list(&self) -> Vec<SpendRow> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT id, created_at, provider, model, input_tokens, output_tokens, thinking_tokens,
                    est_cost_usd, run_id
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

    /// Real token totals + estimated cost across every provider, since
    /// `since_ms` (epoch ms) — the general form behind
    /// [`SpendStore::today_totals`] (`since_ms = today_start_ms()`) and the
    /// `days`-scoped window `ai_spend_summary` reads (issue #1161).
    pub fn totals_since(&self, since_ms: u64) -> SpendTotals {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0), COALESCE(SUM(est_cost_usd),0)
             FROM ai_spend WHERE created_at >= ?1",
            params![ts_to_db(since_ms)],
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

    /// Real token totals + estimated cost across every provider, since the
    /// start of the current UTC day.
    pub fn today_totals(&self) -> SpendTotals {
        self.totals_since(today_start_ms())
    }

    /// Real token totals + estimated cost per provider, since `since_ms`
    /// (epoch ms) — `since_ms = 0` returns ALL-TIME totals for every provider
    /// that ever recorded a call, which is how `ai_spend_summary` learns the
    /// full provider vocabulary to fill in a zero row (issue #1161). Ordered
    /// by estimated cost, highest first.
    pub fn by_provider_since(&self, since_ms: u64) -> Vec<ProviderTotals> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT provider, COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0), COALESCE(SUM(est_cost_usd),0)
             FROM ai_spend WHERE created_at >= ?1
             GROUP BY provider ORDER BY SUM(est_cost_usd) DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(params![ts_to_db(since_ms)], |row| {
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

    /// Real token totals + estimated cost per provider, since the start of
    /// the current UTC day. Ordered by estimated cost, highest first.
    pub fn by_provider_today(&self) -> Vec<ProviderTotals> {
        self.by_provider_since(today_start_ms())
    }

    /// Observed reasoning overhead per (provider, model), over ALL history —
    /// not just today.
    ///
    /// All history on purpose: this answers "how does this model behave", which
    /// does not reset at midnight, and a day-scoped window would give the
    /// advisor nothing to say on the morning of a fresh day. Ordered by
    /// thinking volume, heaviest first.
    ///
    /// Only rows that actually reported a count take part — `thinking_tokens IS
    /// NOT NULL` — so the `output_tokens` denominator covers exactly the same
    /// calls as the numerator, and a provider that reports nothing contributes
    /// no misleading zero.
    pub fn thinking_by_model(&self) -> Vec<ModelThinking> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT provider, model, COUNT(*), COALESCE(SUM(thinking_tokens),0),
                    COALESCE(SUM(output_tokens),0)
             FROM ai_spend WHERE thinking_tokens IS NOT NULL
             GROUP BY provider, model ORDER BY SUM(thinking_tokens) DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(ModelThinking {
                    provider: row.get(0)?,
                    model: row.get(1)?,
                    calls: row.get::<_, i64>(2)? as u64,
                    thinking_tokens: row.get::<_, i64>(3)? as u64,
                    output_tokens: row.get::<_, i64>(4)? as u64,
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
        thinking_tokens: row.get(6)?,
        est_cost_usd: row.get(7)?,
        run_id: row.get(8)?,
    })
}

/// Epoch-ms of the start of the current UTC day — the "today" boundary for
/// [`SpendStore::today_totals`]/[`SpendStore::by_provider_today`]. Mirrors
/// `limits::utc_day()`'s day-bucket convention.
fn today_start_ms() -> u64 {
    (now_ms() / 86_400_000) * 86_400_000
}

/// The largest `days` window `ai_spend_summary` accepts (issue #1161) —
/// generous enough for a "last quarter" glance without letting an unbounded
/// value force a full-table scan on every call.
pub const SPEND_WINDOW_MAX_DAYS: u32 = 90;

/// Epoch-ms start of a `days`-day window ending today (inclusive of today).
/// `days = 1` is exactly [`today_start_ms`] (today only, the pre-#1161
/// default), `days = 7` is the last 7 UTC-day boundaries. `days = 0` is
/// treated as 1 — there is no zero-day window.
pub fn window_start_ms(days: u32) -> u64 {
    let days = days.max(1);
    today_start_ms().saturating_sub(u64::from(days - 1) * 86_400_000)
}

/// The `perProvider` "zero row" reason for a provider present in the ledger
/// with no activity in the requested window (issue #1161). `hist` is that
/// provider's ALL-TIME totals — "not priced" only holds if the provider has
/// moved real tokens at some point without ever costing anything, which
/// distinguishes a genuinely-never-billed endpoint (e.g. an
/// `openai-compatible` gateway that has always pointed at a local server)
/// from one that is simply quiet this window.
///
/// ponytail: a provider-level label, not a per-call one — `openai-compatible`
/// can be free (localhost) on some calls and paid on others, and this
/// function can't see past its own provider's aggregate to tell them apart.
/// Good enough for a Settings-page hint; a per-call breakdown would be the
/// upgrade path if that ever matters.
pub fn zero_row_reason(hist: &ProviderTotals) -> &'static str {
    if is_free_provider(&hist.provider) {
        "local — always $0"
    } else if hist.est_cost_usd == 0.0 && (hist.input_tokens > 0 || hist.output_tokens > 0) {
        "not priced"
    } else {
        "no spend in window"
    }
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
                 (id, created_at, provider, model, input_tokens, output_tokens, thinking_tokens,
                  est_cost_usd, run_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    row.id,
                    ts_to_db(row.created_at),
                    row.provider,
                    row.model,
                    row.input_tokens,
                    row.output_tokens,
                    row.thinking_tokens,
                    row.est_cost_usd,
                    row.run_id,
                ],
            )?;
        }
        tx.commit()?;
        Ok(rows.len())
    }
}

#[cfg(test)]
mod tests;
