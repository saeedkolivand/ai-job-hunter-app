//! Per-request HTTP timeouts for the AI provider adapters.
//!
//! Each constant is named by the *operation* it bounds, not by its raw value, so
//! a call site reads as `.timeout(timeouts::COMPLETION)` instead of a bare magic
//! number. Two sites share a constant only when they bound the **same operation**
//! at the **same duration**; same duration + different operation gets its own
//! constant so the values can drift independently later. The one exception is
//! [`STREAM`]: every `chat_stream` call site uses [`stream_deadline`] (a
//! function of the request's reasoning effort) instead of the bare constant —
//! see its doc comment.
//!
//! Values are intentionally identical to the literals they replaced — this module
//! is a pure extraction with no behavior change (except [`stream_deadline`],
//! added later to fix a high-effort stream being killed mid-generation by the
//! flat [`STREAM`] deadline).

use std::time::Duration;

use crate::ipc_contracts::ai_timeouts::{
    EFFORT_TIMEOUT_MULTIPLIER, MAX_RUN_FIXED_SECS, MAX_RUN_GENERATION_PASSES,
    QUALITY_RUN_FIXED_SECS, QUALITY_RUN_GENERATION_PASSES, STREAM_BASELINE_SECS,
};

// ── Chat generation ─────────────────────────────────────────────────────────────

/// Streaming chat completion (`chat_stream`): the long-running SSE/JSON stream a
/// cloud provider or the local Ollama daemon emits while generating. This is the
/// BASELINE — every `chat_stream` call site uses [`stream_deadline`] (which
/// scales this by the request's reasoning effort), never this constant
/// directly, so a high-effort generation on a slow model isn't killed
/// mid-stream by a one-size-fits-all cap.
///
/// The value itself is generated from `packages/shared/src/ai-timeouts.ts`
/// (`STREAM_BASELINE_SECS`, via `pnpm gen:ipc` →
/// `ipc_contracts::ai_timeouts`) — the SAME source the renderer's own
/// `computeStreamTimeoutMs` (`renderer/lib/generate/stream-promise.ts`)
/// imports directly, so the two sides can no longer drift independently;
/// `pnpm gen:ipc:check` fails if this constant and the TS source disagree.
pub const STREAM: Duration = Duration::from_secs(STREAM_BASELINE_SECS);

/// Reasoning-effort → [`STREAM`] multiplier. `req.effort` (`AiGenerateRequest`)
/// is a closed, cross-provider vocabulary (see `OPENAI_EFFORT_LEVELS`,
/// `OLLAMA_EFFORT_LEVELS`, `anthropic_effort_levels`, `gemini_effort_levels`) —
/// the underlying table (generated `EFFORT_TIMEOUT_MULTIPLIER`) is the union of
/// every level any adapter currently exposes. "minimal"/"low"/unset/unrecognized
/// get 1.0 (no reason to extend the baseline); everything above that scales up,
/// since a higher reasoning budget is the actual driver of a stream legitimately
/// running long. A new provider that reuses this SAME `effort` vocabulary
/// benefits automatically; one that ever needs a genuinely new tier name already
/// requires touching its own `effort_levels()` to expose it, so extending the
/// shared table at the same time is not a departure from the
/// zero-change-per-provider rule.
///
/// TIER ORDER, because it is not alphabetical and reads wrong at a glance:
/// `minimal < low < medium < high < xhigh < max`. **`max` is the TOP tier, not
/// `xhigh`** — per Anthropic's effort docs (`low | medium | high | xhigh |
/// max`) and OpenAI's `reasoning_effort` (`none | minimal | low | medium |
/// high | xhigh | max`). An earlier version of this table gave `max` a SMALLER
/// multiplier than `xhigh`, so the highest-effort requests got the shortest
/// deadline — the exact failure this function exists to prevent, reserved for
/// the runs most likely to hit it. The generated table
/// (`packages/shared/src/ai-timeouts.ts`) keeps its entries in ascending tier
/// order for the same reason — keep the monotonicity test's array in that same
/// order too, or it will pass while pinning the inversion.
fn effort_multiplier(effort: Option<&str>) -> f64 {
    match effort {
        Some(e) => EFFORT_TIMEOUT_MULTIPLIER
            .iter()
            .find(|(tier, _)| *tier == e)
            .map_or(1.0, |(_, mult)| *mult),
        None => 1.0,
    }
}

