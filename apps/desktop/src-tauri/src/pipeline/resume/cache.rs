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
//! `sha256(version ∥ provider ∥ model ∥ context_window ∥ effort ∥ chain)`, where
//! `chain` is a rolling hash of every artifact upstream of this stage. Each
//! term closes a way the same nominal input can mean something different:
//!
//! * [`PIPELINE_PROMPT_VERSION`] — the prompts themselves are an input. Editing
//!   a stage body without bumping it would serve answers to the OLD question.
//! * provider + model — the same prompt is a different function on a different
//!   model, and the whole point of the cache is to skip a provider call.
//! * context_window — `num_ctx` changes what the model can actually SEE of the
//!   prompt, so the same prompt at 4 096 and at 32 768 is two different
//!   questions with two different answers. It reaches the request only because
//!   the staged pipeline now sends it; the key had to move with it, or a run
//!   that raised the window is served the answer the truncated one gave.
//! * effort — on a thinking-capable Ollama model, `complete_structured` sends
//!   it as the request's own `think` field (`ollama::think_level`), which
//!   changes the model's actual reasoning depth and therefore its answer, not
//!   just how long the caller waits for one. It did NOT used to reach the
//!   provider at all (`pipeline::text_request` hard-coded it to `None` for
//!   every non-streaming call), which is why it was correctly absent from this
//!   key before; once the per-call deadline started scaling by it
//!   (`ollama_completion_deadline`), effort started reaching the provider too,
//!   and omitting it here would have let a "high"-effort answer be served back
//!   to a "low"-effort request for the same résumé/posting/model.
//! * the chained artifact hashes — `strategy` reads the analysis AND the
//!   evidence, so a changed analysis has to miss the strategy cache too. A key
//!   built only from the stage's own literal inputs would serve a strategy
//!   planned against an analysis nobody produced.
//!
//! **The rule for adding a term:** anything that reaches the provider and can
//! change the answer belongs here. `temperature` still does NOT, because the
//! structured path resolves it inside the adapter from provider + model, which
//! are already terms — it is never taken from the caller's effort/temperature
//! choice the way `think` now is.

use crate::documents::sha256_hex;
use crate::pipeline::cache::KvCache;
use crate::pipeline::Completer;

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
    context_window: Option<u32>,
    /// Normalized: `""` for `None`, the raw token otherwise — same "positional,
    /// never omitted" discipline as `context_window` (see [`Self::key`]).
    effort: String,
    chain: String,
}

/// Everything about WHO answers a stage that can change the answer — the
/// routing half of a [`StageCacheKey`], and exactly what a resolved
/// [`Completer`] carries.
///
/// A type rather than three loose arguments so "the key binds what the call
/// sends" stays one thing to keep true: a new routed value that changes the
/// answer means a new field here, and every key derivation picks it up.
///
/// Also reused (not re-derived) by `cover_letter::research::cache_key` for
/// the `company_brief` cache — a second place a `Completer` becomes cache-key
/// terms would be exactly the drift this type exists to prevent. That reuse
/// folds `context_window` into a key whose call never reads it (search +
/// synthesize doesn't touch `num_ctx`); harmless — an extra miss on a
/// window-only change, never a wrong answer — and it means a future field
/// added here propagates to that key too instead of needing an update
/// someone can forget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageIdentity<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub context_window: Option<u32>,
    /// The RAW reasoning-effort token a stage's provider call actually sends —
    /// not a `Completer` field (unlike the three above): a completer is
    /// resolved once for the whole run, but effort is per-call, so this is
    /// supplied by the caller of [`Self::of`] rather than read off the
    /// completer. `None` for a call site with no effort concept
    /// (`cover_letter::research`'s reuse of this type).
    pub effort: Option<&'a str>,
}

impl<'a> StageIdentity<'a> {
    /// The identity a resolved completer carries, PLUS the effort the caller
    /// is about to send it — the ONE place a `Completer` (and a call's effort)
    /// becomes cache-key terms.
    pub fn of(completer: &'a Completer, effort: Option<&'a str>) -> Self {
        Self {
            provider: completer.provider_id().as_str(),
            model: completer.model(),
            context_window: completer.context_window(),
            effort,
        }
    }
}

/// Field separator inside the pre-hash string. A control character, not a
/// printable one, so no provider id, model name or artifact body can contain it
/// and shift the field boundaries — `("ab", "c")` and `("a", "bc")` must not
/// hash the same.
const FIELD_SEPARATOR: char = '\u{1f}';

impl StageCacheKey {
    /// Seed the chain with the run's own inputs (source résumé + posting +
    /// target language), under the run's DEFAULT routing identity.
    pub fn new(identity: StageIdentity<'_>, seed: &str) -> Self {
        Self {
            provider: identity.provider.to_string(),
            model: identity.model.to_string(),
            context_window: identity.context_window,
            effort: identity.effort.unwrap_or_default().to_string(),
            chain: sha256_hex(seed),
        }
    }

    /// This chain, re-bound to a different routing identity — the key for a
    /// stage running on its OWN override rather than the run's default.
    ///
    /// The identity half of the key is per-STAGE, not per-run, for exactly the
    /// reason the seed carries it at all: "the same prompt is a different
    /// function on a different model". Once one stage can run
    /// somewhere else, a single run-wide identity would file that stage's
    /// answer under the default model's key — and serve it back on the next run
    /// even after the override is removed. The `chain` is untouched: what came
    /// before this stage is the same regardless of who answers it.
    ///
    /// Returns a new key rather than mutating, so the rolling chain in
    /// `QualityCtx::cache_key` stays the run's shared identity and each stage
    /// derives its own view of it.
    pub fn rebound(&self, identity: StageIdentity<'_>) -> Self {
        Self {
            provider: identity.provider.to_string(),
            model: identity.model.to_string(),
            context_window: identity.context_window,
            effort: identity.effort.unwrap_or_default().to_string(),
            chain: self.chain.clone(),
        }
    }

    /// The cache key for the stage about to run.
    ///
    /// An unconfigured window hashes as an EMPTY field rather than being
    /// omitted, so the term stays positional and no `Some(n)` key can collide
    /// with a `None` one. Adding the term changed the pre-hash shape, which
    /// invalidates every entry written before it — a one-time miss on a 7-day
    /// TTL, and the safe direction: a changed key can only cost a call, never
    /// serve a wrong answer. `effort` follows the exact same discipline (see
    /// the module doc for why it is a term at all).
    pub fn key(&self) -> String {
        let window = self
            .context_window
            .map(|c| c.to_string())
            .unwrap_or_default();
        let pre_hash = format!(
            "v{PIPELINE_PROMPT_VERSION}{FIELD_SEPARATOR}{}{FIELD_SEPARATOR}{}{FIELD_SEPARATOR}{window}{FIELD_SEPARATOR}{}{FIELD_SEPARATOR}{}",
            self.provider, self.model, self.effort, self.chain
        );
        sha256_hex(&pre_hash)
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
