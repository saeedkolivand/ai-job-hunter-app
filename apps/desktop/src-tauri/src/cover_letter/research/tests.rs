use tempfile::TempDir;

use super::{
    attributable_backend, cache_key, is_no_info, settle, FailureKind, ResearchStatus,
    SearchBackend, StageIdentity, CACHE_NS, TTL_SECS,
};
use crate::error::AppError;
use crate::pipeline::cache::KvCache;

/// A routing identity, for the key tests below — same helper shape as
/// `pipeline::resume::tests::support::id`.
fn identity<'a>(provider: &'a str, model: &'a str) -> StageIdentity<'a> {
    StageIdentity {
        provider,
        model,
        context_window: None,
        effort: None,
    }
}

// ── cache_key (ADR-017: identity + search backend are cache-key terms) ──

#[test]
fn cache_key_differs_by_model_so_a_model_switch_never_hits_the_old_brief() {
    let ollama_a = cache_key(identity("ollama", "llama3"), SearchBackend::Native, "Acme");
    let ollama_b = cache_key(
        identity("ollama", "gpt-oss:20b"),
        SearchBackend::Native,
        "Acme",
    );
    assert_ne!(
        ollama_a, ollama_b,
        "same provider + company but a different model must be a different key"
    );
}

#[test]
fn cache_key_differs_by_provider_for_the_same_model_name() {
    let openai = cache_key(identity("openai", "gpt-4o"), SearchBackend::Native, "Acme");
    let compatible = cache_key(
        identity("openai-compatible", "gpt-4o"),
        SearchBackend::Native,
        "Acme",
    );
    assert_ne!(openai, compatible);
}

#[test]
fn cache_key_differs_by_search_backend_for_the_same_provider_and_model() {
    // The defect requirement #2 fixes: `searcher_for` resolves Native vs.
    // Exa from CREDENTIAL PRESENCE at call time, not from (provider,
    // model) — so the SAME provider + model (e.g. Ollama with no
    // ollama.com account key) must still get a different key when the
    // Exa key is added/removed/absent, since the retrieval channel (and
    // therefore the brief) changed.
    let id = identity("ollama", "llama3");
    let native = cache_key(id, SearchBackend::Native, "Acme");
    let exa = cache_key(id, SearchBackend::Exa, "Acme");
    let none = cache_key(id, SearchBackend::None, "Acme");
    assert_ne!(native, exa);
    assert_ne!(native, none);
    assert_ne!(exa, none);
}

#[test]
fn cache_key_is_identical_for_the_same_identity_backend_and_company() {
    assert_eq!(
        cache_key(identity("ollama", "llama3"), SearchBackend::Native, "Acme"),
        cache_key(identity("ollama", "llama3"), SearchBackend::Native, "Acme")
    );
}

// ── KvCache round-trip: proves the fix at the storage layer, not just the
// key-builder in isolation — this is what `enrich_with` actually does. ───

#[test]
fn switching_models_misses_the_other_models_cached_brief() {
    let dir = TempDir::new().expect("tempdir");
    let cache = KvCache::open(dir.path()).expect("open cache");
    let old_key = cache_key(identity("ollama", "llama3"), SearchBackend::Native, "Acme");
    cache.set(CACHE_NS, &old_key, "Acme is a fintech (llama3's brief).");

    // The defect this fixes: before, both models shared the SAME row
    // (keyed on company alone), so switching models kept serving the old
    // model's brief for the whole 7-day TTL instead of recomputing.
    let new_key = cache_key(
        identity("ollama", "gpt-oss:20b"),
        SearchBackend::Native,
        "Acme",
    );
    assert_eq!(
        cache.get(CACHE_NS, &new_key, TTL_SECS),
        None,
        "a different model must MISS the other model's cached brief, forcing a fresh compute"
    );
}

#[test]
fn the_same_model_still_hits_its_own_cached_brief() {
    let dir = TempDir::new().expect("tempdir");
    let cache = KvCache::open(dir.path()).expect("open cache");
    let key = cache_key(identity("ollama", "llama3"), SearchBackend::Native, "Acme");
    cache.set(CACHE_NS, &key, "Acme is a fintech (llama3's brief).");

    assert_eq!(
        cache.get(CACHE_NS, &key, TTL_SECS),
        Some("Acme is a fintech (llama3's brief).".to_string())
    );
}

#[test]
fn is_no_info_flags_empty_short_and_disclaimers() {
    assert!(is_no_info(""));
    assert!(is_no_info("No information available."));
    assert!(is_no_info("  Unable to find details about this company.  "));
    assert!(is_no_info("I could not find any relevant information."));
}

#[test]
fn is_no_info_accepts_a_real_brief() {
    let brief = "Acme is a Series B fintech (≈200 employees) building payment \
        infrastructure for marketplaces. Its core product processes split \
        payouts for platforms; notable customers include several large \
        gig-economy apps. Recently raised funding to expand into Europe, which \
        is relevant for a backend engineer joining the payments team.";
    assert!(!is_no_info(brief));
}

