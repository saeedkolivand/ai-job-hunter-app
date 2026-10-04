//! Backend-owned active AI *generation* provider configuration.
//!
//! Single source of truth for which provider the app generates with, and each
//! provider's model + (OpenAI-compatible) base URL. Mirrors the backend-owned
//! [`crate::documents::EmbeddingConfig`] pattern but for chat/generation. This
//! store is the `base_url` source for EVERY generation path — `ai_generate`,
//! `generate_pipeline`, research/salary, the extension bridge's
//! `resolve_answer_assist`, and autopilot (task #16) — so none of them accept a
//! renderer-supplied `base_url`; routing comes from *here*, not the request.
//! (The now-deleted `agent_run` ("prep this application") agent loop + its
//! tools used to resolve via `Completer::from_active` here too, task #25,
//! closing the base_url-exfil path for the whole tool-calling surface — moot
//! now that surface is gone, PR-5 step 2.)
//!
//! Shape maps 1:1 to the renderer's old Zustand slice:
//! `{ activeProvider, providers: { [id]: { model, baseUrl } } }`.
//!
//! ## Why the context window lives here
//!
//! `options.num_ctx` had exactly one source: the renderer's Zustand
//! `modelLimits[model].contextWindow`, read by `provider-context.ts` and put on
//! the request by the FAST path. A staged run is started by the backend and
//! builds its own requests, so it sent `context_window: None` on every call —
//! the Settings slider silently did nothing at quality/max depth.
//!
//! The honest fix is a column here rather than a second store: this is already
//! the backend-owned answer to "what does generation route to", and `num_ctx`
//! is part of that answer. Two deliberate consequences:
//!
//! * The renderer's map is keyed by MODEL, this column by PROVIDER ROW — so it
//!   means "the window for the model in this row", written together with that
//!   model and replaced with it (see [`ProviderConfig::context_window`]).
//! * `ai_stage_overrides` carries its own column, because an override names a
//!   different model and the active provider's window would be a wrong number
//!   rather than a missing one.
//!
//! Nothing GUESSES a window. Absent stays absent all the way to the adapter,
//! where the provider's own default applies. Embeddings are untouched: that
//! path deliberately ignores `num_ctx` (`documents::embed`).
//!
//! Persistence: a single-row `active_provider` scalar (`id = 1`) plus one row per
//! configured provider in `ai_provider_config`. **Unseeded = no active provider**,
//! so generation errors "No AI provider selected" rather than silently falling
//! back — matching the no-silent-fallback invariant. Holds NO secrets (API keys
//! stay in the OS keychain), so it is safe to include in backups; a factory reset
//! must clear it (both wired in `commands/privacy.rs` + `commands/data.rs`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use parking_lot::Mutex;
use rusqlite::{params, Connection};

pub mod stage_overrides;
mod types;
mod validation;

pub use self::stage_overrides::StageOverride;
pub use self::types::{ActiveAiConfig, AiConfigSnapshot, ProviderConfig, ProviderSettingsPatch};
pub use self::validation::validate_context_window;

use crate::commands::ai_provider::ProviderId;
use crate::data_store::DataStore;
use crate::db::{now_ms, open, run_migrations, ts_to_db, Migration};
use crate::error::AppResult;

// ── Store ─────────────────────────────────────────────────────────────────────

pub struct AiConfigStore {
    /// `parking_lot::Mutex` — not reentrant. Never re-lock while a guard is held
    /// and never hold a guard across an `.await`. Every method takes/releases the
    /// lock and returns owned values, so callers (e.g. `Completer::from_active`)
    /// can snapshot the config before any await.
    conn: Mutex<Connection>,
}

