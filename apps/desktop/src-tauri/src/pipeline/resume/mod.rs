//! The staged résumé generation + validation pipeline.
//!
//! One run is a [`Pipeline`](crate::pipeline::Pipeline) of stages over one
//! [`QualityCtx`]:
//!
//! | stage            | calls  | what it produces                              |
//! | ---------------- | ------ | ---------------------------------------------- |
//! | `analyze_job`    | 1      | [`JobAnalysis`] — what the posting asks         |
//! | `match_evidence` | 0      | [`EvidenceMap`] — what the RÉSUMÉ backs         |
//! | `strategy`       | 1      | [`ResumeStrategy`] — how to present it          |
//! | `draft`          | 0 or 1 | the résumé body, streamed — 0 unless `includeResume` |
//! | `cover_letter`   | 0 or 1 | the letter body, streamed — 0 unless `includeCoverLetter` |
//! | `validate`       | 0      | the deterministic [`ContentReport`]             |
//! | `repair`         | ≤2×N   | section-scoped corrections, re-checked          |
//! | `humanize`       | ≤2     | `voice.*`-flagged lines rewritten, re-checked   |
//!
//! There used to be a second, `max`, depth here — one structured call per
//! section instead of the single streamed `draft`, plus a Warning-only
//! `llm_judge` review pass. The owner ruled it wasted tokens for no acted-on
//! value (`max` alone cost 12+ calls a run) and it was removed; every run is
//! this one pipeline now. [`project_seed::ProjectOut`] and
//! [`project_render::render_project`] survive — they are also how the
//! deterministic Projects normalization (`projects.rs`, PR #990) renders an
//! entry, unrelated to depth.
//!
//! ## The rule the stage split exists to enforce
//!
//! The model decides HOW to present verified evidence, never WHAT the candidate
//! has done. Each stage re-anchors to the SOURCE résumé rather than to the
//! previous stage's output: `match_evidence` copies its quotes straight out of
//! the source, `strategy` has its company identities re-seeded
//! from the parsed source after the model answers, and every Critical in the
//! report comes from a deterministic comparison against the source — never from
//! a model. A chain where each step trusted the last is exactly how a single
//! early fabrication becomes a confident finished document.
//!
//! ## Tauri-free (L2)
//!
//! Nothing here holds an `AppHandle`, emits an event, or touches the run store.
//! The `KvCache` and the resolved `Completer` are INJECTED by the L3 command,
//! which also owns the [`StageHooks`](crate::pipeline::StageHooks)
//! implementation and therefore every `pipeline:stage` emit. What the stages
//! need to tell that hook travels through the shared [`RunLedger`].

pub mod cache;
mod deadline;
pub mod early_research;
mod floor;
pub mod project_render;
pub mod project_seed;
pub mod projects;
pub mod prompt_blocks;
pub mod prompts;
pub mod source;
pub mod stages;
pub mod types;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Value};

use crate::error::AppResult;
use crate::pipeline::budget::{Budget, StoppedReason};
use crate::pipeline::cache::KvCache;
use crate::pipeline::{Completer, Pipeline};
use crate::validate::content::ContentReport;

pub use self::deadline::{guard_deadline, run_deadline, run_timeout_error, RunDeadline};

use self::cache::{StageCacheKey, StageIdentity};
use self::types::{EvidenceMap, JobAnalysis, ResumeStrategy};

/// The completers a run resolved for the stages the user explicitly overrode,
/// keyed by stage name. Built by L3 ([`Completer::for_stages`]) before the
/// first stage runs; a stage with no entry uses the run's default completer.
pub type StageCompleters = HashMap<String, Completer>;

/// Which entry of a per-stage map applies to `stage`, falling back to the
/// run's default.
///
/// A free generic function rather than a method so the rule it encodes —
/// **override wins for its own stage, default for every other, and an absent
/// map changes nothing** — is testable without an `AppHandle` (a `Completer`
/// needs one; a `String` does not). It is also the single lookup BOTH
/// [`QualityCtx::completer_for`] and [`QualityCtx::stage_cache_key`] go
/// through, so the two cannot answer differently.
pub(crate) fn pick<'m, T>(
    per_stage: Option<&'m HashMap<String, T>>,
    default: &'m T,
    stage: &str,
) -> &'m T {
    per_stage.and_then(|map| map.get(stage)).unwrap_or(default)
}