/// The daily-budget-on-cache-hit fix: `enrich_with` must check its cache
/// and return on a hit BEFORE it ever reaches `completer.charge_daily()`.
/// Same source-position technique `pipeline::resume::tests`'s sibling
/// `research_company_brief_has_no_fallible_operator_...` test uses for
/// this crate's other AppHandle-requiring, harness-less research code —
/// an honest structural guard, not a substitute for an integration test
/// this crate has no `tauri::test` harness to write.
#[test]
fn enrich_with_checks_the_cache_before_charging_the_daily_budget() {
    let source = include_str!("mod.rs");
    let start = source
        .find("pub async fn enrich_with")
        .expect("enrich_with must exist");
    let body = &source[start..];
    let cache_check_pos = body
        .find("cache.get(CACHE_NS")
        .expect("enrich_with must check its cache");
    let charge_pos = body
        .find("completer.charge_daily()")
        .expect("enrich_with must charge the daily ceiling before a real provider call");
    assert!(
        cache_check_pos < charge_pos,
        "enrich_with must check its cache BEFORE charging the daily budget — a cache \
         hit must never spend a day's provider allowance: cache check at byte \
         {cache_check_pos}, daily charge at byte {charge_pos}"
    );
}

// ── settle (#1500: a failed lookup is not an empty one) ──

const REAL_BRIEF: &str = "Example Corp builds payment infrastructure for mid-size merchants,     competes with larger processors, and is currently expanding its fraud tooling.";

/// The reported case: an out-of-credit account must read `failed` + `billing`, never `empty`.
/// Mutation check: map `Some(Err(_))` to `Empty` (the old behaviour) and the first case fails.
#[test]
fn a_provider_error_is_failed_with_its_kind_not_empty() {
    let (brief, status) = settle(Some(Err(AppError::Network(
        "ollama web_search 402 Payment Required: add credits".into(),
    ))));
    assert!(brief.is_empty());
    assert_eq!(status, ResearchStatus::Failed(Some(FailureKind::Billing)));
    assert_eq!(
        status.wire(),
        serde_json::json!({ "status": "failed", "errorKind": "billing" })
    );

    let (_, key) = settle(Some(Err(AppError::Network(
        "ollama web_search 401 Unauthorized".into(),
    ))));
    assert_eq!(key, ResearchStatus::Failed(Some(FailureKind::Auth)));
}

#[test]
fn a_timeout_is_failed_unavailable() {
    assert_eq!(
        settle(None).1,
        ResearchStatus::Failed(Some(FailureKind::Unavailable))
    );
}

#[test]
fn an_answer_with_nothing_in_it_is_empty_and_a_real_brief_is_ok() {
    assert_eq!(settle(Some(Ok(String::new()))).1, ResearchStatus::Empty);
    assert_eq!(
        settle(Some(Ok("No information found.".into()))).1,
        ResearchStatus::Empty
    );
    let (brief, status) = settle(Some(Ok(REAL_BRIEF.to_string())));
    assert_eq!((brief.as_str(), status), (REAL_BRIEF, ResearchStatus::Ok));
    assert_eq!(
        ResearchStatus::Empty.wire(),
        serde_json::json!({ "status": "empty" })
    );
}

/// A deadline names no backend (the one in flight had not answered); otherwise the backend whose
/// search last returned stays named, even when synthesis then failed. Mutation check: ignore
/// `timed_out` and the first assertion fails.
#[test]
fn a_backend_is_named_unless_the_deadline_fired() {
    let served = crate::ai_provider::ServedBackend::default();
    assert_eq!(
        attributable_backend(false, &served),
        None,
        "nothing returned yet"
    );
    // Native FAILED (billing), a fallback is in flight, the deadline fires: name none.
    served.set(SearchBackend::Native, false);
    assert_eq!(attributable_backend(true, &served), None);
    assert_eq!(
        attributable_backend(false, &served),
        Some(SearchBackend::Native),
        "a finished pass still names the last backend it reached"
    );
    // SearXNG's search returned Ok and synthesis then hit the deadline: keep it.
    served.set(SearchBackend::Searxng, true);
    assert_eq!(
        attributable_backend(true, &served),
        Some(SearchBackend::Searxng)
    );
    served.set(SearchBackend::Native, true);
    assert_eq!(
        attributable_backend(false, &served),
        Some(SearchBackend::Native)
    );
    // Synthesis (the user's own model) failing is classified from ITS error, not the backend's.
    let (_, status) = settle(Some(Err(AppError::Billing("model out of credit".into()))));
    assert_eq!(status, ResearchStatus::Failed(Some(FailureKind::Billing)));
}
