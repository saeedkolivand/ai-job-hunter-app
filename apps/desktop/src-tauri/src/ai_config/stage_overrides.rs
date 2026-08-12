//! Per-stage generation model overrides — "run `strategy` on the big model and
//! everything else on the small one".
//!
//! ## Never a silent switch
//!
//! A stage runs on the ACTIVE provider unless the user explicitly set a row for
//! it. There is no inference, no suggested default applied behind the user's
//! back, and no partial row: an override names a provider AND a model, and a
//! stage with no row resolves exactly as it did before this table existed
//! ([`crate::pipeline::Completer::from_active`]). "Absent" is therefore always
//! readable as "the user did not ask for anything here", which is what makes it
//! safe for a later suggested-defaults UI to propose a value without applying
//! it.
//!
//! ## The same validation chain as the active config, twice
//!
//! A row is validated at WRITE time through
//! [`AiConfigStore::validate_settings`] — the identical
//! `ProviderId::parse` → `validate_model` → `base_url`-provenance
//! (`net::ssrf`) chain the active provider config goes through — and again at
//! READ/RESOLVE time by `Completer::from_active_for_stage`, because a store can
//! also be populated by a restored backup rather than by the settings writer.
//! The read side is not paranoia theatre: `DataStore::import` accepts an
//! untrusted bundle.
//!
//! ## Why the stage vocabulary is checked in code, not in SQL
//!
//! `stage` must be one of `ipc_contracts::events::PIPELINE_STAGES` (generated
//! from `packages/shared/src/events/pipeline.ts`). That check lives in Rust
//! rather than in a `CHECK` constraint because a constraint would bake a COPY
//! of the vocabulary into every user's database file at the version they first
//! opened it — adding a stage later could not reach it, and dropping one would
//! need a table rebuild. The PRIMARY KEY still does the structural half: one
//! row per stage, so the table cannot outgrow the vocabulary.

use std::collections::BTreeMap;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::AiConfigStore;
use crate::commands::ai_provider::ProviderId;
use crate::db::{now_ms, ts_to_db};
use crate::error::AppResult;
use crate::ipc_contracts::events::PIPELINE_STAGES;

/// The most override rows that can exist, ever: `stage` is the PRIMARY KEY and
/// must be one of the generated names, so the vocabulary IS the cap. Named so
/// the import path can state the bound it enforces instead of trusting the
/// filter above it to have been applied.
pub const MAX_STAGE_OVERRIDES: usize = PIPELINE_STAGES.len();

/// One stage's explicitly-chosen routing.
///
/// Provider + model + `base_url` rather than a bare model id: a user who wants
/// the judge on a cloud model while drafting locally is changing the PROVIDER,
/// and a shape that only carried a model would have to guess which provider it
/// belonged to. `context_window` rides along because it is a property of the
/// model this row names, not of the run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageOverride {
    pub provider: String,
    /// Empty ONLY for a CLI agent, which runs on its own configured default —
    /// the same rule `Completer::resolve_parts` applies to the active config.
    #[serde(default)]
    pub model: String,
    /// Only meaningful for `openai-compatible`; dropped to `None` for every
    /// other provider, exactly as in the active config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// `options.num_ctx` for THIS stage's calls, when the user set one. Absent
    /// means the provider's own default — never a guessed size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
}

/// Whether `stage` is a name a pipeline actually runs.
pub fn is_pipeline_stage(stage: &str) -> bool {
    PIPELINE_STAGES.contains(&stage)
}

impl AiConfigStore {
    // ── Reads ──────────────────────────────────────────────────────────────────

    /// Every explicitly-set override, keyed by stage name.
    ///
    /// Rows the current vocabulary no longer knows (a stage renamed between
    /// releases, a hand-edited database) are DROPPED from the read rather than
    /// surfaced: nothing downstream can act on them, and showing one in
    /// Settings would offer the user a control with no effect. Owned + lock-free
    /// to the caller, like every other accessor here.
    pub fn stage_overrides(&self) -> BTreeMap<String, StageOverride> {
        let conn = self.conn.lock();
        Self::stage_overrides_conn(&conn)
    }

    /// One stage's override, or `None` when the user set none — the read
    /// `Completer::from_active_for_stage` makes per stage.
    pub fn stage_override(&self, stage: &str) -> Option<StageOverride> {
        if !is_pipeline_stage(stage) {
            return None;
        }
        let conn = self.conn.lock();
        Self::stage_overrides_conn(&conn).remove(stage)
    }

    // ── Writes ─────────────────────────────────────────────────────────────────

    /// Set (or replace) one stage's override.
    ///
    /// Strict, like `set_provider_settings`: an unknown stage, an unknown
    /// provider, a cross-family model, a bad `base_url` or an out-of-range
    /// context window are hard errors the Settings UI shows — a silently
    /// scrubbed override would leave the user believing a stage runs somewhere
    /// it does not.
    ///
    /// An EMPTY model is rejected for everything but a CLI agent. The active
    /// config tolerates one (a half-finished settings screen is a legitimate
    /// intermediate state); an override does not, because its entire content is
    /// "run this stage on this model" and an empty one would resolve to an
    /// error at generation time instead of at the click that created it.
    pub fn set_stage_override(&self, stage: &str, over: StageOverride) -> AppResult<()> {
        let (stage, over) = validate_stage_override(stage, over)?;
        let conn = self.conn.lock();
        Self::upsert_stage_override_conn(&conn, &stage, &over)
    }

