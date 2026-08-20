//! Phase 2 of the Autopilot rank: the optional semantic re-rank (ADR-020
//! addendum).
//!
//! Split out of `commands/autopilot.rs` (the `commands/geocoding.rs` +
//! `commands/geocoding/` precedent) purely to keep the parent under R8's LOC
//! cap — a pure move, nothing about the behaviour changed with it. The gate,
//! the cost bounds (top-N, wall clock, daily ceiling, degrade breaker), the
//! `RerankEnv` seam and the loop all live here; the command keeps the phase-1
//! keyword prefilter and the step events.
//!
//! `pub(super)` throughout: every consumer is `commands::autopilot` or its test
//! module, and keeping it that way is what makes the cost bounds auditable in
//! one place.

use super::*;

/// THE production gate for phase 2 — the single place that decides whether a
/// run re-ranks at all.
///
/// `semantic_scoring` is the user's app-wide preference (read from its
/// backend-readable mirror); a résumé-less autopilot has nothing to re-rank
/// because phase 1 produced no scores for it. Extracted as a named function
/// with exactly one production call site so the load-bearing "semantic OFF
/// makes zero embed calls" regression can be pinned against the REAL decision
/// — a test that re-types the condition (`let semantic_on = false`) pins
/// nothing.
pub(crate) fn should_semantic_rerank(semantic_scoring: bool, resume: &str) -> bool {
    semantic_scoring && !resume.trim().is_empty()
}

