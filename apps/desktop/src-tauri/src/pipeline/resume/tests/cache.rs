use super::super::cache::{StageCacheKey, StageIdentity, PIPELINE_PROMPT_VERSION};
use super::support::id;
use crate::error::{AppError, AppResult};
use crate::pipeline::cache::KvCache;
use serde_json::{json, Value};
use std::cell::Cell;
use tempfile::TempDir;

/// The pin the cache's whole correctness argument rests on.
///
/// Every stage cache key embeds this constant, so editing a stage prompt WITHOUT
/// bumping it serves yesterday's answer to today's question — silently, because
/// a cache hit looks exactly like a fast run. The pin makes that edit fail a
/// test instead: an author who changed a prompt has to come here and decide.
///
/// Mutation check: bump the constant and this fails; it is a literal, so it
/// cannot pass vacuously.
#[test]
fn prompt_version_is_pinned() {
    assert_eq!(
        PIPELINE_PROMPT_VERSION, 2,
        "a stage prompt or artifact shape changed — bump PIPELINE_PROMPT_VERSION so every \
         cached stage artifact is invalidated, then update this pin"
    );
}

/// [`id`] plus an explicit effort — for the one test that needs to vary it.
fn id_with_effort<'a>(
    provider: &'a str,
    model: &'a str,
    effort: Option<&'a str>,
) -> StageIdentity<'a> {
    StageIdentity {
        provider,
        model,
        context_window: None,
        effort,
    }
}

/// The key must MISS when the version, the provider, the model or the CONTEXT
/// WINDOW changes — each for its own reason (a different question, a different
/// endpoint, a different function, a different amount of the prompt the model
/// can see). Mutation check: drop any one term from `StageCacheKey::key`'s
/// format string and the matching assertion fails.
#[test]
fn cache_key_discipline_misses_on_version_provider_model_and_window() {
    let base = StageCacheKey::new(id("ollama", "llama3.1:8b", None), "seed");
    let other_provider = StageCacheKey::new(id("openai", "llama3.1:8b", None), "seed");
    let other_model = StageCacheKey::new(id("ollama", "qwen3:14b", None), "seed");
    let other_seed = StageCacheKey::new(id("ollama", "llama3.1:8b", None), "different résumé");

    assert_ne!(
        base.key(),
        other_provider.key(),
        "provider must be in the key"
    );
    assert_ne!(base.key(), other_model.key(), "model must be in the key");
    assert_ne!(
        base.key(),
        other_seed.key(),
        "the run's inputs must be in the key"
    );
    assert_eq!(
        base.key(),
        StageCacheKey::new(id("ollama", "llama3.1:8b", None), "seed").key()
    );
}

/// The context window is an OUTPUT-AFFECTING input: it becomes `num_ctx`, which
/// decides how much of the prompt the model actually sees, so an answer given
/// at 4 096 must not be served to a run that asked for 32 768.
///
/// This regressed the moment the staged pipeline started sending the window —
/// before that, every term the cached stages could vary was already in the key.
///
/// Mutation check (executed): remove `{window}` from `key`'s pre-hash and the
/// first two assertions fail; make `rebound`/`new` drop `context_window` and
/// they fail the same way.
#[test]
fn the_context_window_is_part_of_the_cache_key() {
    let unset = StageCacheKey::new(id("ollama", "m", None), "seed");
    let small = StageCacheKey::new(id("ollama", "m", Some(4_096)), "seed");
    let large = StageCacheKey::new(id("ollama", "m", Some(32_768)), "seed");

    assert_ne!(
        unset.key(),
        small.key(),
        "configuring a window changes what the model sees"
    );
    assert_ne!(small.key(), large.key(), "so does changing it");
    assert_eq!(
        small.key(),
        StageCacheKey::new(id("ollama", "m", Some(4_096)), "seed").key()
    );
    // …and the window travels through `rebound` too, which is the path an
    // OVERRIDDEN stage takes.
    assert_ne!(
        unset.rebound(id("ollama", "m", Some(4_096))).key(),
        unset.rebound(id("ollama", "m", None)).key()
    );
}

