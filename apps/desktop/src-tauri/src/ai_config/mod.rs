//! Backend-owned active AI *generation* provider configuration.
//!
//! Single source of truth for which provider the app generates with, and each
//! provider's model + (OpenAI-compatible) base URL. Mirrors the backend-owned
//! [`crate::documents::EmbeddingConfig`] pattern but for chat/generation. This
//! store is the `base_url` source for EVERY generation path — `ai_generate`,
//! `generate_pipeline`, research/salary, the extension bridge's
//! `resolve_answer_assist`, autopilot (task #16), and the `agent_run` ("prep this
//! application") agent loop + its tools (task #25) — so none of them accept a
//! renderer-supplied `base_url`; routing comes from *here*, not the request. The
//! `agent_run` path now resolves via `Completer::from_active` for both the agent's
//! own turns and every tool provider call (`agent::tools::complete_trusted`), and
//! its `ToolContext` no longer carries provider/model/base_url — closing the last
//! base_url-exfil path in this class (`docs/NEXT_ISSUES.md`).
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
use serde::{Deserialize, Deserializer, Serialize};

pub mod stage_overrides;

pub use self::stage_overrides::StageOverride;

use crate::commands::ai_provider::ProviderId;
use crate::data_store::DataStore;
use crate::db::{now_ms, open, run_migrations, ts_to_db, Migration};
use crate::error::AppResult;

// ── Types ─────────────────────────────────────────────────────────────────────

/// One provider's persisted generation settings. `base_url` is only meaningful
/// for `openai-compatible`; `model` is empty/absent for a not-yet-configured
/// provider (and legitimately empty for CLI agents, which use their own default).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// The context window (`num_ctx`) to run **`model`** with, when the user
    /// configured one. Belongs to the model in THIS row, not to the provider:
    /// the renderer's own limits map is keyed by model, so the two move
    /// together — `set_provider_settings` replaces both or neither, and a row
    /// whose model changed without a new window is a row with no window.
    ///
    /// Only Ollama reads it (`options.num_ctx`); every other adapter ignores
    /// it, which is why it is stored rather than gated per provider — a user
    /// who switches provider and back keeps the value they set.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_context_window"
    )]
    pub context_window: Option<u32>,
}

/// The persisted snapshot — the export/import/seed shape (`{ activeProvider,
/// providers }`), 1:1 with the renderer's old Zustand `aiProviderConfig`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfigSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_provider: Option<String>,
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderConfig>,
    /// Per-stage model overrides, keyed by the generated stage vocabulary. A
    /// defaulted field so a bundle (or a first-run renderer seed) written
    /// before overrides existed still deserializes, and an empty map is
    /// omitted from the export entirely.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stage_overrides: BTreeMap<String, StageOverride>,
}

impl AiConfigSnapshot {
    /// Parse a RESTORE bundle's `aiProviderConfig` section entry by entry, so a
    /// single unparseable entry costs only itself.
    ///
    /// A plain `from_value::<AiConfigSnapshot>` is all-or-nothing, and that is
    /// the wrong shape for untrusted input: a probe of five hand-editable
    /// mistakes (`-1`, `2^32`, a missing `provider`, a non-object entry, a
    /// wrong-typed `model`) showed EVERY one aborting the whole section —
    /// active provider `None`, providers 0, overrides 0, with the valid rows in
    /// the same bundle lost too. Field-level leniency
    /// ([`lenient_context_window`], `StageOverride::provider`) fixes the rows
    /// whose FIELDS are recoverable; this fixes the ones whose SHAPE is not, by
    /// dropping the entry rather than its siblings.
    ///
    /// Only the section itself being unparseable is still an error: that is a
    /// corrupt bundle, not one bad row, and reporting it beats silently
    /// restoring nothing.
    fn from_untrusted(data: &serde_json::Value) -> AppResult<Self> {
        let obj = data.as_object().ok_or_else(|| {
            crate::error::AppError::Parse(
                "the aiProviderConfig section is not an object".to_string(),
            )
        })?;
        // A wrong-typed `activeProvider` is dropped rather than fatal — the
        // store simply reads as unseeded, which is a state it already handles.
        let active_provider = obj
            .get("activeProvider")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        Ok(Self {
            active_provider,
            providers: parse_entries(obj.get("providers")),
            stage_overrides: parse_entries(obj.get("stageOverrides")),
        })
    }
}

