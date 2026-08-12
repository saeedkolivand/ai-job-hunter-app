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
//! ## Only stages that spend a call
//!
//! `assemble` and `validate` are live stages that make no provider call, so an
//! override on either would be a control the user can set and never observe.
//! Both are refused ([`is_overridable_stage`]) — which also closes a sharper
//! edge: `Completer::for_stages` resolves every stored override BEFORE the run
//! starts and propagates a failure, so one malformed row on a stage that never
//! calls a provider used to be enough to abort the whole run.
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

use super::{validate_context_window, AiConfigStore};
use crate::commands::ai_provider::ProviderId;
use crate::db::{now_ms, ts_to_db};
use crate::error::AppResult;
use crate::ipc_contracts::events::{PIPELINE_STAGES, PIPELINE_STAGES_FREE};

/// The most override rows that can exist, ever: `stage` is the PRIMARY KEY and
/// must be one of the generated names that also spends a provider call, so the
/// OVERRIDABLE vocabulary IS the cap. Named so the import path can state the
/// bound it enforces instead of trusting the filter above it to have been
/// applied.
pub const MAX_STAGE_OVERRIDES: usize = PIPELINE_STAGES.len() - PIPELINE_STAGES_FREE.len();

/// One stage's explicitly-chosen routing.
///
/// Provider + model rather than a bare model id: a user who wants the judge on
/// a cloud model while drafting locally is changing the PROVIDER, and a shape
/// that only carried a model would have to guess which provider it belonged
/// to. `context_window` rides along because it is a property of the model this
/// row names, not of the run.
///
/// **No `base_url`, by construction.** The endpoint is a property of the
/// PROVIDER, and that provider's own settings row already holds it — read at
/// resolve time by [`Completer::from_active_for_stage`](crate::pipeline::Completer::from_active_for_stage).
/// A per-stage copy would be a second egress endpoint for the same provider
/// that no screen displays: the override editor forwards the provider's
/// configured URL and the stage list renders provider + model only, so a
/// planted value would be invisible in Settings while still receiving every
/// prompt that stage generates. `net::ssrf`'s threat model assumes the renderer
/// never controls this value; the wire shape now makes that true instead of
/// merely validated.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageOverride {
    pub provider: String,
    /// Empty ONLY for a CLI agent, which runs on its own configured default —
    /// the same rule `Completer::resolve_parts` applies to the active config.
    #[serde(default)]
    pub model: String,
    // No `base_url`: see the type doc. The endpoint comes from the named
    // provider's own stored row, at resolve time.
    /// `options.num_ctx` for THIS stage's calls, when the user set one. Absent
    /// means the provider's own default — never a guessed size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
}

/// Whether `stage` is a name a pipeline actually runs.
pub fn is_pipeline_stage(stage: &str) -> bool {
    PIPELINE_STAGES.contains(&stage)
}

/// Whether `stage` can carry a model override at all: a live stage name that
/// also SPENDS a provider call.
///
/// `assemble` and `validate` are live stages that ask no model anything
/// ([`PIPELINE_STAGES_FREE`]), so an override on either is a control with no
/// effect — and, before this refused them, a malformed row on one could still
/// fail a whole run when `Completer::for_stages` resolved it up front.
pub fn is_overridable_stage(stage: &str) -> bool {
    is_pipeline_stage(stage) && !PIPELINE_STAGES_FREE.contains(&stage)
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
        if !is_overridable_stage(stage) {
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
        let Ok(mut stmt) =
            conn.prepare("SELECT stage, provider, model, context_window FROM ai_stage_overrides")
        else {
            return out;
        };
        let rows = stmt.query_map([], |row| {
            // Read as i64 and narrow HERE rather than letting rusqlite reject
            // the column: `get::<Option<u32>>` fails the whole ROW on a
            // negative or oversized integer, and `rows.flatten()` below would
            // swallow that as a missing override — the stage would quietly run
            // on the active provider instead, which is exactly the silent
            // fallback `from_active_for_stage` promises never to do. Losing the
            // out-of-range WINDOW (the field the user cannot have set through
            // any UI) while keeping the routing they can see is the smaller
            // lie; `Completer::from_override` re-validates what survives.
            let context_window = row
                .get::<_, Option<i64>>(3)?
                .and_then(|v| u32::try_from(v).ok())
                .filter(|v| validate_context_window(Some(*v)).is_ok());
            Ok((
                row.get::<_, String>(0)?,
                StageOverride {
                    provider: row.get::<_, String>(1)?,
                    model: row.get::<_, String>(2)?,
                    context_window,
                },
            ))
        });
        if let Ok(rows) = rows {
            for (stage, over) in rows.flatten() {
                if is_overridable_stage(&stage) {
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
                (stage, provider, model, context_window, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(stage) DO UPDATE SET
                provider = excluded.provider, model = excluded.model,
                context_window = excluded.context_window,
                updated_at = excluded.updated_at",
            params![
                stage,
                over.provider,
                over.model,
                over.context_window,
                ts_to_db(now_ms()),
            ],
        )?;
        Ok(())
    }

    /// Apply a snapshot's overrides (seed + backup restore). Returns how many
    /// rows were written.
    ///
    /// Lenient per ROW, where the provider half is lenient per FIELD — because
    /// a partial provider row is legal (a provider may legitimately have a
    /// model and no base URL) and a partial override is not (an override IS
    /// "run this stage on this model"; drop the model and nothing remains). So
    /// a row naming an unknown stage, a free stage, an unknown provider, a
    /// cross-family model or a bad `base_url` is DROPPED whole, not an error —
    /// a restore must not fail wholesale on one bad entry, and a tampered
    /// `base_url` must never land as a live egress endpoint.
    ///
    /// The ONE exception is `context_window`, scrubbed per field like the
    /// provider half: it is a tuning knob rather than part of the row's
    /// identity, so an out-of-range value costs the row its window, not its
    /// existence. Import only — the interactive writer still errors, because
    /// there a human typed it and can be told.
    ///
    /// Bounded by [`MAX_STAGE_OVERRIDES`]: the vocabulary filter already makes
    /// more rows impossible, and the explicit `take` is what keeps that true if
    /// the filter is ever loosened.
    pub(super) fn apply_stage_overrides_conn(
        conn: &Connection,
        overrides: &BTreeMap<String, StageOverride>,
    ) -> AppResult<usize> {
        let mut written = 0;
        for (stage, over) in overrides
            .iter()
            .filter(|(stage, _)| is_overridable_stage(stage))
            .take(MAX_STAGE_OVERRIDES)
        {
            let mut over = over.clone();
            // Scrubbed to `None` rather than failing the row — see the
            // per-field exception in this method's doc.
            over.context_window = validate_context_window(over.context_window).ok().flatten();
            let Ok((stage, over)) = validate_stage_override(stage, over) else {
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
    if !is_overridable_stage(stage) {
        return Err(format!("{stage:?} makes no AI call — it has no model to choose.").into());
    }
    let provider_id = ProviderId::parse(&over.provider)?;
    // The same function `set_provider_settings` calls — one chain, not a second
    // copy of it: cross-family model check, `base_url` dropped for every
    // provider but `openai-compatible`, and SSRF provenance on what survives.
    // `None` for the base URL, always: the wire carries none and the row
    // stores none. `validate_settings` is still the one chain used here (cross-
    // family model check + window bound), so a stage override cannot be
    // validated more loosely than the provider settings it shadows.
    let (model, _base_url, context_window) =
        AiConfigStore::validate_settings(provider_id, Some(over.model), None, over.context_window)?;
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