/// The letter text a downstream stage reads: `stage_letter` (the `cover_letter`
/// stage's own output) when it produced one, else `request_letter` (the
/// renderer-supplied, validate-only legacy text). A free function, not just a
/// `QualityCtx` method, so the decision is a test on two `&str`s rather than a
/// claim about a type this crate cannot construct in a test — see
/// [`QualityCtx::letter_text`], the only production caller.
pub(crate) fn effective_letter_text<'a>(stage_letter: &'a str, request_letter: &'a str) -> &'a str {
    if stage_letter.trim().is_empty() {
        request_letter
    } else {
        stage_letter
    }
}

/// Item + per-item byte cap on the RESOLVED requirement list below — the
/// SAME values as the wire schema (`packages/shared/src/schemas/index.ts`,
/// `topRequirements: z.array(z.string().max(300)).max(50)`) and its Rust
/// mirror (`commands::resume::{TOP_REQUIREMENTS_CAP, TOP_REQUIREMENT_BYTES_CAP}`).
/// Duplicated as local literals rather than imported: this module is L2 and
/// `commands` is L3 (`docs/architecture-rules.md` — L2 may not depend on
/// L3), and both numbers actually mirror the wire schema, not each other.
const RESOLVED_REQUIREMENTS_CAP: usize = 50;
const RESOLVED_REQUIREMENT_BYTES_CAP: usize = 300;

/// The run's actual requirement list — every downstream reader
/// ([`QualityCtx::top_requirements`], and through it `stages::validate`,
/// `stages::draft`'s emphasis directive, and the persisted aggregate) goes
/// through this rather than [`QualityInput::top_requirements`] directly, so
/// none of them can disagree about which list is "the" requirements.
///
/// **Prefers the `analyze_job` stage's OWN extraction** — `analysis.must_have`
/// then `analysis.nice_to_have`, deduped case-insensitively and capped like
/// the wire schema — over the request's own list: the analysis is what this
/// pipeline actually asked the model to extract FROM the posting, so a run
/// that produced one should never fall back to whatever the caller happened
/// to send. Falls back to `fallback` (the request's own list) only when the
/// analysis produced nothing — a run whose `analyze_job` stage has not run
/// yet, or one that genuinely extracted zero requirements.
///
/// This closes the root cause behind three symptoms at once: the renderer
/// used to send `topRequirements: []` unconditionally, making the request's
/// list the run's ONLY source — the quality panel's coverage metric read as
/// unmeasured (`validate::content::alignment::validate`'s empty-list branch),
/// the `alignment.missing_top_requirement` finding could never fire, and
/// `persist_document` saved that same empty list, which
/// `ai_generations::merge_application`'s pick-non-empty merge then silently
/// froze — a stale or even a different posting's requirements kept showing
/// as this one's keyword chips.
pub(crate) fn resolved_top_requirements(
    analysis: &JobAnalysis,
    fallback: &[String],
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for requirement in analysis
        .must_have
        .iter()
        .chain(analysis.nice_to_have.iter())
    {
        let clamped = crate::applications::clamp_to_bytes(
            requirement.trim().to_string(),
            RESOLVED_REQUIREMENT_BYTES_CAP,
        );
        if clamped.is_empty() || !seen.insert(clamped.to_lowercase()) {
            continue;
        }
        out.push(clamped);
        if out.len() >= RESOLVED_REQUIREMENTS_CAP {
            break;
        }
    }
    if out.is_empty() {
        fallback.to_vec()
    } else {
        out
    }
}

/// The BINDING: pick the routing for `stage`, then derive that stage's cache
/// key from the routing that was picked.
///
/// Generic over the routed value, and taking `identity` as a parameter, purely
/// so the binding is reachable from a test — a `Completer` needs an
/// `AppHandle`, a [`StageIdentity`] does not.
/// [`QualityCtx::stage_cache_key`] instantiates it at `Completer` with
/// `StageIdentity::of`; `the_stage_cache_key_binding_follows_the_override`
/// instantiates it at `StageIdentity` with the identity function. Both run THIS
/// body, so "the key is derived from the completer the stage will actually
/// call" is a test rather than a claim.
///
/// **What this does NOT pin** (needs a `Completer`, so needs an `AppHandle`):
/// the two field arguments `QualityCtx::stage_cache_key` passes in —
/// `self.stage_completers` and `self.default_completer`. Swapping either for a
/// wrong value there is invisible to every test in this crate.
pub(crate) fn stage_cache_key_for<'m, T>(
    base: &StageCacheKey,
    per_stage: Option<&'m HashMap<String, T>>,
    default: &'m T,
    stage: &str,
    identity: impl Fn(&'m T) -> StageIdentity<'m>,
) -> StageCacheKey {
    base.rebound(identity(pick(per_stage, default, stage)))
}