/// One `{ key: entry }` map, parsed per entry — an entry that will not
/// deserialize is dropped, never the map. Shared by both of the snapshot's
/// maps because both take the same untrusted input.
fn parse_entries<T: serde::de::DeserializeOwned>(
    value: Option<&serde_json::Value>,
) -> BTreeMap<String, T> {
    value
        .and_then(serde_json::Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| {
                    serde_json::from_value::<T>(v.clone())
                        .ok()
                        .map(|parsed| (k.clone(), parsed))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The read model returned to the renderer: the active provider's own resolved
/// `model`/`baseUrl` (the convenience `useGenerateConfig` reads) plus the full
/// `providers` map (for the Settings AI tab). `activeProvider`/`model`/`baseUrl`
/// are all absent when unseeded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveAiConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// The active provider row's [`ProviderConfig::context_window`] — the
    /// window `model` is configured to run with, or absent when the user never
    /// set one (in which case the provider keeps its own default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    pub providers: BTreeMap<String, ProviderConfig>,
}

/// A PATCH to one provider's settings — the shape the settings writer takes.
///
/// Per field: **absent = keep what is stored**, explicit `null` = clear, a value
/// = set. Replace-everything semantics were the first design and they failed on
/// first contact: three renderer call sites each saved one field and silently
/// erased the other two, and a doc comment saying "send them all" is not a
/// mechanism. Absence is what a caller produces by accident, so absence has to
/// be the harmless answer.
///
/// Hand-written rather than emitted by `pnpm gen:ipc`: the whole point is the
/// `Option<Option<T>>` + `deserialize_with` pair below, which the generator has
/// no way to express. The TS counterpart is
/// `AiContract.setProviderSettings` (`field?: T | null`) — keep the two in step
/// by hand, and prefer adding a field HERE first so the compiler catches the
/// store side.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsPatch {
    pub provider: String,
    #[serde(default, deserialize_with = "double_option")]
    pub model: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub base_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub context_window: Option<Option<u32>>,
}

/// Distinguish "the key was absent" (`None`) from "the key was present and
/// null" (`Some(None)`).
///
/// Needed because a plain `Option<Option<T>>` collapses both to `None`: serde's
/// `deserialize_option` visits `none` for a missing key AND for an explicit
/// null. This is also why the command takes a struct rather than loose
/// arguments — a Tauri command parameter has no serde attributes to hang this
/// on.
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

/// Read a stored context window WITHOUT letting a bad one fail its row.
///
/// The derived `Option<u32>` rejects `-1` and `2^32` at PARSE time, which in a
/// restore is not a per-field failure at all: it aborts the whole
/// `aiProviderConfig` section, so a bundle with one hand-edited number restores
/// no providers, no active provider, and no overrides. That contradicts what
/// both scrub paths promise — an out-of-range value costs the row its window,
/// not its existence, and one bad entry never fails the restore wholesale.
///
/// So anything a `u32` cannot represent reads as `None` (= the provider's own
/// default) and the row survives to be validated normally. `Value` rather than
/// `i64` so a float, a string or a null is tolerated too — every shape, not
/// just the two that were reported.
///
/// REPRESENTABILITY only. Whether an in-range-looking number is actually
/// allowed stays with the scrub the apply path already runs
/// (`scrub_settings` / `apply_stage_overrides_conn`), so the bound lives in one
/// place: re-checking it here passed every test with the check deleted, which
/// is what a second owner of the same rule looks like. This is the LENIENT side
/// of the split `scrub_settings` makes — the interactive writer still errors,
/// because there a human typed the number and can be told it was refused.
fn lenient_context_window<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(raw
        .as_ref()
        .and_then(serde_json::Value::as_u64)
        .and_then(|v| u32::try_from(v).ok()))
}

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

    /// Strict validation used by the interactive writer: a cross-family model or a
    /// bad base_url is a hard error (surfaced to the user in Settings). Trims and
    /// drops empty strings so an empty model/base_url stores as NULL. An empty
    /// model is allowed here (a valid intermediate settings state, and legitimate
    /// for CLI agents) — the "no model selected" rule is enforced at generation
    /// resolve time (`Completer::from_active`), not at settings-write time.
    fn validate_settings(
        provider_id: ProviderId,
        model: Option<String>,
        base_url: Option<String>,
        context_window: Option<u32>,
    ) -> AppResult<(Option<String>, Option<String>, Option<u32>)> {
        let model = model
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty());
        if let Some(ref m) = model {
            provider_id.validate_model(m)?;
        }
        let context_window = validate_context_window(context_window)?;
        // `base_url` is only meaningful for `OpenAiCompatible` — `resolve()`
        // ignores it for every other provider. It's inert for egress there, but
        // a stored value still reaches `record_usage`'s free/paid cost gate, so
        // drop it to NULL for any other provider rather than persist dead data
        // that could nudge cost classification.
        let base_url = if matches!(provider_id, ProviderId::OpenAiCompatible) {
            base_url
                .map(|u| u.trim().to_string())
                .filter(|u| !u.is_empty())
        } else {
            None
        };
        if let Some(ref u) = base_url {
            crate::net::ssrf::validate_provider_base_url(u)?;
        }
        Ok((model, base_url, context_window))
    }

    /// Lenient sibling of [`Self::validate_settings`] for seed/import: drop a
    /// cross-family model and a bad base_url instead of erroring, so a first-run
    /// seed or a restore never fails on one bad field.
    fn scrub_settings(
        provider_id: ProviderId,
        model: Option<String>,
        base_url: Option<String>,
        context_window: Option<u32>,
    ) -> (Option<String>, Option<String>, Option<u32>) {
        let model = model
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .filter(|m| provider_id.validate_model(m).is_ok());
        // Same non-`OpenAiCompatible` guard as `validate_settings` — a
        // native-provider base_url from a first-run renderer seed or a restored
        // backup bundle is inert for egress but still reaches `record_usage`'s
        // free/paid cost gate, so drop it to NULL rather than persist it.
        let base_url = if matches!(provider_id, ProviderId::OpenAiCompatible) {
            base_url
                .map(|u| u.trim().to_string())
                .filter(|u| !u.is_empty())
                .filter(|u| crate::net::ssrf::validate_provider_base_url(u).is_ok())
        } else {
            None
        };
        // Out-of-range → dropped, never clamped: a clamp would silently invent
        // a window the user never chose, and the provider's own default is the
        // honest answer to "we don't know".
        let context_window = context_window.filter(|c| validate_context_window(Some(*c)).is_ok());
        (model, base_url, context_window)
    }
}