/// The closed TIER token an effort string resolves to — `"baseline"` for
/// absent/`minimal`/`low`/unrecognized, exactly where [`effort_multiplier`]
/// returns 1.0.
///
/// Exists for LOGGING. `effort` arrives from the renderer as free text (the
/// wire schema's cap is Zod, which never runs on the bare-`invoke` transport),
/// so a raw copy of it in a `Span` message is an unbounded, newline-capable
/// value landing in the diagnostics bundle — a log-injection primitive of the
/// same shape `normalize_language` exists to close. Log the token this returns,
/// which is always one of the table's own `&'static str`s.
pub fn effort_tier(effort: Option<&str>) -> &'static str {
    match effort {
        Some(e) => EFFORT_TIMEOUT_MULTIPLIER
            .iter()
            .find(|(tier, _)| *tier == e)
            .map_or("baseline", |(tier, _)| *tier),
        None => "baseline",
    }
}

/// The actual per-request deadline for `chat_stream`: [`STREAM`] scaled by
/// [`effort_multiplier`]. `reqwest::RequestBuilder::timeout` bounds the WHOLE
/// request (connect through the last streamed byte — see reqwest's own docs),
/// so this is a total deadline, not a per-chunk idle timeout; a per-chunk idle
/// timeout would be architecturally nicer (it wouldn't need effort awareness
/// at all — a stream that's still actively producing text would never be
/// killed, regardless of how long it legitimately runs) but would mean
/// loosening or removing this same total timeout at all 4 call sites AND
/// adding a second timing mechanism inside the shared `stream_response` loop
/// (`stream.rs`) — a materially bigger surface for the same bug. Scaling the
/// existing total deadline is the smaller, fully-tested fix.
pub fn stream_deadline(effort: Option<&str>) -> Duration {
    Duration::from_secs_f64(STREAM.as_secs_f64() * effort_multiplier(effort))
}

/// Non-streaming cloud completion (`complete`): a single full-response call to a
/// cloud provider (OpenAI / Anthropic / Gemini).
pub const COMPLETION: Duration = Duration::from_secs(120);

/// Non-streaming **local** Ollama completion (`complete`): the local daemon can
/// be far slower than a cloud API on first token, so it gets the longer
/// stream-class budget rather than the cloud [`COMPLETION`] bound.
///
/// Like [`COMPLETION`], this is handed to
/// [`retry::send_with_retry`](super::retry::send_with_retry) rather than set on
/// the request builder, so it bounds the whole retry SEQUENCE and not one
/// attempt. Every outer deadline derived from these constants —
/// [`quality_run_deadline`] above all — counts one of them per provider call and
/// is wrong by `retry::MAX_ATTEMPTS` if that ever stops being true.
pub const OLLAMA_COMPLETION: Duration = Duration::from_secs(300);

// ── Embeddings ──────────────────────────────────────────────────────────────────

/// Cloud embeddings (`embed`): a single-vector embeddings request to OpenAI or
/// Gemini.
pub const EMBED: Duration = Duration::from_secs(30);

/// Local Ollama embeddings (`/api/embeddings`): the local daemon's embeddings
/// endpoint, bounded tighter than cloud embeddings.
pub const OLLAMA_EMBED: Duration = Duration::from_secs(15);

// ── Company research (provider-native web search) ───────────────────────────────

/// Cloud native web-search research (`research`): the provider's own server-side
/// web-search tool call (OpenAI `web_search`, Anthropic `web_search_20250305`,
/// Gemini `google_search`).
pub const WEB_SEARCH: Duration = Duration::from_secs(25);

/// Ollama Web Search API (`/api/web_search` on ollama.com): the hosted search
/// call that backs the Ollama-family research path.
pub const OLLAMA_WEB_SEARCH: Duration = Duration::from_secs(15);