/// The seed of a run's cache chain: the inputs `analyze_job` actually reads.
/// Deliberately excludes the résumé — see `StageCacheKey::extend_source`.
pub(crate) fn chain_seed(job_ad: &str, target_language: &str) -> String {
    format!("{job_ad}\u{1f}{target_language}")
}

/// Everything one run is run AGAINST — all of it resolved server-side before
/// the pipeline starts. Borrowed: a run reads these repeatedly and copying a
/// 200 KB résumé per stage would be silly.
#[derive(Debug, Clone, Copy)]
pub struct QualityInput<'a> {
    /// The candidate's own résumé — the ONLY source of factual truth.
    pub source_resume: &'a str,
    pub job_ad: &'a str,
    /// The language the document must be written in.
    pub target_language: &'a str,
    pub top_requirements: &'a [String],
    /// Resolved job-market id (`crate::locale::letter::conventions`'s key) —
    /// drives the letter's market etiquette. `"intl"` is the wire default.
    pub market: &'a str,
    /// Today's date, pre-formatted by the caller for the target locale — the
    /// letter prompt places this exactly rather than inventing one. Empty
    /// means no date (see `letter_system`'s `has_date` gate).
    pub today: &'a str,
    /// An already-generated cover letter to validate alongside the résumé;
    /// empty when no letter is in scope. Legacy/validate-only: the `cover_letter`
    /// stage's OWN output (`QualityCtx::letter`) takes precedence over this once
    /// it has one — see [`QualityCtx::letter_text`].
    pub cover_letter: &'a str,
    /// Whether the `cover_letter` stage should generate a letter. `false` is a
    /// complete no-op (the stage finishes instantly, zero cost) — the default
    /// for every caller that predates it, so this field is the ONLY thing that
    /// changes behavior.
    pub include_cover_letter: bool,
    /// Whether the `draft` stage should generate a résumé. `true` is the wire
    /// default and every pre-existing caller's behavior; `false` is the
    /// cover-letter-only run, where `draft` finishes instantly at zero cost and
    /// [`QualityCtx::draft`] stays empty for the whole run.
    ///
    /// **Three downstream readers depend on this, and two of them fail QUIETLY
    /// if it is ignored.** `stages::validate` must not grade a résumé that was
    /// never written (an empty draft against a real source is a
    /// `factual.dropped_role` Critical per employer); `stages::humanize` keys
    /// its whole stage on a résumé report existing, so a `None` there would
    /// skip the LETTER's polish pass while recording a zero-flag artifact; and
    /// `commands::resume_pipeline::save_verdict` treats an empty draft as
    /// "nothing to save", which would discard the letter this run just paid
    /// for. The grounding stages (`analyze_job`, `match_evidence`, `strategy`)
    /// deliberately still run — `stages::cover_letter` fences `ctx.strategy`
    /// into the letter prompt and is instructed to follow it.
    pub include_resume: bool,
    /// The run's resolved company identity (`meta.company` — cache-resolved
    /// on the id path, the request's own `companyName` on the text path).
    /// Read ONLY by `cover_letter`'s opt-in research call, as the accurate
    /// override `CompanyResearch::enrich_with`'s heuristic job-ad extraction
    /// otherwise falls back to. Empty is a real absence, not an omission.
    pub company_name: &'a str,
    /// Opt-in: `cover_letter` researches [`Self::company_name`] before writing
    /// the letter and fences a `<company_research>` block into its prompt
    /// when a brief comes back non-empty. `false` is a complete no-op for
    /// this concern — the default for every caller that predates it, so this
    /// field (like [`Self::include_cover_letter`]) is the ONLY thing that
    /// changes behavior.
    pub research_company: bool,
    /// The cross-provider reasoning-effort token, threaded to the draft's
    /// stream request and to the run deadline.
    pub effort: Option<&'a str>,
    /// The run's umbrella job id — the draft and cover_letter stages stream
    /// under it.
    pub job_id: &'a str,
}

