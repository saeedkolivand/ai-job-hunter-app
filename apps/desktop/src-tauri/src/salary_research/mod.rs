//! Web-grounded salary-range research for the salary application question (C2).
//!
//! Mirrors [`crate::cover_letter::research::CompanyResearch`]: resolve → cache
//! check → the **active provider's own** web search (via
//! [`crate::pipeline::Completer::research_salary`]) → cache store. The one
//! difference that matters most here: the provider's raw response is **never**
//! trusted prose — it is parsed into a small JSON shape and every field is
//! strictly validated before a [`SalaryRange`] exists at all. Only that
//! validated struct (two integers + a currency code) ever reaches the prompt
//! layer, which is the core defense against prompt injection via web content
//! (OWASP LLM01) for this feature. Degrades gracefully — `None` on any missing
//! role / cache miss / provider failure / timeout / parse or validation
//! failure — so the salary answer always falls back to the C1
//! applicant-preference-only grounding.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::pipeline::cache::KvCache;
use crate::pipeline::Completer;

const CACHE_NS: &str = "salary_range";
const TTL_SECS: i64 = 7 * 24 * 3600;
/// Hard cap on a single research call so generation never stalls on a slow or
/// hung provider search. Same bound as `CompanyResearch::RESEARCH_TIMEOUT_SECS`.
const RESEARCH_TIMEOUT_SECS: u64 = 25;
/// Sanity ceiling on an annual salary figure, in any currency's minor-unit-free
/// face value — comfortably above any real annual salary, so a wildly
/// hallucinated figure is rejected rather than reaching the prompt.
const MAX_PLAUSIBLE_SALARY: u64 = 100_000_000;
/// Cap on each of `role`/`company`/`location` (chars, so always a valid UTF-8
/// boundary) before it reaches the cache key or a provider query — a caller
/// passing something absurdly long can't inflate the key or the outbound
/// search query.
const MAX_INPUT_CHARS: usize = 200;

/// A validated market salary range for a role (optionally scoped to a company
/// and/or location), in a single currency. Every field is validated by
/// [`parse_and_validate`] before this struct is constructed — never build one
/// directly from unparsed provider output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SalaryRange {
    pub min: u32,
    pub max: u32,
    /// ISO-4217-shaped currency code (3-4 ASCII letters, upper-cased). Not
    /// validated against the real ISO-4217 list — just shape-checked, since the
    /// model may occasionally return a locale-conventional 4-letter code.
    pub currency: String,
}

/// Abstraction over "search the web for a salary range for this role" — the
/// dependency [`SalaryResearch::enrich`] needs, injected rather than reached
/// for via `completer.app().try_state()`. [`Completer`] is the sole production
/// implementation (a thin forward to its own `research_salary`); tests supply
/// a canned fake, which is the only way to exercise `enrich`'s parse/validate/
/// cache/timeout logic without a live `AppHandle` (this crate has no
/// `tauri::test` mock-app harness). A native (non-`async-trait`, unboxed)
/// return-position `impl Future`, used only through the generic bound on
/// `enrich` (never as `dyn SalarySearcher`) — `+ Send` is spelled out
/// explicitly (rather than plain `async fn` sugar) because a tauri command's
/// future is spawned onto the runtime and must be `Send`, and bare `async fn`
/// in a trait leaves that unspecified (rustc's `async_fn_in_trait` lint).
pub trait SalarySearcher {
    fn research_salary(
        &self,
        role: &str,
        company: &str,
        location: &str,
    ) -> impl std::future::Future<Output = AppResult<String>> + Send;
}

impl SalarySearcher for Completer {
    async fn research_salary(
        &self,
        role: &str,
        company: &str,
        location: &str,
    ) -> AppResult<String> {
        Completer::research_salary(self, role, company, location).await
    }
}

/// Web-grounded salary-range enricher. Same shape as `CompanyResearch`, but
/// returns a validated structured range instead of prose.
pub struct SalaryResearch;

