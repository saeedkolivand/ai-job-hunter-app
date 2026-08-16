pub mod extractor;

use std::time::Duration;

use tauri::Manager;

use crate::ai_provider::SearchBackend;
use crate::pipeline::cache::KvCache;
use crate::pipeline::enrichment::EnrichmentResult;
use crate::pipeline::resume::cache::StageIdentity;
use crate::pipeline::Completer;

const CACHE_NS: &str = "company_brief";
const TTL_SECS: i64 = 7 * 24 * 3600;

/// Company-research enricher: resolve company → resolve search ROUTE once →
/// cache check → the **active provider's own** web search + brief synthesis
/// (via [`Completer::research_via`]) → cache store. Degrades gracefully —
/// any missing key / unsupported provider / failure / timeout / "no
/// information" result yields an empty brief, never an error, so generation
/// still proceeds. The brief is reused by cover letters **and** application
/// answers.
pub struct CompanyResearch;

impl CompanyResearch {
    /// Research a company. `company_override` — the accurate company name the
    /// generation flow already AI-extracted — takes precedence over the heuristic
    /// job-ad extraction, which frequently grabs a tagline ("…platform built for
    /// the era of agentic commerce") rather than the company. Falls back to the
    /// heuristic extraction only when the override is absent/empty.
    ///
    /// `deadline` bounds the whole pass (search + synthesis) and is INJECTED by
    /// the L3 caller — which derives it from the request's reasoning effort via
    /// `timeouts::research_deadline` — rather than resolved here, for the same
    /// reason `SalaryResearch::enrich` takes its `cache`: this module must not
    /// reach up into `commands` (R7), and an injected bound is directly
    /// testable. A FLAT bound was the bug: synthesis is a model call, so its
    /// cost scales with the model's reasoning budget, and a reasoning model's
    /// research never finished inside the old fixed 25s.
    pub async fn enrich_with(
        &self,
        completer: &Completer,
        job_ad: &str,
        company_override: Option<&str>,
        role_override: Option<&str>,
        deadline: Duration,
    ) -> EnrichmentResult {
        let meta = extractor::extract(job_ad);
        let company = pick(company_override, &meta.company);
        // Same precedence as `company`, and for the same reason: the heuristic's
        // last resort is "the ad's first short line", which on a scraped page is
        // routinely an apply button or a nav link. A reported session searched
        // for role="Jetzt bewerben" and role="[← Alle offenen Stellen](/karriere)".
        let role = pick(role_override, &meta.role);
        if company.is_empty() {
            tracing::debug!("research: no company name available (override + extraction empty)");
            return EnrichmentResult::empty();
        }

        let app = completer.app();
        // ADR-017 key discipline: built from the canonical `StageIdentity` a
        // resolved `Completer` carries, plus the retrieval backend — see
        // `cache_key`'s doc comment for the full term-by-term reasoning
        // (including why `role` is deliberately NOT a term). Without
        // provider + model, a user who switched models kept getting the OLD
        // model's cached brief for the full 7-day TTL.
        //
        // The route is resolved HERE, exactly once, and reused below by
        // `research_via` — never re-resolved (PR #989 CodeRabbit MAJOR: the
        // old shape resolved the backend a SECOND time inside the fetch,
        // after this cache check's `.await`ed timeout; if credentials
        // changed in that window — an Exa key added/removed mid-flight —
        // the key could name a backend that did NOT produce the brief it
        // gets stored under). See `CompanySearchRoute`'s doc comment.
        // No effort concept on this path (search + synthesis, never a `think`
        // request) — `None` here is a real absence, not an omission.
        let identity = StageIdentity::of(completer, None);
        let route = completer.resolve_search_route();
        let key = cache_key(identity, route.backend(), &company);

        // Fast path: cached brief younger than the TTL.
        if let Some(cache) = app.try_state::<KvCache>() {
            if let Some(brief) = cache.get(CACHE_NS, &key, TTL_SECS) {
                // Length only — never the brief itself (ADR-027): it's free web
                // prose about a company, not something the redaction pass
                // (paths/URLs/credentials/hosts/emails) is built to catch, and
                // this line fires on every staged run and ships in the
                // diagnostics bundle.
                tracing::info!(
                    company = %company,
                    source = "cache",
                    chars = brief.len(),
                    "research: company brief"
                );
                return EnrichmentResult {
                    key: company,
                    content: brief,
                };
            }
        }

        // Charge the per-provider DAILY ceiling only NOW — right before the
        // real provider call, after the cache check above has already come
        // back empty. `Completer::admit_research` (both this crate's and
        // the résumé pipeline's callers admit through it) only acquires the
        // rate/concurrency slot; charging the daily budget any earlier —
        // the pre-fix shape — burned a day's allowance on every cache hit,
        // since a hit never reaches this line at all.
        if let Err(e) = completer.charge_daily() {
            tracing::debug!("research: daily budget exceeded for {company}: {e}");
            return EnrichmentResult {
                key: company,
                content: String::new(),
            };
        }

        // Provider-native research, bounded so generation never stalls. Any
        // failure / timeout / unconfigured provider yields an empty brief.
        // `route` (resolved above, before the cache check) is MOVED in here —
        // the only way to get a brief is along the exact route the key above
        // was named from; there is no second resolution to diverge from it.
        let brief =
            match tokio::time::timeout(deadline, completer.research_via(route, &company, &role))
                .await
            {
                Ok(Ok(b)) => b,
                Ok(Err(e)) => {
                    tracing::warn!("research: provider research failed for {company}: {e}");
                    String::new()
                }
                Err(_) => {
                    tracing::warn!(
                        company = %company,
                        deadline_secs = deadline.as_secs(),
                        "research: timed out"
                    );
                    String::new()
                }
            };

        // Drop unhelpful "no information" / too-short responses so they neither
        // pollute the cover letter nor get cached (a bad miss must not stick for
        // the 7-day TTL).
        if is_no_info(&brief) {
            // Length only — same discipline as the two `tracing::info!` calls
            // above/below (ADR-027); this fires on the provider's own filler
            // text, not attacker input, but it is still not ours to log.
            tracing::info!(
                company = %company,
                chars = brief.len(),
                "research: no usable brief (provider found nothing)"
            );
            return EnrichmentResult {
                key: company,
                content: String::new(),
            };
        }

        // Length only — never the brief itself (ADR-027): it's free web prose
        // about a company, and this line fires on every staged run with the
        // research toggle on and ships in the diagnostics bundle.
        tracing::info!(
            company = %company,
            role = %role,
            source = "provider",
            chars = brief.len(),
            "research: company brief"
        );
        if let Some(cache) = app.try_state::<KvCache>() {
            cache.set(CACHE_NS, &key, &brief);
        }

        EnrichmentResult {
            key: company,
            content: brief,
        }
    }
}