/// The shared, stage-writable half of a run: what the L3 hook needs to see, and
/// what the command reads back as metrics.
///
/// Behind an `Arc<Mutex<…>>` because [`StageHooks::after`] receives no context —
/// it is handed a [`StageInfo`](crate::pipeline::StageInfo) and an outcome, by
/// design (an observer must not be able to reach into the run). A stage that
/// wants its summary in the `pipeline:stage` event and in the persisted trail
/// therefore leaves it here.
#[derive(Debug, Default)]
pub struct RunLedger {
    state: Mutex<LedgerState>,
}

#[derive(Debug, Default)]
struct LedgerState {
    artifacts: HashMap<&'static str, Value>,
    stopped: Option<StoppedReason>,
    calls: u32,
    cached: u32,
    repair_rounds: u32,
    reverted: bool,
    /// The stage name + wall-clock ms a [`StoppedReason::Timeout`] fired at —
    /// content-free per ADR-027 (a code and a duration, never generated text).
    /// What lets `execute` build an actionable failure message ("the strategy
    /// step didn't respond within 300s") without threading the stage/duration
    /// pair through the command by hand.
    timeout: Option<(&'static str, u64)>,
}

impl RunLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one stage's content-free summary. Counts, codes and section keys
    /// only — never generated text (ADR-027); this value is emitted on
    /// `pipeline:stage` AND persisted as the event's `artifact_json`.
    pub fn record(&self, stage: &'static str, artifact: Value) {
        self.state.lock().artifacts.insert(stage, artifact);
    }

    /// The summary for `stage`, if it left one.
    pub fn artifact(&self, stage: &str) -> Option<Value> {
        self.state.lock().artifacts.get(stage).cloned()
    }

    /// Count one provider round-trip, and whether it was served from the stage
    /// cache instead.
    pub fn count_call(&self, cached: bool) {
        let mut state = self.state.lock();
        if cached {
            state.cached += 1;
        } else {
            state.calls += 1;
        }
    }

    /// Record why the run stopped early. FIRST writer wins: the earliest cause
    /// is the true one, and a later stage's own stop must not relabel a run
    /// that was already cancelled or already out of time.
    pub fn stop(&self, reason: StoppedReason) {
        let mut state = self.state.lock();
        if state.stopped.is_none() {
            state.stopped = Some(reason);
        }
    }

    pub fn stopped(&self) -> Option<StoppedReason> {
        self.state.lock().stopped
    }

    /// Record which stage a [`StoppedReason::Timeout`] fired at, and how long
    /// it had been running. FIRST writer wins, same as [`Self::stop`] — the
    /// pipeline aborts at the first stage error, so at most one stage can ever
    /// call this for one run, but the guard keeps the two methods' semantics
    /// identical rather than assumed.
    pub fn note_timeout(&self, stage: &'static str, ms: u64) {
        let mut state = self.state.lock();
        if state.timeout.is_none() {
            state.timeout = Some((stage, ms));
        }
    }

    /// The stage + duration [`Self::note_timeout`] recorded, if any.
    pub fn timeout_detail(&self) -> Option<(&'static str, u64)> {
        self.state.lock().timeout
    }

    pub fn note_repair(&self, rounds: u32, reverted: bool) {
        let mut state = self.state.lock();
        state.repair_rounds = rounds;
        state.reverted = reverted;
    }

    /// The run's content-free metrics blob, for `pipeline_runs.metrics_json`.
    pub fn metrics(&self) -> Value {
        let state = self.state.lock();
        json!({
            "calls": state.calls,
            "cached": state.cached,
            "repairRounds": state.repair_rounds,
            "reverted": state.reverted,
        })
    }
}