impl SalaryResearch {
    /// Look up the market salary range for `role` (optionally at `company`, in
    /// `location`). `company`/`location` may be empty — the prompt then falls
    /// back to a broader market estimate. Returns `None` (never an error) when
    /// `role` is empty, the searcher can't search, the search/synthesis fails,
    /// times out, or its output doesn't parse into a plausible range.
    ///
    /// `cache` is injected (`None` when the caller has no `KvCache` managed
    /// state) rather than looked up here — the sole production caller
    /// (`commands::ai::ai_lookup_salary`) resolves it once via
    /// `app.try_state::<KvCache>()` and passes it through, which is what keeps
    /// this function testable without an `AppHandle`.
    pub async fn enrich<S: SalarySearcher>(
        &self,
        searcher: &S,
        cache: Option<&KvCache>,
        role: &str,
        company: &str,
        location: &str,
    ) -> Option<SalaryRange> {
        // The very first thing this does — before touching `searcher` at all —
        // so a whitespace-only role never reaches it. Factored to a pure
        // predicate ([`role_is_missing`]) purely so it stays unit-testable in
        // isolation.
        if role_is_missing(role) {
            tracing::debug!("salary_research: no role available, skipping lookup");
            return None;
        }
        let role = truncate_input(role.trim());
        let company = truncate_input(company.trim());
        let location = truncate_input(location.trim());

        // Case-folded so "Berlin"/"berlin" don't miss each other; the
        // case-preserved values above still go to the prompt/query/logging.
        let key = cache_key(&role, &company, &location);

        // Fast path: cached, validated range younger than the TTL.
        if let Some(cache) = cache {
            if let Some(json) = cache.get(CACHE_NS, &key, TTL_SECS) {
                if let Some(range) = parse_and_validate(&json) {
                    tracing::info!(role = %role, company = %company, source = "cache", "salary_research: range");
                    return Some(range);
                }
            }
        }

        // Provider-native research, bounded so generation never stalls. Any
        // failure/timeout yields no range.
        let raw = match tokio::time::timeout(
            Duration::from_secs(RESEARCH_TIMEOUT_SECS),
            searcher.research_salary(&role, &company, &location),
        )
        .await
        {
            Ok(Ok(text)) => text,
            Ok(Err(e)) => {
                tracing::warn!("salary_research: provider research failed for {role}: {e}");
                return None;
            }
            Err(_) => {
                tracing::warn!("salary_research: timed out for {role}");
                return None;
            }
        };

        // `{}` ("no reliable data"), malformed JSON, and any failed validation
        // all fall through here — never cached, so a bad miss doesn't stick for
        // the 7-day TTL.
        let range = parse_and_validate(&raw)?;

        if let Some(cache) = cache {
            if let Ok(json) = serde_json::to_string(&range) {
                cache.set(CACHE_NS, &key, &json);
            }
        }

        tracing::info!(role = %role, company = %company, source = "provider", "salary_research: range");
        Some(range)
    }
}

/// Cap `s` to [`MAX_INPUT_CHARS`] (char-boundary safe — never splits a
/// multi-byte character). Pure + unit-tested.
fn truncate_input(s: &str) -> String {
    s.chars().take(MAX_INPUT_CHARS).collect()
}

/// Whether `role` is missing/whitespace-only — [`SalaryResearch::enrich`] has
/// nothing to search for without it. Pure + unit-tested.
fn role_is_missing(role: &str) -> bool {
    role.trim().is_empty()
}

/// Build the cache key for a (role, company, location) lookup — case-folded so
/// "Berlin"/"berlin" (or "Acme"/"ACME") land on the same cache entry, cutting
/// avoidable cache misses (and duplicate paid provider calls) on the only
/// difference being capitalization. Pure + unit-tested.
fn cache_key(role: &str, company: &str, location: &str) -> String {
    format!(
        "{}|{}|{}",
        role.to_lowercase(),
        company.to_lowercase(),
        location.to_lowercase()
    )
}