/// The AI-extracted value when it is non-empty, else the heuristic one. The
/// AI extraction sees the whole ad and is simply better; the heuristic exists
/// for the paths that have no extraction to offer.
fn pick(override_value: Option<&str>, heuristic: &str) -> String {
    override_value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(heuristic)
        .to_string()
}

/// Field separator between key terms — a control character, not printable,
/// so no provider id, model name, backend id or company name can contain it
/// and shift field boundaries. Mirrors
/// [`pipeline::resume::cache`](crate::pipeline::resume::cache)'s own
/// `FIELD_SEPARATOR`, same reasoning.
const FIELD_SEPARATOR: char = '\u{1f}';

/// Build the `company_brief` cache key from the [`StageIdentity`] a resolved
/// [`Completer`] carries — reused wholesale, not re-derived (see the
/// cross-reference on [`StageIdentity`] itself), plus the two terms that
/// identity type doesn't carry but this call DOES depend on:
///
/// * `backend` ([`SearchBackend`]) — the retrieval half of the brief. The
///   backend that answers a research pass is resolved from CREDENTIAL
///   PRESENCE, not from `(provider, model)` alone: the same provider and
///   model can retrieve from a different backend (an Exa key added or
///   removed) and must not share a cache row. Callers MUST pass
///   `route.backend()` from the SAME `CompanySearchRoute` the fetch itself
///   consumes (see its doc comment on `commands::ai_provider::search`) —
///   resolving separately for the key and for the fetch is the bug PR #989
///   fixed.
/// * `company` — the research subject.
///
/// `identity.context_window` is folded in even though
/// [`Completer::research_via`] never reads it (search + synthesize doesn't touch
/// `num_ctx`) — reusing the WHOLE identity keeps this key from silently
/// drifting the day a new `StageIdentity` field starts mattering here too,
/// at the cost of a harmless extra miss on a window-only change (ADR-017's
/// own safe direction: a changed key can only cost a call, never serve a
/// wrong answer).
///
/// `role` is deliberately NOT a term, even though it reaches the synthesis
/// prompt: the brief is reused across roles at the same company by design
/// (this module's doc comment — "reused by cover letters **and**
/// application answers") — one paid search per company per (identity,
/// backend), not per role. That is the documented cost tradeoff, not an
/// oversight.
///
/// `company` is passed verbatim, not lowercased: `kv_cache`'s `key` column is
/// `COLLATE NOCASE` ([`pipeline::cache`](crate::pipeline::cache)), so the
/// storage layer already matches case-insensitively — folding it here would
/// be redundant, not a behavior change. Pure + unit-tested.
fn cache_key(identity: StageIdentity<'_>, backend: SearchBackend, company: &str) -> String {
    let window = identity
        .context_window
        .map(|c| c.to_string())
        .unwrap_or_default();
    format!(
        "{}{FIELD_SEPARATOR}{}{FIELD_SEPARATOR}{window}{FIELD_SEPARATOR}{}{FIELD_SEPARATOR}{company}",
        identity.provider,
        identity.model,
        backend.as_str(),
    )
}

/// A brief the model couldn't actually fill: too short to be a real ~150-word
/// brief, or an explicit "no information" disclaimer. Treated as empty. Pure +
/// unit-tested.
fn is_no_info(brief: &str) -> bool {
    let b = brief.trim().to_lowercase();
    b.len() < 60
        || b.contains("no information")
        || b.contains("not available")
        || b.contains("couldn't find")
        || b.contains("could not find")
        || b.contains("unable to find")
        || b.contains("no relevant")
}

// This suite guards the `cache_key` builder and the `KvCache` storage layer
// it writes through — NOT the `enrich_with` wiring that calls
// `StageIdentity::of(completer)` / `completer.resolve_search_route()` /
// `completer.research_via(route, ..)`. This crate has no `tauri::test`
// mock-app harness (see `SalaryResearch::enrich`'s doc comment for the same
// limitation), so that wiring — including the "route is resolved exactly
// once and reused" invariant PR #989 fixed — is unverified END TO END at the
// unit level here; `commands::ai_provider::search::test`'s
// `resolve_via_backend` tests cover the pure half of that fix (the
// tag-to-searcher pairing) instead. An honest gap, not a fix skipped.
#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::{cache_key, is_no_info, SearchBackend, StageIdentity, CACHE_NS, TTL_SECS};
    use crate::pipeline::cache::KvCache;

    /// A routing identity, for the key tests below — same helper shape as
    /// `pipeline::resume::test::id`.
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
    /// Same source-position technique `pipeline::resume::test`'s sibling
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
}