/// Exa (`api.exa.ai/search`): the configurable fallback search backend. Same
/// bound as [`OLLAMA_WEB_SEARCH`] — same operation (one hosted search POST), so
/// they share a value but not a constant, per this module's own rule.
pub const EXA_SEARCH: Duration = Duration::from_secs(15);

/// BASELINE for the OUTER bound on a whole research pass — search **plus** the
/// model's synthesis of the results — held by `CompanyResearch::enrich_with` and
/// `SalaryResearch`. Every call site uses [`research_deadline`], never this
/// constant directly, for the same reason [`STREAM`] has [`stream_deadline`]:
/// synthesis is a model call, so its cost scales with reasoning effort.
///
/// This wraps [`WEB_SEARCH`]/[`OLLAMA_WEB_SEARCH`] *and* a completion, so it must
/// exceed their sum or it becomes the binding constraint and the inner bounds
/// (which produce actionable errors) never get to fire. The previous flat 25s
/// was barely above the 25s [`WEB_SEARCH`] bound alone: a 2026-08-08 support
/// bundle shows a reasoning model taking 9.2s for synthesis at idle, and all six
/// research calls in a concurrent batch timing out — every one of those cover
/// letters shipped with no company knowledge at all.
pub const RESEARCH_BASELINE: Duration = Duration::from_secs(90);

/// The actual deadline for one research pass: [`RESEARCH_BASELINE`] scaled by
/// [`effort_multiplier`], exactly as [`stream_deadline`] scales [`STREAM`].
/// Shares the one generated multiplier table, so a new effort tier is picked up
/// here for free.
pub fn research_deadline(effort: Option<&str>) -> Duration {
    Duration::from_secs_f64(RESEARCH_BASELINE.as_secs_f64() * effort_multiplier(effort))
}

// ── Staged résumé pipeline ──────────────────────────────────────────────────────

/// Deadline for ONE WHOLE quality-depth résumé pipeline run — what makes
/// `StoppedReason::RunTimeout` reachable.
///
/// **Not `baseline × multiplier`, unlike [`stream_deadline`] and
/// [`research_deadline`].** All but ONE of a run's calls do not scale with
/// reasoning effort: the three JSON stages and every repair-round section
/// rewrite go through `complete_with_usage`, whose bounds ([`COMPLETION`] /
/// [`OLLAMA_COMPLETION`]) are flat constants. So the formula is
/// `fixed + baseline × passes × multiplier`, where the fixed term is
/// 3 stages × 2 round-trips (`complete_json` allows exactly one re-ask) +
/// `max_repair_attempts` (2) rounds × `MAX_SECTIONS_PER_ROUND` (4) sections,
/// i.e. 14 calls × [`OLLAMA_COMPLETION`] = 4200 s, and the scaling term is the
/// draft — the run's only streamed call.
///
/// Both terms come from `packages/shared/src/ai-timeouts.ts` through
/// `pnpm gen:ipc`; the resulting per-tier table is pinned on BOTH sides
/// (`quality_run_deadline_pins_the_derived_table` here,
/// `qualityRunDeadlineSecs > pins the derived per-tier table` there), which is
/// what keeps the shared arithmetic from drifting even though each side spells
/// it out.
///
/// **Counting ONE [`OLLAMA_COMPLETION`] per call is a claim about
/// [`retry`](super::retry), not just about `.timeout()`.** A transient failure is
/// re-sent up to `retry::MAX_ATTEMPTS` times, so the retry loop bounds the whole
/// sequence by the caller's timeout — it applies that timeout itself and gives a
/// retry only the remainder. Before it did, one "300 s" call could cost 3 × 300 s
/// plus backoff and this deadline was short by that factor;
/// `retry::a_one_shot_call_stops_once_its_own_timeout_is_spent` is the guard that
/// keeps this arithmetic true.
///
/// This is a BACKSTOP, not a target: [`stream_deadline`]/[`OLLAMA_COMPLETION`]
/// catch a single hung call, and `Budget::step_timeout` catches a hung stage.
/// This one catches the run that answers every step just slowly enough never to
/// trip either.
pub fn quality_run_deadline(effort: Option<&str>) -> Duration {
    let generation =
        STREAM.as_secs_f64() * QUALITY_RUN_GENERATION_PASSES as f64 * effort_multiplier(effort);
    Duration::from_secs_f64(QUALITY_RUN_FIXED_SECS as f64 + generation.round())
}