impl AiConfigStore {
    /// APPEND-ONLY: `run_migrations` is position-indexed off `PRAGMA
    /// user_version`, so an existing element must never be edited or reordered
    /// — a store already at version N would skip the change entirely.
    const MIGRATIONS: &'static [Migration] = &[
        Migration {
            name: "create_ai_provider_config",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS active_provider (
                    id       INTEGER PRIMARY KEY CHECK (id = 1),
                    provider TEXT
                );
                INSERT OR IGNORE INTO active_provider (id, provider) VALUES (1, NULL);
                CREATE TABLE IF NOT EXISTS ai_provider_config (
                    provider   TEXT PRIMARY KEY,
                    model      TEXT,
                    base_url   TEXT,
                    updated_at INTEGER NOT NULL
                );",
                )
            },
        },
        Migration {
            name: "create_ai_stage_overrides",
            up: |conn| {
                // `stage` is the PRIMARY KEY, so one stage can have at most one
                // override and the table can never hold more rows than the
                // vocabulary has names. There is deliberately NO `base_url`
                // column: a stage override names a PROVIDER, and that
                // provider's row already holds the one base URL it uses, which
                // Settings displays. A per-stage copy would be a second egress
                // endpoint that no screen shows — see `StageOverride`.
                //
                // The vocabulary itself is checked in
                // CODE, not in a SQL `CHECK`: the list is generated
                // (`ipc_contracts::events::PIPELINE_STAGES`) and a CHECK
                // constraint would freeze a copy of it into every existing
                // user's database file, where adding a stage later cannot
                // reach it. Same precedent as the run-event `phase` column.
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS ai_stage_overrides (
                        stage          TEXT PRIMARY KEY,
                        provider       TEXT NOT NULL,
                        model          TEXT NOT NULL,
                        context_window INTEGER,
                        updated_at     INTEGER NOT NULL
                    );",
                )
            },
        },
        Migration {
            name: "add_ai_provider_config_context_window",
            up: |conn| {
                conn.execute_batch(
                    "ALTER TABLE ai_provider_config ADD COLUMN context_window INTEGER;",
                )
            },
        },
    ];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("ai_provider_config.db");
        let mut conn = open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // ── Reads ──────────────────────────────────────────────────────────────────

    /// The active generation provider id, or `None` when unseeded (→ generation
    /// errors "No AI provider selected", never a silent fallback).
    pub fn active_provider(&self) -> Option<String> {
        let conn = self.conn.lock();
        Self::active_provider_conn(&conn)
    }

    /// The full read model (active provider's resolved model/base_url + the
    /// providers map). Owned + lock-free to the caller, so it is safe to snapshot
    /// before an `.await`.
    pub fn active_config(&self) -> ActiveAiConfig {
        let conn = self.conn.lock();
        let active_provider = Self::active_provider_conn(&conn);
        let providers = Self::providers_conn(&conn);
        let (model, base_url, context_window) = active_provider
            .as_deref()
            .and_then(|p| providers.get(p))
            .map_or((None, None, None), |c| {
                (c.model.clone(), c.base_url.clone(), c.context_window)
            });
        ActiveAiConfig {
            active_provider,
            model,
            base_url,
            context_window,
            providers,
        }
    }

    /// The stored base URL for ONE provider — the single endpoint that provider
    /// uses, wherever it is routed from.
    ///
    /// The read behind a stage override's egress URL: an override names a
    /// provider, and this is that provider's own configured endpoint, so the
    /// two can never disagree. Only `openai-compatible` ever has one stored
    /// (`validate_settings` nulls it for the rest), which is also the only
    /// provider `resolve()` honors it for.
    pub fn provider_base_url(&self, provider: &str) -> Option<String> {
        let conn = self.conn.lock();
        Self::providers_conn(&conn)
            .get(provider)
            .and_then(|c| c.base_url.clone())
    }

    /// The export/import/seed snapshot.
    pub fn snapshot(&self) -> AiConfigSnapshot {
        let conn = self.conn.lock();
        AiConfigSnapshot {
            active_provider: Self::active_provider_conn(&conn),
            providers: Self::providers_conn(&conn),
            stage_overrides: Self::stage_overrides_conn(&conn),
        }
    }

    /// Whether anything has ever been persisted — the row-presence seed gate.
    pub fn is_seeded(&self) -> bool {
        let conn = self.conn.lock();
        Self::is_seeded_conn(&conn)
    }

    // ── Writes ─────────────────────────────────────────────────────────────────

    /// Switch the active provider (the "switch" half of the switch-vs-edit split).
    /// Validates the id is known; does NOT require the provider to be fully
    /// configured yet (generation validates model/base_url at resolve time).
    pub fn set_active_provider(&self, provider: &str) -> AppResult<()> {
        let provider_id = ProviderId::parse(provider)?;
        let conn = self.conn.lock();
        Self::set_active_conn(&conn, provider_id.as_str())
    }

    /// Edit a provider's model/base_url (the "edit" half — never flips the active
    /// provider). Server-side validation: known id, cross-family model check, and
    /// base_url provenance (scheme + cloud-metadata block).
    ///
    /// PATCH semantics, per field: absent keeps the stored value, explicit
    /// `null` clears it, a value sets it — see [`ProviderSettingsPatch`]. The
    /// merge happens under the SAME lock as the write, so two concurrent saves
    /// cannot read the same "before" and each drop the other's field.
    ///
    /// The merged result is validated as a whole, not just the changed fields:
    /// a patch that only changes the model must still be rejected if the model
    /// is wrong for the STORED base_url's provider.
    pub fn set_provider_settings(&self, patch: ProviderSettingsPatch) -> AppResult<()> {
        let provider_id = ProviderId::parse(&patch.provider)?;
        let conn = self.conn.lock();
        let stored = Self::providers_conn(&conn)
            .remove(provider_id.as_str())
            .unwrap_or_default();
        let (model, base_url, context_window) = Self::validate_settings(
            provider_id,
            patch.model.unwrap_or(stored.model),
            patch.base_url.unwrap_or(stored.base_url),
            patch.context_window.unwrap_or(stored.context_window),
        )?;
        Self::upsert_provider_conn(
            &conn,
            provider_id.as_str(),
            model.as_deref(),
            base_url.as_deref(),
            context_window,
        )
    }

    /// First-run seed from the renderer's migrated Zustand config. Row-presence
    /// gated server-side: a no-op once ANYTHING has been set, so it can never
    /// clobber a later explicit change. Lenient (never fails first run): unknown
    /// providers are skipped and an invalid base_url/model is scrubbed rather than
    /// rejected. Returns whether it actually seeded.
    pub fn seed_if_empty(&self, snapshot: &AiConfigSnapshot) -> AppResult<bool> {
        let conn = self.conn.lock();
        if Self::is_seeded_conn(&conn) {
            return Ok(false);
        }
        Self::apply_snapshot_conn(&conn, snapshot)?;
        Ok(true)
    }

    /// Clear all persisted config (factory reset / import-replace).
    pub fn clear(&self) {
        let conn = self.conn.lock();
        let _ = Self::clear_conn(&conn);
    }

    // ── Connection-bound helpers (single lock; reused by seed/import) ───────────

    fn active_provider_conn(conn: &Connection) -> Option<String> {
        conn.query_row(
            "SELECT provider FROM active_provider WHERE id = 1",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .filter(|p| !p.trim().is_empty())
    }

    fn providers_conn(conn: &Connection) -> BTreeMap<String, ProviderConfig> {
        let mut out = BTreeMap::new();
        let Ok(mut stmt) = conn
            .prepare("SELECT provider, model, base_url, context_window FROM ai_provider_config")
        else {
            return out;
        };
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ProviderConfig {
                    model: row.get::<_, Option<String>>(1)?,
                    base_url: row.get::<_, Option<String>>(2)?,
                    context_window: row.get::<_, Option<u32>>(3)?,
                },
            ))
        });
        if let Ok(rows) = rows {
            for (provider, cfg) in rows.flatten() {
                out.insert(provider, cfg);
            }
        }
        out
    }

    fn is_seeded_conn(conn: &Connection) -> bool {
        let active = Self::active_provider_conn(conn).is_some();
        // Any row in EITHER table counts: a stage override is something the
        // user set, so a first-run seed arriving afterwards must not clobber
        // it any more than it may clobber a provider row.
        let rows = |table: &str| {
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
                r.get::<_, i64>(0)
            })
            .map(|c| c > 0)
            .unwrap_or(false)
        };
        active || rows("ai_provider_config") || rows("ai_stage_overrides")
    }

    fn set_active_conn(conn: &Connection, provider: &str) -> AppResult<()> {
        conn.execute(
            "UPDATE active_provider SET provider = ?1 WHERE id = 1",
            params![provider],
        )?;
        Ok(())
    }

    fn upsert_provider_conn(
        conn: &Connection,
        provider: &str,
        model: Option<&str>,
        base_url: Option<&str>,
        context_window: Option<u32>,
    ) -> AppResult<()> {
        conn.execute(
            "INSERT INTO ai_provider_config (provider, model, base_url, context_window, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(provider) DO UPDATE SET
                model = excluded.model, base_url = excluded.base_url,
                context_window = excluded.context_window,
                updated_at = excluded.updated_at",
            params![
                provider,
                model,
                base_url,
                context_window,
                ts_to_db(now_ms())
            ],
        )?;
        Ok(())
    }

    fn clear_conn(conn: &Connection) -> AppResult<()> {
        conn.execute("DELETE FROM ai_provider_config", [])?;
        // The per-stage overrides live in this store, so a factory reset /
        // import-replace has to sweep them too — otherwise a "cleared" config
        // still routes half the pipeline at the model the old config named.
        conn.execute("DELETE FROM ai_stage_overrides", [])?;
        conn.execute(
            "UPDATE active_provider SET provider = NULL WHERE id = 1",
            [],
        )?;
        Ok(())
    }

    /// Apply a full snapshot (seed + import). Lenient by design: unknown providers
    /// are skipped and a cross-family model / bad base_url are scrubbed instead of
    /// erroring. This is the right behavior for first-run seed AND untrusted backup
    /// restore — a malicious base_url from a tampered bundle must never persist as
    /// a live egress endpoint. Returns the number of provider rows written.
    fn apply_snapshot_conn(conn: &Connection, snapshot: &AiConfigSnapshot) -> AppResult<usize> {
        let mut written = 0;
        for (provider, cfg) in &snapshot.providers {
            let Ok(provider_id) = ProviderId::parse(provider) else {
                continue;
            };
            let (model, base_url, context_window) = Self::scrub_settings(
                provider_id,
                cfg.model.clone(),
                cfg.base_url.clone(),
                cfg.context_window,
            );
            Self::upsert_provider_conn(
                conn,
                provider_id.as_str(),
                model.as_deref(),
                base_url.as_deref(),
                context_window,
            )?;
            written += 1;
        }
        written += Self::apply_stage_overrides_conn(conn, &snapshot.stage_overrides)?;
        if let Some(ap) = snapshot.active_provider.as_deref() {
            if let Ok(id) = ProviderId::parse(ap) {
                Self::set_active_conn(conn, id.as_str())?;
            }
        }
        Ok(written)
    }
}

