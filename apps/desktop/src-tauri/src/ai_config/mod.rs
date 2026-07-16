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
use serde::{Deserialize, Serialize};

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
    pub providers: BTreeMap<String, ProviderConfig>,
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
    const MIGRATIONS: &'static [Migration] = &[Migration {
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
    }];

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
        let (model, base_url) = active_provider
            .as_deref()
            .and_then(|p| providers.get(p))
            .map_or((None, None), |c| (c.model.clone(), c.base_url.clone()));
        ActiveAiConfig {
            active_provider,
            model,
            base_url,
            providers,
        }
    }

    /// The export/import/seed snapshot.
    pub fn snapshot(&self) -> AiConfigSnapshot {
        let conn = self.conn.lock();
        AiConfigSnapshot {
            active_provider: Self::active_provider_conn(&conn),
            providers: Self::providers_conn(&conn),
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
    pub fn set_provider_settings(
        &self,
        provider: &str,
        model: Option<String>,
        base_url: Option<String>,
    ) -> AppResult<()> {
        let provider_id = ProviderId::parse(provider)?;
        let (model, base_url) = Self::validate_settings(provider_id, model, base_url)?;
        let conn = self.conn.lock();
        Self::upsert_provider_conn(
            &conn,
            provider_id.as_str(),
            model.as_deref(),
            base_url.as_deref(),
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
        let Ok(mut stmt) = conn.prepare("SELECT provider, model, base_url FROM ai_provider_config")
        else {
            return out;
        };
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ProviderConfig {
                    model: row.get::<_, Option<String>>(1)?,
                    base_url: row.get::<_, Option<String>>(2)?,
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
        let has_cfg = conn
            .query_row("SELECT COUNT(*) FROM ai_provider_config", [], |r| {
                r.get::<_, i64>(0)
            })
            .map(|c| c > 0)
            .unwrap_or(false);
        active || has_cfg
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
    ) -> AppResult<()> {
        conn.execute(
            "INSERT INTO ai_provider_config (provider, model, base_url, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(provider) DO UPDATE SET
                model = excluded.model, base_url = excluded.base_url,
                updated_at = excluded.updated_at",
            params![provider, model, base_url, ts_to_db(now_ms())],
        )?;
        Ok(())
    }

    fn clear_conn(conn: &Connection) -> AppResult<()> {
        conn.execute("DELETE FROM ai_provider_config", [])?;
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
            let (model, base_url) =
                Self::scrub_settings(provider_id, cfg.model.clone(), cfg.base_url.clone());
            Self::upsert_provider_conn(
                conn,
                provider_id.as_str(),
                model.as_deref(),
                base_url.as_deref(),
            )?;
            written += 1;
        }
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
    ) -> AppResult<(Option<String>, Option<String>)> {
        let model = model
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty());
        if let Some(ref m) = model {
            provider_id.validate_model(m)?;
        }
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
        Ok((model, base_url))
    }

    /// Lenient sibling of [`Self::validate_settings`] for seed/import: drop a
    /// cross-family model and a bad base_url instead of erroring, so a first-run
    /// seed or a restore never fails on one bad field.
    fn scrub_settings(
        provider_id: ProviderId,
        model: Option<String>,
        base_url: Option<String>,
    ) -> (Option<String>, Option<String>) {
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
        (model, base_url)
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
        let snapshot: AiConfigSnapshot = serde_json::from_value(data.clone())?;
        let conn = self.conn.lock();
        Self::clear_conn(&conn)?;
        // REPLACE semantics from an untrusted bundle → apply leniently (scrub, so a
        // tampered base_url can never be restored as a live egress endpoint).
        Self::apply_snapshot_conn(&conn, &snapshot)
    }
}

#[cfg(test)]
mod tests;