/// The max-depth deadline's two terms come from `packages/shared` through
/// `pnpm gen:ipc`, exactly like their quality siblings:
/// [`MAX_RUN_FIXED_SECS`] and [`MAX_RUN_GENERATION_PASSES`].
///
/// **They were two hand-typed literals in two files, and the doc on the TS side
/// claimed they could not drift.** Nothing enforced it: the generator emitted
/// only the quality pair, and each side's per-tier table pinned its own
/// literals — so changing one constant left the other's table green and the two
/// deadlines silently disagreeing, which is the failure the renderer's client
/// bound exists to prevent (the backend must give up FIRST, because it is the
/// side that knows WHY).
///
/// The fixed term is 24 calls at the flat [`OLLAMA_COMPLETION`] bound: 4
/// single-call stages (analyze, evidence, strategy, **and the judge**) +
/// `max_sections` (12) section calls + `max_repair_attempts` (2) ×
/// `MAX_SECTIONS_PER_ROUND` (4) repair rewrites. Max depth streams nothing, so
/// unlike [`QUALITY_RUN_FIXED_SECS`] it covers the WHOLE run rather than its
/// effort-invariant part.
///
/// **The judge was missed once, and the arithmetic hid it.** At 23 calls the
/// fixed term was 6 900 s, and the effort-scaled term (300 s at the bottom
/// tier) brought `max_run_deadline(None)` to exactly 7 200 s = the 24 calls a
/// max run really plans — so the "the deadline clears the inner bounds" pin
/// passed with ZERO slack, on an equality it was never meant to sit on. That
/// pin now derives its call count from `max_pipeline()` itself, so the next
/// stage that makes a call fails it instead of silently eating the slack.
///
/// **The one allowed re-ask per JSON call is not counted**, which is the one
/// place this departs from the quality derivation. See
/// [`Budget::RESUME_MAX`](crate::pipeline::budget::Budget::RESUME_MAX)'s doc
/// for the argument: a re-ask is a parse-failure path guarded by the same
/// clock, and a max run stopped mid-fan-out keeps the sections it already
/// assembled — so buying the worst case here would only make the deadline
/// unreachable.
/// Deadline for ONE WHOLE max-depth résumé run — the max-depth twin of
/// [`quality_run_deadline`], and what makes `StoppedReason::RunTimeout`
/// reachable at this depth.
///
/// Same `fixed + baseline × passes × multiplier` shape, with both terms
/// documented above. Pinned by `max_run_deadline_clears_the_inner_per_call_bounds`
/// (which recomputes the fixed half from the fan-out constants themselves) and
/// by `max_run_deadline_agrees_with_the_budget_floor_at_the_bottom_tier`.
pub fn max_run_deadline(effort: Option<&str>) -> Duration {
    let generation =
        STREAM.as_secs_f64() * MAX_RUN_GENERATION_PASSES as f64 * effort_multiplier(effort);
    Duration::from_secs_f64(MAX_RUN_FIXED_SECS as f64 + generation.round())
}

// ── Model discovery & health ────────────────────────────────────────────────────

/// Listing models / validating a key (`list_models`, `test_key`): a quick GET to
/// the provider's model catalog or tags endpoint. Applied PER REQUEST.
pub const LIST_MODELS: Duration = Duration::from_secs(10);

/// Cumulative deadline across a PAGINATED `list_models` fetch (Anthropic's
/// `after_id`/Gemini's `pageToken` cursor loop, capped at
/// `MAX_LIST_MODELS_PAGES`) — bounds the WHOLE fetch, not any single request,
/// so a misbehaving/slow provider can't chain up to `MAX_LIST_MODELS_PAGES`
/// individual [`LIST_MODELS`] timeouts into one very long invoke (50 × 10s =
/// 500s, worst case, without this).
pub const LIST_MODELS_TOTAL: Duration = Duration::from_secs(30);