// ── Context window ────────────────────────────────────────────────────────────

/// Narrowest context window a stored setting may name. Below this a run has no
/// room for the prompt at all (`prompts::ARTIFACT_CAP` alone is 16 000 chars).
/// Mirrors `LocalModelLimitsSchema` in the renderer's preferences schema — the
/// SAME bounds the Settings slider already enforces, re-checked here because
/// this value now arrives from IPC and from restored backups too.
pub const MIN_CONTEXT_WINDOW: u32 = 512;

/// Widest context window a stored setting may name.
pub const MAX_CONTEXT_WINDOW: u32 = 131_072;

/// A stored context window, or a hard error naming the bound it broke.
///
/// The value reaches an Ollama request as `options.num_ctx`, where an absurd
/// number is not merely wrong — it is an out-of-memory kill of the user's
/// machine on the next generation.
pub fn validate_context_window(context_window: Option<u32>) -> AppResult<Option<u32>> {
    match context_window {
        Some(c) if !(MIN_CONTEXT_WINDOW..=MAX_CONTEXT_WINDOW).contains(&c) => Err(format!(
            "A context window of {c} is outside the supported range \
             {MIN_CONTEXT_WINDOW}–{MAX_CONTEXT_WINDOW} tokens."
        )
        .into()),
        other => Ok(other),
    }
}

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