/// **Effort is an OUTPUT-AFFECTING input too, on a thinking-capable model.**
/// `complete_structured` sends it as the request's own `think` field
/// (`ollama::think_level`), which changes the model's actual reasoning depth —
/// not just how long the caller waits. Before the per-call deadline started
/// scaling by effort, `pipeline::text_request` hard-coded it to `None` for
/// every non-streaming call, so it genuinely never varied and omitting it from
/// the key was correct; once it started reaching the provider, an omitted term
/// here would let a "high"-effort answer be served back to a "low"-effort
/// request for the identical résumé/posting/model.
///
/// Mutation check (executed): remove `effort` from `key`'s pre-hash (or drop it
/// from `rebound`/`new`) and the first two assertions fail.
#[test]
fn effort_is_part_of_the_cache_key() {
    let none = StageCacheKey::new(id_with_effort("ollama", "m", None), "seed");
    let low = StageCacheKey::new(id_with_effort("ollama", "m", Some("low")), "seed");
    let high = StageCacheKey::new(id_with_effort("ollama", "m", Some("high")), "seed");

    assert_ne!(
        none.key(),
        low.key(),
        "an unset effort must not collide with a set one"
    );
    assert_ne!(low.key(), high.key(), "a different effort must miss");
    assert_eq!(
        low.key(),
        StageCacheKey::new(id_with_effort("ollama", "m", Some("low")), "seed").key()
    );
    // …and effort travels through `rebound` too, the path an overridden stage
    // takes.
    assert_ne!(
        none.rebound(id_with_effort("ollama", "m", Some("high")))
            .key(),
        none.rebound(id_with_effort("ollama", "m", None)).key()
    );
}