/// The mutable context one run threads through its stages.
pub struct QualityCtx<'a> {
    pub input: QualityInput<'a>,
    /// The run's DEFAULT resolved provider — routing is backend-owned, so a
    /// stage never chooses one.
    ///
    /// Named `default_` rather than `completer` deliberately: a stage must ask
    /// [`completer_for`](Self::completer_for), and Rust cannot stop a sibling
    /// module from reaching a private field of its own ancestor, so the next
    /// best guard is a name that makes the bypass read as one.
    pub(crate) default_completer: &'a Completer,
    /// The stages the user explicitly overrode, resolved ONCE by L3 before the
    /// run started ([`Completer::for_stages`]). `None` for every caller that
    /// has no overrides to inject — including every test — which is the same
    /// thing as an empty map and is why the default path is untouched.
    stage_completers: Option<&'a StageCompleters>,
    /// `None` when the app has no `KvCache` managed (tests, an early failure at
    /// setup): every stage then simply runs.
    pub cache: Option<&'a KvCache>,
    pub budget: Budget,
    /// The whole run's wall clock. Read by the stage that fans out (`repair`)
    /// so the deadline is enforced INSIDE it, not only at the boundaries around
    /// it — see [`RunDeadline`].
    pub deadline: RunDeadline,
    pub ledger: Arc<RunLedger>,
    /// The rolling cache identity — each stage extends it with the artifact it
    /// produced, so a later stage's key depends on everything upstream.
    pub cache_key: StageCacheKey,

    pub analysis: JobAnalysis,
    pub evidence: EvidenceMap,
    pub strategy: ResumeStrategy,
    /// The résumé body. Written by `draft`, spliced by `repair`, corrected by
    /// `humanize`.
    pub draft: String,
    pub report: Option<ContentReport>,
    /// The letter `cover_letter` generated. Empty when
    /// [`QualityInput::include_cover_letter`] is false (the stage no-ops).
    /// Never read directly by a downstream stage — see [`Self::letter_text`].
    pub letter: String,
    /// The letter's own report — present only when a letter was in scope.
    pub letter_report: Option<ContentReport>,
    /// The letter, when `draft` wrote it beside itself (see
    /// `stages::letter_ahead`); `cover_letter` then only reports it.
    pub(crate) letter_ahead: Option<stages::LetterAhead>,
    /// Channels to the early company-research lookup — `Some` only when the
    /// run driver armed one (see `early_research`).
    pub(crate) early_research: Option<early_research::EarlyResearch>,
}

impl<'a> QualityCtx<'a> {
    pub fn new(
        input: QualityInput<'a>,
        completer: &'a Completer,
        cache: Option<&'a KvCache>,
        deadline: RunDeadline,
        ledger: Arc<RunLedger>,
    ) -> Self {
        // The seed binds the cache chain to the posting + language ONLY: the
        // first cached stage (`analyze_job`) is résumé-blind, so a résumé edit
        // must not miss its cache. The résumé joins the chain in
        // `MatchEvidence` (`extend_source`), before any résumé-reading stage
        // is keyed. The posting goes in whole: a key built from an id instead
        // would serve an analysis of a posting the user has since re-scraped.
        let seed = chain_seed(input.job_ad, input.target_language);
        let cache_key = StageCacheKey::new(StageIdentity::of(completer, input.effort), &seed);
        Self {
            input,
            default_completer: completer,
            stage_completers: None,
            cache,
            budget: Budget::RESUME_QUALITY,
            deadline,
            ledger,
            cache_key,
            analysis: JobAnalysis::default(),
            evidence: EvidenceMap::default(),
            strategy: ResumeStrategy::default(),
            draft: String::new(),
            report: None,
            letter: String::new(),
            letter_report: None,
            letter_ahead: None,
            early_research: None,
        }
    }

    /// Inject the per-stage completers L3 resolved for this run.
    ///
    /// A builder so the existing constructor and every caller that has no
    /// overrides stay untouched.
    pub fn with_stage_completers(mut self, completers: &'a StageCompleters) -> Self {
        self.stage_completers = Some(completers);
        self
    }