/// The list's ranking comparator — **two blocks, not one axis**.
///
/// After phase 2 the list holds two different scales: a re-ranked head carrying
/// the combined semantic+ATS number and a tail still on keyword coverage. Those
/// are not comparable — a never-re-ranked keyword 62 is not "better" than a
/// re-ranked combined 58 — so re-ranked jobs form the head (ordered by
/// combined) and the keyword tail follows (ordered by coverage). This is not
/// cosmetic: `generate_assistant_notes` takes its ≤3 AI-note recipients
/// straight off this order, so a single mixed axis would spend a provider
/// completion on a job the re-rank had already demoted.
///
/// Before phase 2 every job is `Keyword`, so this degenerates to the original
/// score-descending sort (unscored last) — one comparator for both call sites,
/// nothing to drift.
pub(super) fn by_rank(a: &FoundJob, b: &FoundJob) -> std::cmp::Ordering {
    fn block(j: &FoundJob) -> u8 {
        match j.score_source {
            ScoreSource::Combined => 0,
            ScoreSource::Keyword => 1,
        }
    }
    block(a).cmp(&block(b)).then_with(|| {
        b.score
            .unwrap_or(-1.0)
            .partial_cmp(&a.score.unwrap_or(-1.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// How many of a run's top keyword-ranked jobs get the semantic re-rank.
///
/// Evidence for the number: a run's raw harvest is bounded per board (the
/// aggregator caps at 2 pages × `ADZUNA_PAGE_SIZE` = ≤100 postings, matching the
/// manual search's own 100-item amount cap), and by the time this step runs the
/// list has already been through the keyword filter, the user's `minMatchScore`
/// gate and cross-board cluster dedup — so a typical kept set is a few dozen.
/// Re-ranking the top 20 therefore covers the head of the list a user actually
/// scans and acts on, while bounding the worst case at 20 posting embeds + 1
/// résumé embed per run.
///
// ponytail: the real ceiling is cost, and it is already tiny — 21 embeds is a
// rounding error next to the ONE completion `ASSISTANT_NOTES_MAX` (3) guards,
// and the ADR-017 caches (`posting_vectors` + `match_scores`) make a steady-state
// repeat run cost ZERO embeds. This is a hard bound against a pathological
// harvest, not a tuning knob — raise it only with a measurement.
pub(super) const SEMANTIC_RERANK_MAX: usize = 20;

/// How many CONSECUTIVE degraded jobs end the re-rank pass.
///
/// The per-job degrade contract is what keeps a run from failing over one bad
/// posting — but it also means a provider that is simply DOWN gets attempted
/// once per job, every scheduled run, each attempt carrying the provider's own
/// connect/read timeout. At the top-N ceiling that is a full
/// [`RERANK_STEP_TIMEOUT`] phase (300s) burned hourly to produce nothing.
///
/// Three in a row is the signal, and the number is chosen for what it rules
/// OUT: one degrade is ordinary (an unscorable posting), two can be
/// coincidence, three consecutive failures across DIFFERENT postings is not a
/// per-job shape — it is the provider. Small enough to bound the wasted phase
/// at three timeouts, large enough that an isolated bad posting never stops a
/// healthy run (the counter resets on every success).
///
/// This is a per-RUN circuit breaker only: it stops the loop, exactly like the
/// daily ceiling and cancellation already do, leaving every unvisited job on
/// its keyword score. Nothing is persisted and nothing is cached — a degraded
/// score is never written under the semantic key ([`crate::commands::match_resume`]'s
/// `cacheable` gate), so the next run retries from scratch. That is the
/// distinction from the frozen-cache defect: this bounds cost, it does not
/// remember the failure.
pub(super) const RERANK_DEGRADE_BREAKER: usize = 3;

/// Cache identity for an Autopilot posting in the ADR-017 caches.
///
/// Keyed on `canonical_job_key` — the SAME identity `merge_found_jobs` uses —
/// rather than the raw URL, so a job re-surfacing under different tracking
/// params hits the row it already paid for instead of re-embedding. Prefixed so
/// it can never collide with a real `PostingsCache` posting id (mirrors
/// `extension_bridge::match_live::adhoc_job_id`).
///
/// NOTE — the prefix is load-bearing and the resulting double embed is
/// deliberate. A posting the user later opens on the Jobs page is embedded
/// again under its REAL `PostingsCache` id: Autopilot postings never enter that
/// cache, so there is no real id to share, and the two ids are keyed on
/// different things (a stable cross-run job identity vs. a cache-lifetime
/// posting row). "Unifying" them by dropping the prefix would collide the two
/// key spaces instead of deduping them.
pub(super) fn autopilot_job_id(job: &FoundJob) -> String {
    let key =
        crate::scraping::boards::common::canonical_job_key(&job.url, &job.title, &job.company);
    format!("autopilot:{}", crate::documents::sha256_hex(&key))
}

/// Outcome counts of one semantic re-rank pass — the content-free traceability
/// summary (counts only, never posting text) the run logs and reports.
///
/// Owned by [`semantic_rerank_phase`] and filled in place by the loop, so a pass
/// the wall clock cuts off still reports the counts it reached. Returning it
/// from the loop instead lost the whole summary with the dropped future, and a
/// timed-out phase — which had already spent embeds and promoted jobs — read in
/// the step log exactly like a keyword-only run.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct RerankSummary {
    /// Jobs the keyword prefilter handed to phase 2 (≤ [`SEMANTIC_RERANK_MAX`]).
    pub(crate) considered: usize,
    /// Jobs whose score is now the combined semantic+ATS number.
    pub(crate) rescored: usize,
    /// Jobs that fell back to their keyword score (embed/provider failure, no
    /// scorable text, the daily ceiling, or cancellation).
    pub(crate) degraded: usize,
    /// The pass hit [`RERANK_STEP_TIMEOUT`]: the counts above are PARTIAL (as of
    /// the cutoff) and the untouched tail stayed keyword-only. Reported as its
    /// own step so the log distinguishes "re-rank finished" from "re-rank ran
    /// out of clock", which the counts alone cannot say.
    pub(crate) timed_out: bool,
}

/// What one job's re-rank attempt produced.
#[derive(Debug, PartialEq)]
pub(crate) enum RerankOutcome {
    /// A real combined semantic+ATS score from the shared kernel.
    Scored(f64),
    /// No semantic score for this job — an embed that failed, a provider that
    /// is offline, or an error object from the kernel. The job keeps its
    /// keyword score and the loop continues: a run NEVER fails because of
    /// scoring. A RUN of these in a row is a different signal and trips
    /// [`RERANK_DEGRADE_BREAKER`].
    Degraded,
    /// The shared per-provider daily ceiling refused the call, so no provider
    /// work happened. Every remaining job stays on its keyword score.
    BudgetExhausted,
}

/// The re-rank's I/O seam — mirrors `autopilot_helpers`'s `NoteEnv`: the one
/// external effect (a combined-kernel score, which may reach the embedding
/// provider, and the daily-budget charge that goes with an actual round-trip)
/// sits behind a trait so the loop's control flow (top-N ceiling, cancellation,
/// daily-ceiling short-circuit, per-job degrade) is unit-testable with a fake —
/// there is no way to fake a live embedding provider in-process. Prod wiring is
/// [`LiveRerankEnv`].
///
/// COST NOTE — the kernel's translation step. Scoring a cross-language posting
/// also runs `translate_if_needed`, so this phase can make up to
/// [`SEMANTIC_RERANK_MAX`] LOCAL chat completions per run on top of the embeds.
/// Those are deliberately NOT charged against `charge_provider_daily`: the
/// ceiling meters a paid provider's API budget, and translation is structurally
/// local-only (`translation::provider_allows_translation` excludes every cloud
/// provider, and the embedding-config validation is what keeps a cloud endpoint
/// out of this path), so a cloud round-trip is unreachable here. They are
/// bounded instead by the phase's own [`RERANK_STEP_TIMEOUT`] wall clock, which
/// covers embeds and translations together.
#[async_trait::async_trait]
pub(super) trait RerankEnv: Send + Sync {
    /// Score one posting through the SHARED combined kernel, charging the
    /// shared per-provider daily ceiling once per ACTUAL provider round-trip —
    /// never for a job the ADR-017 caches already answer.
    ///
    /// The charge is made by the call that reaches the provider, deep inside
    /// the kernel (`documents::embed_charged`, reached through the
    /// [`RerankBudget`] this env hands down) — the `answer_assist` precedent:
    /// charge immediately before the work that reaches the provider, on the
    /// bytes it consumes, and not at all when a cached path short-circuits.
    /// Neither this trait nor the loop can answer "will this reach the
    /// provider": both see the PRE-translation blob and neither can see the
    /// résumé-side cache. A steady-state repeat run therefore costs ZERO budget
    /// instead of one charge per considered job.
    async fn score(&self, job_id: &str, job_text: String) -> RerankOutcome;
}

/// Extract a usable SEMANTIC score from a `score_one` result — the degrade
/// boundary, kept pure so it is directly testable against realistic kernel
/// output.
///
/// `None` unless the kernel itself reports `scoreSource: "combined"`. The
/// `combined` NUMBER alone is not sufficient evidence: `score_one` degrades to
/// `combined == ats` when no embedding vector is available, and that keyword
/// number is exactly what the job already carries — promoting it would relabel a
/// keyword score as semantic and lie to the user. An error object (`{"error":…}`)
/// and a `scoreSource`-less legacy cache row likewise degrade.
pub(super) fn rerank_score_from(result: &Value) -> Option<f64> {
    if result.get("scoreSource").and_then(Value::as_str)
        != Some(crate::commands::match_resume::SCORE_SOURCE_COMBINED)
    {
        return None;
    }
    result.get("combined").and_then(Value::as_f64)
}

/// Production [`RerankEnv`]: the shared combined kernel + the shared limiter.
pub(super) struct LiveRerankEnv<'a> {
    pub(super) app: &'a AppHandle,
    pub(super) store: &'a crate::documents::DocumentStore,
    pub(super) resume: &'a str,
    pub(super) active: crate::documents::EmbeddingConfig,
    pub(super) budget: RerankBudget,
}

/// The re-rank's share of the SAME per-provider daily ceiling as interactive AI
/// — no parallel budget architecture (the `NoteEnv::charge_daily` precedent).
/// Keyed on the EMBEDDING provider, not the generation provider the AI-notes
/// step charges.
///
/// It is handed DOWN into the scoring kernel rather than consulted up here,
/// because only the kernel knows what a given job will actually cost: whether
/// the posting's cached vector matches the POST-translation text it is about to
/// embed, and whether the résumé snapshot needs an embed of its own. A
/// predicate evaluated at this level answered a different question than the one
/// the call asks — it hashed the untranslated blob (so a translated posting was
/// charged on every total cache hit) and could not see the résumé embed at all.
///
/// `exhausted` is the way the refusal travels back out: the charge happens deep
/// inside `score_one`, which degrades to keyword-only rather than failing, so
/// this flag is what tells the loop to STOP instead of walking the rest of the
/// list making refused calls.
pub(super) struct RerankBudget {
    limiter: Arc<crate::limits::Limiter>,
    provider: String,
    exhausted: std::sync::atomic::AtomicBool,
}

impl RerankBudget {
    pub(super) fn new(limiter: Arc<crate::limits::Limiter>, provider: String) -> Self {
        Self {
            limiter,
            provider,
            exhausted: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub(super) fn is_exhausted(&self) -> bool {
        self.exhausted.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl crate::documents::EmbedBudget for RerankBudget {
    fn charge_one_embed(&self) -> crate::error::AppResult<()> {
        let charged = self
            .limiter
            .charge_provider_daily(&self.provider, crate::limits::PROVIDER_DAILY_MAX);
        if let Err(ref e) = charged {
            log::info!("[autopilot] semantic re-rank stopped at daily ceiling: {e}");
            self.exhausted
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        charged
    }
}

#[async_trait::async_trait]
impl RerankEnv for LiveRerankEnv<'_> {
    async fn score(&self, job_id: &str, job_text: String) -> RerankOutcome {
        let result = crate::commands::match_resume::score_autopilot_semantic(
            self.app,
            self.store,
            self.resume,
            &self.active,
            job_id,
            job_text,
            &self.budget,
        )
        .await;
        // Checked BEFORE the score is read: a refused round-trip degrades this
        // job to keyword-only exactly like an offline provider would, so the
        // outcome alone cannot distinguish "no semantic signal" from "the
        // ceiling is gone" — and only the latter must stop the loop.
        if self.budget.is_exhausted() {
            return RerankOutcome::BudgetExhausted;
        }
        match rerank_score_from(&result) {
            Some(combined) => RerankOutcome::Scored(combined),
            None => RerankOutcome::Degraded,
        }
    }
}

/// Wall-clock ceiling for the WHOLE phase-2 pass, independent of `cancel` —
/// the same discipline (and the same reason) as the AI-notes step's
/// `NOTES_STEP_TIMEOUT`: phase 2 runs BEFORE `record_run`/`on_new_jobs`, and
/// cancellation is only checked BETWEEN jobs, so without this a run of
/// sequential cloud embeds (each with its own multi-minute provider timeout)
/// could delay the user-facing "new jobs" notification for as long as it liked.
///
/// Derived from [`SEMANTIC_RERANK_MAX`] × a generous per-job allowance so
/// raising the top-N cannot silently make the bound too tight. A hard backstop
/// against a hung provider, not a tuning knob: hitting it degrades the
/// not-yet-visited tail to keyword-only and the run continues normally.
pub(super) const RERANK_STEP_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(SEMANTIC_RERANK_MAX as u64 * 15);

/// The per-job allowance above (the bare `15`) started life as
/// `timeouts::OLLAMA_EMBED`, and the two have to stay compatible even though the
/// literal no longer says so: [`RERANK_DEGRADE_BREAKER`] consecutive degraded
/// jobs must FIT inside [`RERANK_STEP_TIMEOUT`], or the wall clock kills the
/// phase mid-job first and the breaker can never reach its threshold — which is
/// precisely the "full phase burned hourly to produce nothing" that breaker
/// exists to prevent.
///
/// One degraded job costs a whole embed budget: `OLLAMA_EMBED` ×
/// `EMBED_BUDGET_ATTEMPTS` (see `send_embed_with_retry`). Asserted here rather
/// than commented, because the coupling is invisible from `timeouts.rs` — a
/// well-meaning widening of `OLLAMA_EMBED` now fails the BUILD instead of
/// silently disabling the breaker.
const _: () = assert!(
    (crate::commands::ai_provider::timeouts::OLLAMA_EMBED.as_secs()
        * (crate::commands::ai_provider::EMBED_BUDGET_ATTEMPTS as u64)
        * (RERANK_DEGRADE_BREAKER as u64))
        < RERANK_STEP_TIMEOUT.as_secs(),
    "RERANK_DEGRADE_BREAKER embed budgets must fit inside RERANK_STEP_TIMEOUT —      lower OLLAMA_EMBED, lower RERANK_DEGRADE_BREAKER, or raise RERANK_STEP_TIMEOUT"
);

/// Phase 2 of the two-phase rank, exactly as the command runs it: the GATE,
/// then the wall-clock-bounded re-rank.
///
/// `setup` resolves the I/O seam and builds the phase-1 blob map, and is called
/// **only after the gate passes** — that is what makes "semantic OFF does no
/// work at all" a property of this function (testable with a counting fake and
/// a setup that records whether it ran) instead of a property of an untestable
/// call site. It receives the [`rerank_candidate_urls`] set so the map it builds
/// covers the candidates rather than the whole harvest. Returns `None` only
/// when NO pass ran — the gate is off, or setup found the required state
/// missing. A pass the wall clock cut off returns its PARTIAL summary with
/// `timed_out` set: it spent embeds and promoted jobs, so reporting nothing
/// would describe the run as keyword-only.
pub(super) async fn semantic_rerank_phase<E, F>(
    semantic_scoring: bool,
    resume: &str,
    found_jobs: &mut [FoundJob],
    clusters: &[crate::scraping::cluster::ClusterAssignment],
    cancel: &CancellationToken,
    setup: F,
) -> Option<RerankSummary>
where
    E: RerankEnv,
    F: FnOnce(&HashSet<&str>) -> Option<(E, std::collections::HashMap<String, String>)>,
{
    if !should_semantic_rerank(semantic_scoring, resume) {
        return None;
    }
    let candidates = rerank_candidate_urls(found_jobs, clusters);
    let Some((env, blobs)) = setup(&candidates) else {
        log::warn!(
            "[autopilot] semantic re-rank skipped: document/limiter state unavailable; ranking stays keyword-only"
        );
        return None;
    };
    // The accumulator lives HERE, outside the future the timeout may drop, so
    // the counts survive the cutoff. Whatever the loop already wrote onto
    // `found_jobs` is KEPT too (it mutates in place before each next await);
    // the unvisited tail is still on its keyword score, the ordinary degrade.
    let mut summary = RerankSummary::default();
    let timed_out = tokio::time::timeout(
        RERANK_STEP_TIMEOUT,
        semantic_rerank(&env, found_jobs, clusters, &blobs, cancel, &mut summary),
    )
    .await
    .is_err();
    if timed_out {
        summary.timed_out = true;
        log::warn!(
            "[autopilot] semantic re-rank exceeded {RERANK_STEP_TIMEOUT:?} after rescoring {} of {}; the remaining jobs stay keyword-only",
            summary.rescored,
            summary.considered
        );
    }
    Some(summary)
}

/// The urls phase 2 could spend an embed on, so the caller can build its blob
/// map for those instead of for every posting the run scraped.
///
/// A true **superset** of what [`semantic_rerank`]'s loop visits: the two
/// filters here (scored, cluster-canonical) are the loop's own, and the loop
/// then applies two MORE of its own — a missing blob, and URL variants that
/// collapse to one cache id — which need the derived id rather than the url.
/// Superset is the required direction, because the loop SKIPS a candidate whose
/// blob is missing: an under-inclusive set drops re-rank candidates silently.
///
/// Deliberately NOT capped at [`SEMANTIC_RERANK_MAX`]. The ceiling is enforced
/// by the loop's `considered` counter, which only counts jobs that got PAST
/// those two later filters — so every skip inside the first N positions pushes a
/// real candidate beyond position N, where a positionally-capped set no longer
/// has its blob. That is a capability loss (fewer jobs re-ranked than the user's
/// ceiling allows, with nothing in the summary saying so), traded here against
/// holding a JD blob for the scored canonicals of one harvest — bounded by the
/// per-board scrape cap, ~1 MB at the pathological end.
pub(super) fn rerank_candidate_urls<'a>(
    found_jobs: &'a [FoundJob],
    clusters: &[crate::scraping::cluster::ClusterAssignment],
) -> HashSet<&'a str> {
    found_jobs
        .iter()
        .enumerate()
        .filter(|(i, job)| job.score.is_some() && clusters.get(*i).is_none_or(|c| c.canonical))
        .map(|(_, job)| job.url.as_str())
        .collect()
}

/// Phase 2's loop: re-score the top [`SEMANTIC_RERANK_MAX`] CLUSTER CANONICALS
/// of the (already keyword-ranked, filtered and deduped) list through the shared
/// combined kernel, in place.
///
/// `clusters` is `cluster_aware_retain`'s verdict per surviving job, in the same
/// order, and only a cluster's CANONICAL member is re-ranked. That is one
/// decision doing two jobs: the canonical is the row the UI displays for the
/// cluster (scoring a hidden member would spend an embed on a number nobody
/// sees), and since a cluster has exactly one canonical, the same job surfaced
/// on three boards — three different `canonical_job_key`s, hence three
/// different cache ids — takes ONE top-N slot and one embed instead of three.
/// An empty `clusters` (no verdicts available) degrades to per-job identity,
/// which is the pre-cluster behaviour.
///
/// `blobs` maps a job url to the EXACT scoring blob phase 1 used, so the two
/// phases can never score different text for the same posting (`FoundJob` drops
/// `requirements`, so re-deriving the blob here would silently diverge on the
/// boards that populate it).
///
/// Degrade contract — a run never fails because of scoring:
/// - a job with no keyword score (no résumé / no scorable text) is skipped
///   entirely: there is nothing to re-rank and no reason to spend an embed;
/// - a job whose scoring degrades keeps its keyword score AND its `Keyword`
///   label, and the loop moves on to the next job;
/// - the daily ceiling, cancellation and [`RERANK_DEGRADE_BREAKER`] consecutive
///   degrades stop the loop, leaving every not-yet-visited job on its keyword
///   score.
///
/// Split from the command (mirroring `run_notes_loop`) so a fake `env` unit-tests
/// this control flow without a provider.
/// `summary` is filled IN PLACE rather than returned, so the caller still holds
/// the partial counts when [`RERANK_STEP_TIMEOUT`] drops this future mid-pass.
pub(super) async fn semantic_rerank(
    env: &dyn RerankEnv,
    found_jobs: &mut [FoundJob],
    clusters: &[crate::scraping::cluster::ClusterAssignment],
    blobs: &std::collections::HashMap<String, String>,
    cancel: &CancellationToken,
    summary: &mut RerankSummary,
) {
    // Cache identities already re-ranked THIS run. A single run can surface the
    // same job under two URL variants (the merge that collapses them runs
    // later, in `record_run`), and both derive the SAME `autopilot_job_id` — so
    // the second would burn a top-N slot to recompute a score the first already
    // produced. Same guard, same reason, as `run_notes_loop`'s `seen_this_run`.
    // Cross-BOARD duplicates (different urls, different keys, one cluster) are
    // collapsed by the canonical check below instead; this set is what still
    // holds when no clustering verdicts are available.
    let mut seen_this_run: HashSet<String> = HashSet::new();
    // Circuit breaker — see [`RERANK_DEGRADE_BREAKER`]. Reset by every success,
    // so it can only fire on a provider that is failing right now.
    let mut consecutive_degraded = 0usize;
    for (i, job) in found_jobs.iter_mut().enumerate() {
        if summary.considered >= SEMANTIC_RERANK_MAX {
            break; // top-N ceiling — the hard cost bound
        }
        // An unscored job had no résumé or no extractable text in phase 1;
        // neither is fixed by an embedding. Not counted as considered OR
        // degraded — it was never a re-rank candidate.
        if job.score.is_none() {
            continue;
        }
        let cluster = clusters.get(i);
        if cluster.is_some_and(|c| !c.canonical) {
            // A cross-board duplicate. Its cluster's canonical is the row the
            // UI shows (and the one that gets re-ranked); paying for this copy
            // would buy a score that is never displayed.
            continue;
        }
        let Some(job_text) = blobs.get(&job.url).cloned() else {
            continue; // same reasoning: no phase-1 blob means nothing to score
        };
        let cache_id = autopilot_job_id(job);
        if !seen_this_run.insert(cache_id.clone()) {
            // A different URL variant of this same job already ran this pass.
            // Checked BEFORE `env.score` so the duplicate also can't burn the
            // shared per-provider ceiling.
            continue;
        }
        summary.considered += 1;
        if cancel.is_cancelled() {
            summary.degraded += 1;
            break; // stopped by the user — the keyword score stands
        }
        match env.score(&cache_id, job_text).await {
            RerankOutcome::Scored(combined) => {
                job.score = Some(combined);
                job.score_source = ScoreSource::Combined;
                summary.rescored += 1;
                consecutive_degraded = 0;
            }
            // Per-job degrade: keep the keyword score and the `Keyword` label,
            // keep going. One offline embed must not cost the whole run — but a
            // RUN of them is the provider, not the postings, so the breaker
            // stops the pass rather than paying a provider timeout per job.
            RerankOutcome::Degraded => {
                summary.degraded += 1;
                consecutive_degraded += 1;
                if consecutive_degraded >= RERANK_DEGRADE_BREAKER {
                    log::warn!(
                        "[autopilot] semantic re-rank stopped after {consecutive_degraded} consecutive degraded jobs; the provider looks unavailable and the rest stay keyword-only"
                    );
                    break;
                }
            }
            RerankOutcome::BudgetExhausted => {
                summary.degraded += 1;
                break;
            }
        }
    }
    log::info!(
        "[autopilot] semantic re-rank: considered {} (max {SEMANTIC_RERANK_MAX}), rescored {}, degraded to keyword-only {}",
        summary.considered,
        summary.rescored,
        summary.degraded
    );
}