    /// Remove one stage's override — that stage falls back to the active
    /// provider on the next run.
    pub fn clear_stage_override(&self, stage: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM ai_stage_overrides WHERE stage = ?1",
            params![stage],
        )?;
        Ok(())
    }

    // ── Connection-bound helpers (single lock; reused by snapshot/import) ───────

    pub(super) fn stage_overrides_conn(conn: &Connection) -> BTreeMap<String, StageOverride> {
        let mut out = BTreeMap::new();
        let Ok(mut stmt) = conn.prepare(
            "SELECT stage, provider, model, base_url, context_window FROM ai_stage_overrides",
        ) else {
            return out;
        };
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                StageOverride {
                    provider: row.get::<_, String>(1)?,
                    model: row.get::<_, String>(2)?,
                    base_url: row.get::<_, Option<String>>(3)?,
                    context_window: row.get::<_, Option<u32>>(4)?,
                },
            ))
        });
        if let Ok(rows) = rows {
            for (stage, over) in rows.flatten() {
                if is_pipeline_stage(&stage) {
                    out.insert(stage, over);
                }
            }
        }
        out
    }

    fn upsert_stage_override_conn(
        conn: &Connection,
        stage: &str,
        over: &StageOverride,
    ) -> AppResult<()> {
        conn.execute(
            "INSERT INTO ai_stage_overrides
                (stage, provider, model, base_url, context_window, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(stage) DO UPDATE SET
                provider = excluded.provider, model = excluded.model,
                base_url = excluded.base_url, context_window = excluded.context_window,
                updated_at = excluded.updated_at",
            params![
                stage,
                over.provider,
                over.model,
                over.base_url,
                over.context_window,
                ts_to_db(now_ms()),
            ],
        )?;
        Ok(())
    }

    /// Apply a snapshot's overrides (seed + backup restore). Returns how many
    /// rows were written.
    ///
    /// LENIENT per row, like the provider half: a row naming an unknown stage,
    /// an unknown provider, a cross-family model or a bad `base_url` is
    /// DROPPED, not an error — a restore must not fail wholesale on one bad
    /// entry, and a tampered `base_url` must never land as a live egress
    /// endpoint. Bounded by [`MAX_STAGE_OVERRIDES`]: the vocabulary filter
    /// already makes more rows impossible, and the explicit `take` is what
    /// keeps that true if the filter is ever loosened.
    pub(super) fn apply_stage_overrides_conn(
        conn: &Connection,
        overrides: &BTreeMap<String, StageOverride>,
    ) -> AppResult<usize> {
        let mut written = 0;
        for (stage, over) in overrides
            .iter()
            .filter(|(stage, _)| is_pipeline_stage(stage))
            .take(MAX_STAGE_OVERRIDES)
        {
            let Ok((stage, over)) = validate_stage_override(stage, over.clone()) else {
                continue;
            };
            Self::upsert_stage_override_conn(conn, &stage, &over)?;
            written += 1;
        }
        Ok(written)
    }
}

/// The write-time gate: stage vocabulary, then the EXACT provider/model/base_url
/// chain the active config uses, then the model-presence rule.
///
/// Returns the canonical (parsed, trimmed, scrubbed-of-inert-fields) row to
/// persist, so what lands in the table is what the resolver will accept.
fn validate_stage_override(stage: &str, over: StageOverride) -> AppResult<(String, StageOverride)> {
    // Deliberately NOT trimmed: the vocabulary is closed and generated, so
    // `"draft "` is a different string, not a sloppy spelling of `"draft"`.
    // Accepting it would give one stage two spellings in a PRIMARY KEY column —
    // the same canonical-form argument the section-key grammar makes.
    if !is_pipeline_stage(stage) {
        return Err(format!(
            "{stage:?} is not a stage this pipeline runs, so it cannot have a model override."
        )
        .into());
    }
    let provider_id = ProviderId::parse(&over.provider)?;
    // The same function `set_provider_settings` calls — one chain, not a second
    // copy of it: cross-family model check, `base_url` dropped for every
    // provider but `openai-compatible`, and SSRF provenance on what survives.
    let (model, base_url, context_window) = AiConfigStore::validate_settings(
        provider_id,
        Some(over.model),
        over.base_url,
        over.context_window,
    )?;
    let model = match model {
        Some(model) => model,
        None if provider_id.is_cli_agent() => String::new(),
        None => {
            return Err(format!(
                "Pick a model for the {} override, or remove it.",
                provider_id.as_str()
            )
            .into())
        }
    };
    Ok((
        stage.to_string(),
        StageOverride {
            provider: provider_id.as_str().to_string(),
            model,
            base_url,
            context_window,
        },
    ))
}

/// Sanity bound on the vocabulary itself: an empty `PIPELINE_STAGES` would make
/// `MAX_STAGE_OVERRIDES` zero and silently disable the whole feature.
const _: () = assert!(
    MAX_STAGE_OVERRIDES > 0,
    "the generated stage vocabulary must not be empty"
);

#[cfg(test)]
mod tests;