    /// The completer `stage` runs on: its own override if the user set one,
    /// otherwise the run's default. **The only way a stage should reach a
    /// provider** — `ctx.default_completer` would ignore the override.
    ///
    /// Returns a `&'a Completer`, not a borrow of `self`: both fields it reads
    /// are already `'a` references, so a stage can hold the result across the
    /// `.await` it makes and still write back into `ctx` afterwards.
    pub fn completer_for(&self, stage: &str) -> &'a Completer {
        pick(self.stage_completers, self.default_completer, stage)
    }

    /// The cache key for `stage`, bound to the routing THAT stage will actually
    /// call — provider, model AND context window.
    ///
    /// Goes through the same [`pick`] as [`completer_for`](Self::completer_for),
    /// inside the shared [`stage_cache_key_for`] binding, so the routing a
    /// stage's answer is FILED under and the routing that PRODUCED it cannot
    /// disagree. A single run-wide key would let an overridden stage's artifact
    /// be served back to a run using the default model (and vice versa), which
    /// is the one failure a cache key exists to prevent.
    ///
    /// Binds the EFFECTIVE effort ([`Self::stage_effort`]'s rule applied to the
    /// stage's own completer), not the raw user setting: this key is only used
    /// by the two mechanical JSON stages, which send exactly that value, so an
    /// answer cached at the default tier is never served to a run that chose a
    /// different one.
    pub fn stage_cache_key(&self, stage: &str) -> StageCacheKey {
        let effort = self.input.effort;
        stage_cache_key_for(
            &self.cache_key,
            self.stage_completers,
            self.default_completer,
            stage,
            move |completer| StageIdentity::of(completer, completer.effort_or_low(effort)),
        )
    }

    /// The reasoning effort a MECHANICAL stage (analyze_job, strategy, repair,
    /// humanize) sends: the user's own choice when they made one, otherwise the
    /// lowest tier the stage's resolved model offers
    /// ([`Completer::effort_or_low`]; `None` where it has no cheap tier or no
    /// lever at all). Draft and cover letter do NOT use this — they keep the
    /// user's setting verbatim, because reasoning is what they are for.
    pub fn stage_effort(&self, stage: &str) -> Option<&'a str> {
        self.completer_for(stage).effort_or_low(self.input.effort)
    }

    /// The run's deadline guard, ready to hand to
    /// [`Completer::complete_json`](crate::pipeline::Completer::complete_json) —
    /// see [`guard_deadline`]. Owned (an `Arc` clone plus a `Copy` clock), so it
    /// does not borrow the context across the call it guards.
    pub fn deadline_guard(&self) -> impl Fn() -> AppResult<()> + 'static + use<> {
        let ledger = Arc::clone(&self.ledger);
        let deadline = self.deadline;
        move || guard_deadline(&ledger, deadline)
    }

    /// The letter text every downstream reader (validate, repair, persist, the
    /// report) must use: the `cover_letter` stage's OWN output when it produced
    /// one, falling back to [`QualityInput::cover_letter`] — the
    /// renderer-supplied, validate-only legacy text — otherwise.
    ///
    /// **The one rule that keeps two callers from disagreeing about "the
    /// letter".** Before the `cover_letter` stage existed, every reader took
    /// `ctx.input.cover_letter` directly; a run that generates its own letter
    /// must not leave any of them still reading the (now stale, usually empty)
    /// request text instead. Preferring [`Self::letter`] when it is non-empty
    /// and falling back otherwise is exactly what preserves the PRE-PR-2
    /// behavior for a run where the stage skipped (`include_cover_letter:
    /// false`).
    ///
    /// Delegates to [`effective_letter_text`], a free function for the same
    /// reason [`pick`] is one: a `QualityCtx` needs a live `Completer` to
    /// construct (which needs an `AppHandle`, which this crate's tests cannot
    /// build), while the DECISION here needs only two `&str`s.
    pub fn letter_text(&self) -> &str {
        effective_letter_text(&self.letter, self.input.cover_letter)
    }

    /// The run's actual requirement list — see [`resolved_top_requirements`]
    /// for the precedence rule. Every reader of "the top requirements" past
    /// `analyze_job` must call this, never [`QualityInput::top_requirements`]
    /// directly.
    pub fn top_requirements(&self) -> Vec<String> {
        resolved_top_requirements(&self.analysis, self.input.top_requirements)
    }

    /// How many Criticals the current report carries. `0` when nothing has been
    /// validated yet — callers must not read that as "clean" without also
    /// checking that a report exists.
    pub fn critical_count(&self) -> usize {
        self.report.as_ref().map_or(0, |report| {
            report
                .issues
                .iter()
                .filter(|issue| issue.severity == crate::validate::Severity::Critical)
                .count()
        })
    }
}

/// The quality-depth stage list, in order.
///
/// A free function rather than a `Pipeline` constant because `Pipeline` owns
/// boxed stages and the context carries a lifetime; building it per run costs
/// six allocations and keeps the stage list in one readable place.
pub fn quality_pipeline<'a>() -> Pipeline<QualityCtx<'a>> {
    Pipeline::new("resume_quality")
        .add(stages::AnalyzeJob)
        .add(stages::MatchEvidence)
        .add(stages::Strategy)
        .add(stages::Draft)
        .add(stages::CoverLetter)
        .add(stages::Validate)
        .add(stages::Repair)
        .add(stages::Humanize)
}

/// The stage names, in pipeline order — the vocabulary a `pipeline:stage`
/// event's `stage` field can carry at quality depth. Pinned by a test so the
/// renderer's timeline can key on them.
pub const QUALITY_STAGES: &[&str] = &[
    "analyze_job",
    "match_evidence",
    "strategy",
    "draft",
    "cover_letter",
    "validate",
    "repair",
    "humanize",
];