// ── Context window ────────────────────────────────────────────────────────────

/// The bounds a stored context window must fall in, re-exported from the
/// GENERATED contract so there is one definition for the validator, the wire
/// schema and every slider that offers the value.
///
/// They used to be literals here, mirroring the renderer's preferences schema
/// by hand — which meant three copies of one rule (this pair, the stage-override
/// editor, the local-model panel) and nothing failing if they drifted. Now
/// packages/shared/src/ai-context-window.ts owns them and `pnpm gen:ipc` writes
/// this side, so `gen:ipc:check` fails CI instead.
///
/// Below the minimum a run has no room for the prompt at all
/// (`prompts::ARTIFACT_CAP` alone is 16 000 chars); above the maximum the
/// request is an out-of-memory kill rather than a size.
pub use crate::ipc_contracts::context_window::{MAX_CONTEXT_WINDOW, MIN_CONTEXT_WINDOW};

impl DataStore for AiConfigStore {
    fn key(&self) -> &'static str {
        "aiProviderConfig"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::to_value(self.snapshot()).unwrap_or_else(|_| serde_json::json!({}))
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        // Single settings object; treat null/missing as "nothing to restore".
        if data.is_null() {
            return Ok(0);
        }
        let snapshot = AiConfigSnapshot::from_untrusted(data)?;
        let conn = self.conn.lock();
        Self::clear_conn(&conn)?;
        // REPLACE semantics from an untrusted bundle → apply leniently (scrub, so a
        // tampered base_url can never be restored as a live egress endpoint).
        Self::apply_snapshot_conn(&conn, &snapshot)
    }
}

#[cfg(test)]
mod tests;