/// Parse a (possibly noisy) provider response into a validated [`SalaryRange`].
/// Tolerant of surrounding prose/markdown fences — it locates the first
/// balanced `{...}` object — but the VALUES are strictly validated: this is the
/// injection boundary between untrusted web-search output and the prompt layer,
/// so only sane integers and a plausible currency code ever survive. Returns
/// `None` for `{}`, malformed JSON, or any failed validation. Pure +
/// unit-tested.
fn parse_and_validate(text: &str) -> Option<SalaryRange> {
    let json_str = extract_json_object(text)?;
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let min = value.get("min")?.as_u64()?;
    let max = value.get("max")?.as_u64()?;
    let currency = value.get("currency")?.as_str()?.trim().to_ascii_uppercase();

    if min == 0 || max == 0 || min > max || max > MAX_PLAUSIBLE_SALARY {
        return None;
    }
    if !(3..=4).contains(&currency.len()) || !currency.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }

    Some(SalaryRange {
        min: u32::try_from(min).ok()?,
        max: u32::try_from(max).ok()?,
        currency,
    })
}

/// Find the first balanced top-level `{...}` object in `text`, tolerant of any
/// surrounding prose the model might add despite instructions. Pure +
/// unit-tested.
fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in text[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..start + i + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::error::AppError;

    // ── cache round-trip (the same namespace/TTL constants `enrich` uses) ────

    #[test]
    fn cache_round_trips_a_validated_range_under_the_salary_namespace() {
        let dir = TempDir::new().expect("tempdir");
        let cache = KvCache::open(dir.path()).expect("open cache");
        let range = SalaryRange {
            min: 65000,
            max: 80000,
            currency: "EUR".to_string(),
        };
        let json = serde_json::to_string(&range).expect("serialize");

        cache.set(CACHE_NS, "backend engineer|acme|berlin", &json);

        let cached = cache
            .get(CACHE_NS, "backend engineer|acme|berlin", TTL_SECS)
            .expect("cache hit");
        assert_eq!(parse_and_validate(&cached), Some(range));
    }

    // ── role_is_missing (enrich's early-return guard) ────────────────────────

    #[test]
    fn role_is_missing_flags_empty_and_whitespace_only() {
        assert!(role_is_missing(""));
        assert!(role_is_missing("   "));
        assert!(role_is_missing("\t\n  "));
    }

    #[test]
    fn role_is_missing_accepts_a_real_role() {
        assert!(!role_is_missing("Backend Engineer"));
        assert!(!role_is_missing("  Backend Engineer  "));
    }

    // ── truncate_input ────────────────────────────────────────────────────────

    #[test]
    fn truncate_input_caps_at_max_input_chars() {
        let long = "a".repeat(500);
        let truncated = truncate_input(&long);
        assert_eq!(truncated.chars().count(), MAX_INPUT_CHARS);
    }

    #[test]
    fn truncate_input_is_a_no_op_under_the_cap() {
        assert_eq!(truncate_input("Backend Engineer"), "Backend Engineer");
    }

    #[test]
    fn truncate_input_never_splits_a_multi_byte_character() {
        // A multi-byte (CJK) string longer than the cap must still produce
        // valid UTF-8 — `.chars().take(n)` is boundary-safe by construction,
        // unlike a byte-index slice.
        let long: String = "日".repeat(500);
        let truncated = truncate_input(&long);
        assert_eq!(truncated.chars().count(), MAX_INPUT_CHARS);
        assert!(truncated.chars().all(|c| c == '日'));
    }

    // ── cache_key (case-folding) ─────────────────────────────────────────────

    #[test]
    fn cache_key_case_folds_so_differently_cased_inputs_collide() {
        assert_eq!(
            cache_key("Backend Engineer", "Acme", "Berlin"),
            cache_key("backend engineer", "ACME", "berlin")
        );
    }

    #[test]
    fn cache_key_preserves_the_pipe_delimited_shape() {
        assert_eq!(
            cache_key("Backend Engineer", "Acme", "Berlin"),
            "backend engineer|acme|berlin"
        );
    }

    #[test]
    fn a_no_info_response_never_produces_a_cacheable_value() {
        // `enrich` only calls `cache.set` with the output of a successful
        // `parse_and_validate` — a `{}` ("no reliable data") response yields
        // `None` here, so the caller has nothing to cache (never a stale miss
        // stuck for the 7-day TTL).
        assert_eq!(parse_and_validate("{}"), None);
    }

    // ── extract_json_object ──────────────────────────────────────────────────

    #[test]
    fn extract_json_object_finds_a_bare_object() {
        assert_eq!(
            extract_json_object(r#"{"min":1,"max":2,"currency":"USD"}"#),
            Some(r#"{"min":1,"max":2,"currency":"USD"}"#)
        );
    }

    #[test]
    fn extract_json_object_ignores_surrounding_prose_and_fences() {
        let text = "Sure, here you go:\n```json\n{\"min\":1,\"max\":2,\"currency\":\"USD\"}\n```";
        assert_eq!(
            extract_json_object(text),
            Some(r#"{"min":1,"max":2,"currency":"USD"}"#)
        );
    }

    #[test]
    fn extract_json_object_none_without_braces() {
        assert_eq!(extract_json_object("no data available"), None);
    }

    // ── parse_and_validate ───────────────────────────────────────────────────

    #[test]
    fn valid_json_parses_to_some() {
        let range = parse_and_validate(r#"{"min":65000,"max":80000,"currency":"eur"}"#).unwrap();
        assert_eq!(
            range,
            SalaryRange {
                min: 65000,
                max: 80000,
                currency: "EUR".to_string()
            }
        );
    }

    #[test]
    fn empty_object_no_info_is_none() {
        assert_eq!(parse_and_validate("{}"), None);
    }

    #[test]
    fn malformed_json_is_none() {
        assert_eq!(parse_and_validate("not json at all"), None);
    }

    #[test]
    fn negative_or_zero_values_are_rejected() {
        // serde_json has no negative-as-u64, so a negative min fails `as_u64`.
        assert_eq!(
            parse_and_validate(r#"{"min":-5,"max":80000,"currency":"USD"}"#),
            None
        );
        assert_eq!(
            parse_and_validate(r#"{"min":0,"max":80000,"currency":"USD"}"#),
            None
        );
        assert_eq!(
            parse_and_validate(r#"{"min":50000,"max":0,"currency":"USD"}"#),
            None
        );
    }

    #[test]
    fn min_greater_than_max_is_rejected() {
        assert_eq!(
            parse_and_validate(r#"{"min":90000,"max":80000,"currency":"USD"}"#),
            None
        );
    }

    #[test]
    fn absurdly_large_values_are_rejected() {
        assert_eq!(
            parse_and_validate(r#"{"min":1,"max":999999999999,"currency":"USD"}"#),
            None
        );
    }

    #[test]
    fn bad_currency_shapes_are_rejected() {
        for currency in ["", "U", "US", "TOOLONG", "12A", "eu-r"] {
            let text = format!(r#"{{"min":1,"max":2,"currency":"{currency}"}}"#);
            assert_eq!(parse_and_validate(&text), None, "currency={currency:?}");
        }
    }

    #[test]
    fn four_letter_currency_codes_are_accepted() {
        let range = parse_and_validate(r#"{"min":1,"max":2,"currency":"USDX"}"#).unwrap();
        assert_eq!(range.currency, "USDX");
    }

    #[test]
    fn missing_fields_are_none() {
        assert_eq!(parse_and_validate(r#"{"min":1,"max":2}"#), None);
        assert_eq!(parse_and_validate(r#"{"currency":"USD"}"#), None);
    }

    // ── enrich (fake SalarySearcher — reaches the parse-SUCCESS/cache paths
    // without a live `AppHandle`/network) ────────────────────────────────────

    struct FakeSearcher(&'static str);

    impl SalarySearcher for FakeSearcher {
        async fn research_salary(
            &self,
            _role: &str,
            _company: &str,
            _location: &str,
        ) -> AppResult<String> {
            Ok(self.0.to_string())
        }
    }

    struct ErrSearcher;

    impl SalarySearcher for ErrSearcher {
        async fn research_salary(
            &self,
            _role: &str,
            _company: &str,
            _location: &str,
        ) -> AppResult<String> {
            Err(AppError::Provider("search failed".to_string()))
        }
    }

    struct SlowSearcher;

    impl SalarySearcher for SlowSearcher {
        async fn research_salary(
            &self,
            _role: &str,
            _company: &str,
            _location: &str,
        ) -> AppResult<String> {
            // Sleeps past RESEARCH_TIMEOUT_SECS; under `start_paused = true` this
            // resolves the moment `enrich`'s own timeout timer fires instead of
            // actually blocking the test for 25+ real seconds.
            tokio::time::sleep(Duration::from_secs(RESEARCH_TIMEOUT_SECS + 5)).await;
            Ok(r#"{"min":1,"max":2,"currency":"USD"}"#.to_string())
        }
    }

    #[tokio::test]
    async fn enrich_returns_a_range_on_valid_json_and_writes_through_the_cache() {
        let dir = TempDir::new().expect("tempdir");
        let cache = KvCache::open(dir.path()).expect("open cache");
        let searcher = FakeSearcher(r#"{"min":65000,"max":80000,"currency":"EUR"}"#);

        let result = SalaryResearch
            .enrich(
                &searcher,
                Some(&cache),
                "Backend Engineer",
                "Acme",
                "Berlin",
            )
            .await;

        assert_eq!(
            result,
            Some(SalaryRange {
                min: 65000,
                max: 80000,
                currency: "EUR".to_string()
            })
        );
        let key = cache_key("Backend Engineer", "Acme", "Berlin");
        assert!(
            cache.get(CACHE_NS, &key, TTL_SECS).is_some(),
            "a successful lookup must write through the cache"
        );
    }

    #[tokio::test]
    async fn enrich_returns_none_and_does_not_cache_on_no_reliable_data() {
        let dir = TempDir::new().expect("tempdir");
        let cache = KvCache::open(dir.path()).expect("open cache");
        let searcher = FakeSearcher("{}");

        let result = SalaryResearch
            .enrich(
                &searcher,
                Some(&cache),
                "Backend Engineer",
                "Acme",
                "Berlin",
            )
            .await;

        assert_eq!(result, None);
        let key = cache_key("Backend Engineer", "Acme", "Berlin");
        assert_eq!(cache.get(CACHE_NS, &key, TTL_SECS), None);
    }

    #[tokio::test]
    async fn enrich_returns_none_and_does_not_cache_on_malformed_output() {
        let dir = TempDir::new().expect("tempdir");
        let cache = KvCache::open(dir.path()).expect("open cache");
        let searcher = FakeSearcher("not json at all");

        let result = SalaryResearch
            .enrich(
                &searcher,
                Some(&cache),
                "Backend Engineer",
                "Acme",
                "Berlin",
            )
            .await;

        assert_eq!(result, None);
        let key = cache_key("Backend Engineer", "Acme", "Berlin");
        assert_eq!(cache.get(CACHE_NS, &key, TTL_SECS), None);
    }

    #[tokio::test]
    async fn enrich_returns_none_and_does_not_cache_on_a_searcher_error() {
        let dir = TempDir::new().expect("tempdir");
        let cache = KvCache::open(dir.path()).expect("open cache");

        let result = SalaryResearch
            .enrich(
                &ErrSearcher,
                Some(&cache),
                "Backend Engineer",
                "Acme",
                "Berlin",
            )
            .await;

        assert_eq!(result, None);
        let key = cache_key("Backend Engineer", "Acme", "Berlin");
        assert_eq!(cache.get(CACHE_NS, &key, TTL_SECS), None);
    }

    #[tokio::test(start_paused = true)]
    async fn enrich_returns_none_and_does_not_cache_when_the_searcher_exceeds_the_timeout() {
        let dir = TempDir::new().expect("tempdir");
        let cache = KvCache::open(dir.path()).expect("open cache");

        let result = SalaryResearch
            .enrich(
                &SlowSearcher,
                Some(&cache),
                "Backend Engineer",
                "Acme",
                "Berlin",
            )
            .await;

        assert_eq!(result, None);
        let key = cache_key("Backend Engineer", "Acme", "Berlin");
        assert_eq!(cache.get(CACHE_NS, &key, TTL_SECS), None);
    }
}