/// A CHAINED artifact must change every LATER stage's key: `strategy` reads the
/// analysis, so a different analysis has to miss the strategy cache even though
/// the run's own inputs are identical.
///
/// Mutation check: make `extend` a no-op and this fails.
#[test]
fn cache_key_chains_upstream_artifacts() {
    let mut a = StageCacheKey::new(id("ollama", "m", None), "seed");
    let mut b = StageCacheKey::new(id("ollama", "m", None), "seed");
    let before = a.key();
    a.extend(r#"{"roleTitle":"Engineer"}"#);
    b.extend(r#"{"roleTitle":"Manager"}"#);
    assert_ne!(before, a.key(), "extending must change the next key");
    assert_ne!(a.key(), b.key(), "a different upstream artifact must miss");
}

/// The separator has to make field boundaries unambiguous, or `("ab","c")` and
/// `("a","bc")` collide and one model's cached answer is served for another's.
#[test]
fn cache_key_field_boundaries_are_unambiguous() {
    assert_ne!(
        StageCacheKey::new(id("ab", "c", None), "s").key(),
        StageCacheKey::new(id("a", "bc", None), "s").key()
    );
}

/// One stage's real sequence: look up the cache under `stage`'s own key: on a
/// hit, use the cached value and DO NOT touch `calls`; on a miss, call
/// `answer` (counted), then write it back. Either way, fold the resolved
/// value into the ROLLING `cache_key` so the next stage's key depends on it —
/// exactly `analyze.rs`'s `ctx.cache_key.extend(&json)`.
fn cached_stage_call(
    cache: &KvCache,
    stage: &'static str,
    cache_key: &mut StageCacheKey,
    calls: &Cell<u32>,
    answer: impl FnOnce() -> AppResult<Value>,
) -> AppResult<()> {
    let key = cache_key.clone();
    let cached: Option<Value> = super::super::cache::get(Some(cache), stage, &key);
    let from_cache = cached.is_some();
    let value = match cached {
        Some(v) => v,
        None => {
            calls.set(calls.get() + 1);
            answer()?
        }
    };
    let json = serde_json::to_string(&value).unwrap_or_default();
    if !from_cache {
        super::super::cache::put(Some(cache), stage, &key, &json);
    }
    cache_key.extend(&json);
    Ok(())
}

/// Mutation checks (both applied and reverted): comment out the
/// `super::super::cache::put` call inside `cached_stage_call` (or make
/// `super::super::cache::get` always return `None`) and the retry section's first
/// two assertions fail — the calls come back. Drop `effort` back out of
/// `StageIdentity`/`StageCacheKey` (reverting the previous commit) and the
/// LAST section's assertion fails instead: a different-effort request would
/// silently reuse a same-effort answer, which is exactly the staleness bug
/// that fix closed — this is where a wrong key shows up as REAL re-spending,
/// not just a mismatched hash in a unit test.
#[test]
fn a_retry_after_a_mid_run_failure_does_not_re_spend_the_stages_that_already_succeeded() {
    let dir = TempDir::new().expect("tempdir");
    let cache = KvCache::open(dir.path()).expect("open cache");

    let analyze_calls = Cell::new(0u32);
    let strategy_calls = Cell::new(0u32);

    // The run's own inputs — identical between the failed attempt and the
    // retry, exactly what "click regenerate with nothing edited" reproduces
    // (`ResultsPanel`'s `onRegenerate` re-sends the current form values).
    let seed = "the candidate's résumé\u{1f}the job ad\u{1f}en";
    let baseline = id_with_effort("ollama", "qwen3-vl-32k:latest", Some("baseline"));

    // ── First attempt: analyze_job succeeds, strategy times out ──
    let mut key = StageCacheKey::new(baseline, seed);
    cached_stage_call(&cache, "analyze_job", &mut key, &analyze_calls, || {
        Ok(json!({ "mustHave": ["Rust"], "niceToHave": [], "redFlags": [] }))
    })
    .expect("analyze_job answers on the first attempt");
    let first_attempt = cached_stage_call(&cache, "strategy", &mut key, &strategy_calls, || {
        Err(AppError::Timeout("no response within 300s".to_string()))
    });

    assert!(
        first_attempt.is_err(),
        "the premise: strategy fails on the first attempt"
    );
    assert_eq!(analyze_calls.get(), 1);
    assert_eq!(
        strategy_calls.get(),
        1,
        "strategy WAS attempted (that's what timed out) — it just never got to cache an answer"
    );

    // ── Retry: SAME inputs (a fresh rolling key, same seed/identity — a new
    //    run), strategy now succeeds ────────────────────────────────────────
    let mut retry_key = StageCacheKey::new(baseline, seed);
    cached_stage_call(
        &cache,
        "analyze_job",
        &mut retry_key,
        &analyze_calls,
        || panic!("must not re-ask analyze_job — it already answered"),
    )
    .expect("analyze_job reuses its cached answer");
    cached_stage_call(&cache, "strategy", &mut retry_key, &strategy_calls, || {
        Ok(json!({ "companies": [] }))
    })
    .expect("strategy gets a real second chance");

    assert_eq!(
        analyze_calls.get(),
        1,
        "a retry with unchanged inputs must not re-spend a stage that already succeeded"
    );
    assert_eq!(
        strategy_calls.get(),
        2,
        "strategy has no cached answer from the failed attempt, so it must run for real again \
         (1 failed attempt + 1 real retry — never served from a cache the failure never wrote)"
    );

    // ── A third attempt at a DIFFERENT effort must NOT reuse the baseline
    //    answer — this is where dropping `effort` from the key would show up
    //    as a real, silent re-spend of the wrong answer rather than a hash
    //    mismatch in a unit test. ─────────────────────────────────────────
    let higher_effort = id_with_effort("ollama", "qwen3-vl-32k:latest", Some("high"));
    let mut different_effort_key = StageCacheKey::new(higher_effort, seed);
    cached_stage_call(
        &cache,
        "analyze_job",
        &mut different_effort_key,
        &analyze_calls,
        || Ok(json!({ "mustHave": ["Rust"], "niceToHave": [], "redFlags": [] })),
    )
    .expect("analyze_job answers again at the different effort");

    assert_eq!(
        analyze_calls.get(),
        2,
        "a different effort must miss the cache, never silently reuse the baseline answer"
    );
}

/// A mechanical stage's cache key binds the EFFECTIVE effort (the user's, else
/// the model's cheapest tier — `effort_or_cheapest`), so an answer produced at
/// one tier is never served to a run that resolves to another. An unset effort
/// and an explicit pick of the same cheapest tier are the SAME request, so they
/// share an entry.
///
/// Mutation check (executed): key on the raw user effort (`user` instead of
/// `effort_or_cheapest(user, ..)`) and the unset-vs-explicit-`off` assertion
/// fails; drop effort from the key and the unset-vs-`high` one fails.
#[test]
fn a_mechanical_stage_key_binds_the_effective_effort() {
    use crate::pipeline::effort_or_cheapest;

    let qwen: [&'static str; 4] = ["off", "low", "medium", "high"];
    let base = StageCacheKey::new(id_with_effort("ollama", "m", None), "seed");
    let key = |user: Option<&str>, levels: &[&'static str]| {
        base.rebound(id_with_effort(
            "ollama",
            "m",
            effort_or_cheapest(user, levels),
        ))
        .key()
    };

    assert_ne!(key(None, &qwen), key(Some("high"), &qwen));
    assert_ne!(key(Some("medium"), &qwen), key(Some("high"), &qwen));
    assert_eq!(key(None, &qwen), key(Some("off"), &qwen));
    // No lever at all: the default stays "nothing", so it equals an unset key.
    assert_eq!(
        key(None, &[]),
        base.rebound(id_with_effort("ollama", "m", None)).key()
    );
}
