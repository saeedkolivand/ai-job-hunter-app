//! Stage caching for the résumé pipeline — ADR-017 key discipline over the
//! shared [`KvCache`].
//!
//! ## What is cached, and what is deliberately not
//!
//! Only the three JSON stages (`analyze_job`, `match_evidence`, `strategy`).
//! They are pure functions of their inputs, they are the expensive part of a
//! re-run against the same posting, and their artifacts are small.
//!
//! **The draft is not cached** — it STREAMS, and a cache hit produces no
//! `ai:stream` deltas at all, so the user would watch an empty pane for a
//! result that had already arrived. **Validation and repair are not cached**
//! for a stronger reason: a validator verdict is the thing the user is being
//! asked to trust, and a stale one would report a document that no longer
//! exists as clean. Neither exclusion is an optimization gap; both are the
//! answer to "what would a wrong hit cost here".
//!
//! ## The key
//!
//! `sha256(version ∥ provider ∥ model ∥ chain)`, where `chain` is a rolling
//! hash of every artifact upstream of this stage. Each term closes a way the
//! same nominal input can mean something different:
//!
//! * [`PIPELINE_PROMPT_VERSION`] — the prompts themselves are an input. Editing
//!   a stage body without bumping it would serve answers to the OLD question.
//! * provider + model — the same prompt is a different function on a different
//!   model, and the whole point of the cache is to skip a provider call.
//! * the chained artifact hashes — `strategy` reads the analysis AND the
//!   evidence, so a changed analysis has to miss the strategy cache too. A key
//!   built only from the stage's own literal inputs would serve a strategy
//!   planned against an analysis nobody produced.

use crate::documents::sha256_hex;
use crate::pipeline::cache::KvCache;

/// Bump this whenever ANY stage prompt in [`super::prompts`], any artifact
/// shape in [`super::types`], or the composition between them changes in a way
/// that changes what the model is being asked.
///
/// It is part of every cache key, so a bump invalidates every cached stage
/// artifact at once — which is the point: a cached answer to a question that is
/// no longer being asked is worse than no cache, because it is invisible.
///
/// Test-pinned (`prompt_version_is_pinned`): the pin exists so that editing a
/// prompt makes the test fail and the author has to decide, rather than
/// shipping a silent stale-cache bug.
pub const PIPELINE_PROMPT_VERSION: u32 = 1;

/// TTL for a cached stage artifact. Seven days, matching the `company_brief`
/// namespace: the same reasoning applies (a posting's requirements do not
/// change within a week, and a user iterating on one application should not pay
/// for the same analysis twice), and a second TTL constant for the same class
/// of data would only invite the two to drift.
pub const STAGE_CACHE_TTL_SECS: i64 = 7 * 24 * 3_600;

/// `KvCache` namespace for one stage. Prefixed so a `KvCache::prune` sweep and
/// a support-bundle reader can both see at a glance which rows belong to this
/// pipeline.
fn namespace(stage: &str) -> String {
    format!("resume_stage:{stage}")
}

/// The rolling identity of "everything this stage's answer depends on".
///
/// Cloned forward stage by stage: each stage folds the artifact it just
/// produced into the chain, so the NEXT stage's key differs whenever anything
/// upstream did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageCacheKey {
    provider: String,
    model: String,
    chain: String,
}

/// Field separator inside the pre-hash string. A control character, not a
/// printable one, so no provider id, model name or artifact body can contain it
/// and shift the field boundaries — `("ab", "c")` and `("a", "bc")` must not
/// hash the same.
const FIELD_SEPARATOR: char = '\u{1f}';

impl StageCacheKey {
    /// Seed the chain with the run's own inputs (source résumé + posting +
    /// target language), under the resolved provider and model.
    pub fn new(provider: &str, model: &str, seed: &str) -> Self {
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
            chain: sha256_hex(seed),
        }
    }

    /// The cache key for the stage about to run.
    pub fn key(&self) -> String {
        sha256_hex(&format!(
            "v{PIPELINE_PROMPT_VERSION}{FIELD_SEPARATOR}{}{FIELD_SEPARATOR}{}{FIELD_SEPARATOR}{}",
            self.provider, self.model, self.chain
        ))
    }

    /// Fold a completed stage's artifact into the chain, so every LATER stage's
    /// key depends on it.
    pub fn extend(&mut self, artifact_json: &str) {
        self.chain = sha256_hex(&format!(
            "{}{FIELD_SEPARATOR}{}",
            self.chain,
            sha256_hex(artifact_json)
        ));
    }
}

/// Read one cached stage artifact, deserializing it. A row that no longer
/// parses into `T` (an artifact shape changed without a
/// [`PIPELINE_PROMPT_VERSION`] bump, a hand-edited DB) is treated as a MISS
/// rather than an error: the stage simply runs.
pub fn get<T: serde::de::DeserializeOwned>(
    cache: Option<&KvCache>,
    stage: &str,
    key: &StageCacheKey,
) -> Option<T> {
    let raw = cache?.get(&namespace(stage), &key.key(), STAGE_CACHE_TTL_SECS)?;
    serde_json::from_str(&raw).ok()
}

/// Store one stage artifact. Best-effort — `KvCache::set` already swallows its
/// own write failures, and a cache that cannot write must never fail a run.
pub fn put(cache: Option<&KvCache>, stage: &str, key: &StageCacheKey, artifact_json: &str) {
    if let Some(cache) = cache {
        cache.set(&namespace(stage), &key.key(), artifact_json);
    }
}