/// Local Ollama reachability probe (`reachable_model`): the fast health check
/// behind the system-health panel, kept short so an unreachable daemon fails fast.
pub const HEALTH: Duration = Duration::from_secs(3);

/// Inspecting a local Ollama model (`/api/show`): fetch a model's trained context
/// length and size labels.
pub const OLLAMA_SHOW: Duration = Duration::from_secs(15);

/// Pulling (downloading) a local Ollama model (`/api/pull`): a large multi-GB
/// download streamed with progress, hence the hour-long ceiling.
pub const MODEL_PULL: Duration = Duration::from_secs(3600);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_deadline_matches_the_baseline_for_no_or_low_effort() {
        assert_eq!(stream_deadline(None), STREAM);
        assert_eq!(stream_deadline(Some("")), STREAM);
        assert_eq!(stream_deadline(Some("minimal")), STREAM);
        assert_eq!(stream_deadline(Some("low")), STREAM);
    }

    #[test]
    fn stream_deadline_scales_up_for_higher_effort() {
        assert_eq!(stream_deadline(Some("medium")), Duration::from_secs(450));
        assert_eq!(stream_deadline(Some("high")), Duration::from_secs(600));
        // `xhigh` then `max` — vendors' ascending order, see `effort_multiplier`.
        assert_eq!(stream_deadline(Some("xhigh")), Duration::from_secs(750));
        assert_eq!(stream_deadline(Some("max")), Duration::from_secs(900));
    }

    /// A caller must be able to trust the ordering, not just the individual
    /// values — this is what "scales with effort" actually promises.
    ///
    /// The array below must stay in the VENDORS' ascending tier order
    /// (`… high < xhigh < max`), not in the order the match arms happen to be
    /// written. A previous version listed `max` before `xhigh`, which made this
    /// test pass against a table that gave the top tier the shortest deadline.
    #[test]
    fn stream_deadline_is_monotonically_nondecreasing_by_effort_tier() {
        let tiers = [
            None,
            Some("minimal"),
            Some("low"),
            Some("medium"),
            Some("high"),
            Some("xhigh"),
            Some("max"),
        ];
        let mut prev = Duration::from_secs(0);
        for effort in tiers {
            let d = stream_deadline(effort);
            assert!(
                d >= prev,
                "stream_deadline({effort:?}) = {d:?} must be >= the previous tier's {prev:?}"
            );
            prev = d;
        }
    }

    /// An effort string outside the known vocabulary must fall back to the
    /// baseline — never explode a typo/future-provider string into an
    /// unbounded multiplier.
    #[test]
    fn stream_deadline_falls_back_to_baseline_for_an_unrecognized_effort_string() {
        assert_eq!(stream_deadline(Some("ultra-mega-think")), STREAM);
    }

    // ── research_deadline ───────────────────────────────────────────────────
    //
    // Same contract as `stream_deadline`, and for the same reason: the thing it
    // bounds ends in a model call. A flat 25s here meant every research call in
    // a reported reasoning-model session timed out, and each cover letter was
    // written with no company knowledge and no visible failure.

    #[test]
    fn research_deadline_exceeds_the_inner_search_bounds_it_wraps() {
        // It wraps a web search AND a synthesis completion. If the outer bound
        // isn't clear of the inner ones, it fires first and the actionable inner
        // error never surfaces. The old flat 25s was EQUAL to `WEB_SEARCH`.
        assert!(
            research_deadline(None) > WEB_SEARCH,
            "the outer research bound must clear the cloud web-search bound"
        );
        assert!(research_deadline(None) > OLLAMA_WEB_SEARCH);
    }

    #[test]
    fn research_deadline_is_monotonically_nondecreasing_by_effort_tier() {
        // Vendors' ascending order — `max` is the TOP tier, above `xhigh`.
        let tiers = [
            None,
            Some("minimal"),
            Some("low"),
            Some("medium"),
            Some("high"),
            Some("xhigh"),
            Some("max"),
        ];
        let mut prev = Duration::from_secs(0);
        for effort in tiers {
            let d = research_deadline(effort);
            assert!(
                d >= prev,
                "research_deadline({effort:?}) = {d:?} must be >= the previous tier's {prev:?}"
            );
            prev = d;
        }
        // Not vacuously true: the top tier must actually exceed the baseline.
        assert!(research_deadline(Some("max")) > research_deadline(None));
    }

    #[test]
    fn research_deadline_falls_back_to_baseline_for_an_unrecognized_effort_string() {
        assert_eq!(
            research_deadline(Some("ultra-mega-think")),
            RESEARCH_BASELINE
        );
        assert_eq!(research_deadline(None), RESEARCH_BASELINE);
    }

    // ── quality_run_deadline ────────────────────────────────────────────────

    /// The cross-language lock. `packages/shared/src/ai-timeouts.test.ts` pins
    /// the identical seven values against `qualityRunDeadlineSecs`; the two
    /// constants are generated, but the ARITHMETIC is spelled out on both
    /// sides, so only a matched pair of pinned tables catches a drift in the
    /// formula itself. Mutation check: change either side's `fixed + …` to
    /// `baseline × …` and one of the two tables fails.
    #[test]
    fn quality_run_deadline_pins_the_derived_table() {
        for (effort, secs) in [
            (None, 4_500),
            (Some("minimal"), 4_500),
            (Some("low"), 4_500),
            (Some("medium"), 4_650),
            (Some("high"), 4_800),
            (Some("xhigh"), 4_950),
            (Some("max"), 5_100),
        ] {
            assert_eq!(
                quality_run_deadline(effort),
                Duration::from_secs(secs),
                "quality_run_deadline({effort:?})"
            );
        }
    }

    /// The outer bound must clear the inner bounds it wraps, at EVERY tier —
    /// the same rule `research_deadline_exceeds_the_inner_search_bounds_it_wraps`
    /// states.
    ///
    /// The inner bounds are computed from the FAN-OUT CONSTANTS themselves, not
    /// from the deadline's own terms: three JSON stages each allowed one
    /// re-ask, plus `max_repair_attempts × MAX_SECTIONS_PER_ROUND` section
    /// rewrites — all of them `Completer::complete*` calls bounded by the flat
    /// `OLLAMA_COMPLETION` — plus the one streamed draft. That is what makes
    /// this a guard rather than an identity: raising either repair constant
    /// without raising the deadline fails here.
    ///
    /// **One `OLLAMA_COMPLETION` per call, not `MAX_ATTEMPTS` of them.** That
    /// holds only because `retry::send_with_retry` bounds the whole retry
    /// sequence by the caller's timeout; the guard for THAT half lives next to
    /// it (`a_one_shot_call_stops_once_its_own_timeout_is_spent`), because it is
    /// a property of the loop's wall clock, not of this arithmetic. Multiplying
    /// the term by `MAX_ATTEMPTS` here instead would pin a dependency the code
    /// no longer has — an identity of exactly the kind this test was rebuilt to
    /// stop being.
    ///
    /// Mutation checks (applied and reverted): `QUALITY_RUN_FIXED_SECS` back to
    /// 1_800 ⇒ every tier fails; `MAX_SECTIONS_PER_ROUND` 4 → 6 ⇒ every tier
    /// fails; `DEFAULT_MAX_REPAIR_ATTEMPTS` 2 → 3 ⇒ every tier fails.
    #[test]
    fn quality_run_deadline_clears_the_inner_per_call_bounds() {
        const JSON_STAGES: u32 = 3;
        const ROUND_TRIPS_PER_JSON_STAGE: u32 = 2; // the one budgeted re-ask
        let json_half = OLLAMA_COMPLETION * JSON_STAGES * ROUND_TRIPS_PER_JSON_STAGE;
        // The repair fan-out is bounded by the SAME flat per-call constant —
        // `regenerate_one_section` goes through `Completer::complete`, never a
        // stream — so it belongs to the effort-invariant half.
        let repair_half = OLLAMA_COMPLETION
            * crate::pipeline::budget::Budget::RESUME_QUALITY.max_repair_attempts as u32
            * crate::pipeline::resume::stages::MAX_SECTIONS_PER_ROUND as u32;
        for effort in [
            None,
            Some("medium"),
            Some("high"),
            Some("xhigh"),
            Some("max"),
        ] {
            // The draft is the only streamed call the run makes.
            let generation = stream_deadline(effort);
            assert!(
                quality_run_deadline(effort) >= json_half + repair_half + generation,
                "quality_run_deadline({effort:?}) must cover the calls it wraps"
            );
        }
    }

    /// The budget constant is the FLOOR the effort-blind path falls back to, so
    /// the two must agree at the bottom tier — otherwise `run_deadline`'s
    /// `max()` silently picks whichever is larger and the derivation in
    /// `ai-timeouts.ts` stops describing what actually runs.
    #[test]
    fn quality_run_deadline_agrees_with_the_budget_floor_at_the_bottom_tier() {
        assert_eq!(
            quality_run_deadline(None),
            crate::pipeline::budget::Budget::RESUME_QUALITY.run_timeout
        );
    }

    #[test]
    fn quality_run_deadline_is_monotonically_nondecreasing_by_effort_tier() {
        let tiers = [
            None,
            Some("minimal"),
            Some("low"),
            Some("medium"),
            Some("high"),
            Some("xhigh"),
            Some("max"),
        ];
        let mut prev = Duration::from_secs(0);
        for effort in tiers {
            let d = quality_run_deadline(effort);
            assert!(
                d >= prev,
                "quality_run_deadline({effort:?}) = {d:?} < {prev:?}"
            );
            prev = d;
        }
        assert!(quality_run_deadline(Some("max")) > quality_run_deadline(None));
    }

    #[test]
    fn quality_run_deadline_falls_back_to_baseline_for_an_unrecognized_effort_string() {
        assert_eq!(
            quality_run_deadline(Some("ultra-mega-think")),
            quality_run_deadline(None)
        );
    }

    // ── max_run_deadline ────────────────────────────────────────────────────

    /// The max deadline must clear the calls a max run PLANS to make, computed
    /// from the PIPELINE and the fan-out constants rather than from the
    /// deadline's own terms: one call per stage that is neither free nor a
    /// fan-out, one call per section up to `max_sections`, and the full repair
    /// fan-out — every one of them bounded by the flat `OLLAMA_COMPLETION`,
    /// because max depth streams nothing.
    ///
    /// **The stage count is READ OFF `max_pipeline()`, not transcribed.** The
    /// previous `const JSON_STAGES: u32 = 3` could not notice the judge, which
    /// is a fourth single-call stage — so the pin passed on an exact equality
    /// (24 planned calls × 300 s = 7 200 s = `max_run_deadline(None)`) with no
    /// slack at all, and would have kept passing while the deadline was
    /// genuinely short. `Pipeline::free_stage_names` is what makes "does this
    /// stage cost a call" machine-readable; the two fan-out names are the only
    /// stages whose count is not one, and they are named here because their
    /// ceilings are the budget's, not the pipeline's.
    ///
    /// The re-ask is deliberately absent from BOTH sides (see
    /// `MAX_RUN_FIXED_SECS`), so this is a guard on the planned run, not on its
    /// pathological worst case.
    ///
    /// Mutation checks (applied and reverted): `DEFAULT_MAX_SECTIONS` 12 → 14 ⇒
    /// every tier fails; `MAX_SECTIONS_PER_ROUND` 4 → 6 ⇒ every tier fails;
    /// `MAX_RUN_FIXED_SECS` 7_200 → 6_900 (the pre-judge value) ⇒ every tier
    /// fails; add a stage that costs a call to `max_pipeline()` ⇒ every tier
    /// fails.
    #[test]
    fn max_run_deadline_clears_the_inner_per_call_bounds() {
        /// The two stages whose call count is a FAN-OUT rather than one.
        const FANS_OUT: [&str; 2] = ["sections", "repair"];
        let budget = crate::pipeline::budget::Budget::RESUME_MAX;
        let pipeline = crate::pipeline::resume::max_pipeline();
        let free = pipeline.free_stage_names();
        let single_call_stages = pipeline
            .stage_names()
            .into_iter()
            .filter(|stage| !free.contains(stage) && !FANS_OUT.contains(stage))
            .count() as u32;
        assert!(
            FANS_OUT
                .iter()
                .all(|stage| pipeline.stage_names().contains(stage)),
            "a fan-out stage was renamed; its ceiling is no longer being counted"
        );
        let planned = single_call_stages
            + budget.max_sections as u32
            + (budget.max_repair_attempts * crate::pipeline::resume::stages::MAX_SECTIONS_PER_ROUND)
                as u32;
        assert_eq!(
            planned, 24,
            "4 single-call stages + 12 sections + 2×4 repair rewrites"
        );
        let inner = OLLAMA_COMPLETION * planned;
        for effort in [
            None,
            Some("medium"),
            Some("high"),
            Some("xhigh"),
            Some("max"),
        ] {
            assert!(
                max_run_deadline(effort) >= inner,
                "max_run_deadline({effort:?}) must cover the calls a max run plans"
            );
        }
        // …and with room to spare at the bottom tier, so the pin is not sitting
        // on an equality the next constant change silently breaks.
        assert!(
            max_run_deadline(None) > inner,
            "the backstop must clear the planned run, not merely equal it"
        );
    }

    /// The per-tier table, spelled out — the max-depth twin of
    /// `quality_run_deadline_pins_the_derived_table`.
    ///
    /// These five numbers are what the RENDERER's `maxRunDeadlineSecs` has to
    /// EXCEED at every tier (`packages/shared/src/ai-timeouts.ts`), or the
    /// client timeout fires before the backend can say why it stopped. A
    /// derivation change that moves them must move that constant too, and a
    /// table nobody wrote down is a table nobody notices moving.
    #[test]
    fn max_run_deadline_pins_the_derived_table() {
        for (effort, secs) in [
            (None, 7_500u64),
            (Some("minimal"), 7_500),
            (Some("low"), 7_500),
            (Some("medium"), 7_650),
            (Some("high"), 7_800),
            (Some("xhigh"), 7_950),
            (Some("max"), 8_100),
        ] {
            assert_eq!(
                max_run_deadline(effort),
                Duration::from_secs(secs),
                "max_run_deadline({effort:?})"
            );
        }
    }

    /// Same rule as the quality pair: the budget constant is the floor the
    /// effort-blind path falls back to through `resume::run_deadline`'s `max()`,
    /// so a disagreement means one of the two stops describing what runs.
    #[test]
    fn max_run_deadline_agrees_with_the_budget_floor_at_the_bottom_tier() {
        assert_eq!(
            max_run_deadline(None),
            crate::pipeline::budget::Budget::RESUME_MAX.run_timeout
        );
    }

    /// Max depth is strictly more work than quality depth at every tier, so its
    /// deadline may never be the shorter of the two — the inversion
    /// `effort_multiplier`'s own doc records for the `max`/`xhigh` tiers, one
    /// level up.
    #[test]
    fn max_run_deadline_is_never_shorter_than_the_quality_one() {
        for effort in [None, Some("medium"), Some("high"), Some("max")] {
            assert!(
                max_run_deadline(effort) >= quality_run_deadline(effort),
                "max_run_deadline({effort:?}) is shorter than the quality deadline"
            );
        }
    }

    #[test]
    fn max_run_deadline_is_monotonically_nondecreasing_by_effort_tier() {
        let mut prev = Duration::from_secs(0);
        for effort in [
            None,
            Some("minimal"),
            Some("low"),
            Some("medium"),
            Some("high"),
            Some("xhigh"),
            Some("max"),
        ] {
            let d = max_run_deadline(effort);
            assert!(d >= prev, "max_run_deadline({effort:?}) = {d:?} < {prev:?}");
            prev = d;
        }
        assert!(max_run_deadline(Some("max")) > max_run_deadline(None));
    }
}
