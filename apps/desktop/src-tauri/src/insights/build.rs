//! Join the stores' rows into the pure inputs the math reads. Pure: the status
//! history comes in through a closure, so the caller owns the I/O.

use std::collections::HashMap;

use super::{classify, effective_status, ever_reached_positive, Outcome};
use crate::ai_generations::AiGenerationRecord;
use crate::applications::{normalize_job_url, Application, ApplicationStatus, StatusEvent};
use crate::export::types::TemplateId;

/// `raw` as a template id this build knows, else `None` (the "unknown" group). The stored
/// column also arrives through backup import and sync, so it is untrusted text: `TemplateId`
/// deserializes leniently (anything unknown becomes Classic), hence the round trip: only a
/// string that survives it unchanged is a real, current id.
fn known_template(raw: &str) -> Option<String> {
    let parsed: TemplateId =
        serde_json::from_value(serde_json::Value::String(raw.to_string())).ok()?;
    let canonical = serde_json::to_value(parsed).ok()?;
    (canonical.as_str() == Some(raw)).then(|| raw.to_string())
}

/// What the insights read from a job's generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationFacts {
    pub target_language: String,
    /// The template the generation was last exported with, when known.
    pub template: Option<String>,
    /// The cover letter text; empty when the generation has none.
    pub cover_letter: String,
}

/// One sent application, ready for grouping and the nudge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInput {
    pub applied_at: u64,
    pub board: String,
    /// `None` = excluded from every outcome count (withdrawn); still part of the
    /// nudge window, because it was sent.
    pub outcome: Option<Outcome>,
    /// The generation for the job's normalized url, when one exists.
    pub generation: Option<GenerationFacts>,
}

/// Inputs for every application that was ever sent (not `saved`), newest first.
/// `events_of` returns one application's status history, oldest first, or `None` when
/// it could not be read in full: that application is excluded from the outcome counts
/// (a partial history must not read as a complete one). It is asked about every sent
/// application except a withdrawn one.
pub fn build_inputs(
    apps: &[Application],
    generations: &[AiGenerationRecord],
    events_of: impl Fn(&str) -> Option<Vec<StatusEvent>>,
    now: u64,
) -> Vec<AppInput> {
    // Two rows can share a normalized key (a legacy raw-url row beside the canonical
    // one): the canonical row (stored url == key) wins, otherwise the first in input
    // order, so the pick never depends on which duplicate came last.
    let mut by_url: HashMap<String, &AiGenerationRecord> = HashMap::new();
    for g in generations.iter().filter(|g| !g.job_url.is_empty()) {
        let key = normalize_job_url(&g.job_url);
        let canonical = g.job_url == key;
        if by_url
            .get(&key)
            .is_none_or(|cur| canonical && cur.job_url != key)
        {
            by_url.insert(key, g);
        }
    }
    let mut out: Vec<AppInput> = apps
        .iter()
        .filter(|a| a.status != ApplicationStatus::Saved)
        .map(|a| {
            let applied_at = a.applied_at.unwrap_or(a.created_at);
            // Withdrawn is excluded whatever its history says; every other sent
            // status can be an unreviewed email suggestion, so read its history.
            let outcome = if a.status == ApplicationStatus::Withdrawn {
                None
            } else {
                events_of(&a.id).and_then(|history| {
                    let status = effective_status(a.status, &history);
                    classify(
                        status,
                        ever_reached_positive(status, &history),
                        applied_at,
                        now,
                    )
                })
            };
            let generation = by_url
                .get(a.job_url.as_str())
                .filter(|_| !a.job_url.is_empty())
                .map(|g| GenerationFacts {
                    target_language: g.target_language.clone(),
                    template: g.last_export_template.as_deref().and_then(known_template),
                    cover_letter: g.cover_letter_text.clone(),
                });
            AppInput {
                applied_at,
                board: a.board.clone(),
                outcome,
                generation,
            }
        })
        .collect();
    out.sort_by_key(|i| std::cmp::Reverse(i.applied_at));
    out
}
