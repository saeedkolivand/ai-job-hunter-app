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
/// Sanity ceiling on an annual salary figure, in any currency's minor-unit-free
/// face value — comfortably above any real annual salary, so a wildly
/// hallucinated figure is rejected rather than reaching the prompt.
const MAX_PLAUSIBLE_SALARY: u64 = 100_000_000;
/// Cap on each of `role`/`company`/`location` (chars, so always a valid UTF-8
/// boundary) before it reaches the cache key or a provider query — a caller
/// passing something absurdly long can't inflate the key or the outbound
/// search query. `pub(crate)` — reused by `commands::ai::ai_research_answer`
/// for the same shape of forwarded strings (question/role/company).
pub(crate) const MAX_INPUT_CHARS: usize = 200;

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
        country: &str,
        currency: &str,
    ) -> impl std::future::Future<Output = AppResult<String>> + Send;
}

impl SalarySearcher for Completer {
    async fn research_salary(
        &self,
        role: &str,
        company: &str,
        location: &str,
        country: &str,
        currency: &str,
    ) -> AppResult<String> {
        Completer::research_salary(self, role, company, location, country, currency).await
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
    /// `country`/`currency` ground the report in the job's actual currency
    /// (resolved client-side from its validated ISO country) — both empty when
    /// unknown, which preserves the unconstrained "local currency for that
    /// location" behavior. When known, `currency` is also the safety net
    /// against a stray hallucinated currency slipping past the model's own
    /// JSON: [`reconcile_expected_currency`] fails safe — a parsed currency
    /// that doesn't match the expected one is untrustworthy and is **dropped**
    /// (never relabeled, which would put the wrong numbers under the right
    /// symbol) — applied on both the cache-hit and fresh-fetch paths below.
    /// The model is still asked to *research* in that currency (see
    /// `commands::ai_provider::research::salary_system`), this is only the
    /// defense-in-depth backstop.
    ///
    /// `cache` is injected (`None` when the caller has no `KvCache` managed
    /// state) rather than looked up here — the sole production caller
    /// (`commands::ai::ai_lookup_salary`) resolves it once via
    /// `app.try_state::<KvCache>()` and passes it through, which is what keeps
    /// this function testable without an `AppHandle`.
    ///
    /// `deadline` is injected for the same reason `cache` is — see
    /// [`CompanyResearch::enrich_with`](crate::cover_letter::research::CompanyResearch::enrich_with).
    /// The L3 caller derives it from the request's reasoning effort via
    /// `timeouts::research_deadline`; a FLAT bound was the bug, since synthesis
    /// is a model call and costs whatever the chosen model's reasoning costs.
    #[allow(clippy::too_many_arguments)]
    pub async fn enrich<S: SalarySearcher>(
        &self,
        searcher: &S,
        cache: Option<&KvCache>,
        role: &str,
        company: &str,
        location: &str,
        country: &str,
        currency: &str,
        deadline: Duration,
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
        let country = truncate_input(country.trim());
        let currency = truncate_input(currency.trim());

        // Case-folded so "Berlin"/"berlin" don't miss each other; the
        // case-preserved values above still go to the prompt/query/logging.
        // Keyed on `currency` (not the raw `country` string) — two jobs
        // sharing role/company/location but resolving to different expected
        // currencies (e.g. a DE and a US "Remote" posting) must never share a
        // cache row, or an unknown-currency read could surface whatever
        // currency a different, known-currency job last wrote there. The
        // `reconcile_expected_currency` self-heal below still re-checks every
        // read regardless, so a legacy (pre-fix) or otherwise wrong-currency
        // cached entry self-heals via a fresh fetch rather than fragmenting
        // the cache further.
        let key = cache_key(&role, &company, &location, &currency);

        // Fast path: cached, validated range younger than the TTL.
        if let Some(cache) = cache {
            if let Some(json) = cache.get(CACHE_NS, &key, TTL_SECS) {
                if let Some(range) = parse_and_validate(&json) {
                    match reconcile_expected_currency(range, &currency) {
                        Some(range) => {
                            tracing::info!(role = %role, company = %company, source = "cache", "salary_research: range");
                            return Some(range);
                        }
                        None => {
                            // A pre-fix (or otherwise stale) cache entry in the
                            // wrong currency — untrustworthy, don't relabel it.
                            // Fall through to a fresh fetch instead of a cache
                            // miss's usual `None`; a successful fetch below
                            // overwrites this row (`cache.set` is upsert), so
                            // the entry self-heals without waiting out the TTL.
                            tracing::debug!(role = %role, company = %company, "salary_research: cached range is in the wrong currency, dropping and re-fetching");
                        }
                    }
                }
            }
        }

        // Provider-native research, bounded so generation never stalls. Any
        // failure/timeout yields no range.
        let raw = match tokio::time::timeout(
            deadline,
            searcher.research_salary(&role, &company, &location, &country, &currency),
        )
        .await
        {
            Ok(Ok(text)) => text,
            Ok(Err(e)) => {
                tracing::warn!("salary_research: provider research failed for {role}: {e}");
                return None;
            }
            Err(_) => {
                tracing::warn!(
                    role = %role,
                    deadline_secs = deadline.as_secs(),
                    "salary_research: timed out"
                );
                return None;
            }
        };

        // `{}` ("no reliable data"), malformed JSON, and any failed validation
        // all fall through here — never cached, so a bad miss doesn't stick for
        // the 7-day TTL.
        let range = parse_and_validate(&raw)?;
        // Fail-safe, not relabel: a fresh result in the wrong currency is
        // dropped (`None`) rather than shown under the wrong symbol.
        let range = reconcile_expected_currency(range, &currency)?;

        if let Some(cache) = cache {
            if let Ok(json) = serde_json::to_string(&range) {
                cache.set(CACHE_NS, &key, &json);
            }
        }

        tracing::info!(role = %role, company = %company, source = "provider", "salary_research: range");
        Some(range)
    }

    /// The cached, still-fresh, currency-reconciled range [`Self::enrich`]'s OWN fast path would
    /// return — checked WITHOUT ever touching `searcher`, so a caller that must not spend a
    /// billable/rate-limited provider round trip (or whatever ELSE gates that round trip, like a
    /// daily quota charge) can check for a hit first. Built from the exact same pure building
    /// blocks `enrich`'s fast path uses ([`truncate_input`], [`cache_key`], [`parse_and_validate`],
    /// [`reconcile_expected_currency`]) rather than a hand-copied fast path, so a hit here is
    /// always the SAME hit `enrich` would find — nothing to keep in sync by hand.
    ///
    /// `None` on a missing role, a cache miss, an expired/malformed entry, or an entry in the
    /// wrong currency (which `enrich` would re-fetch rather than trust — see its own doc for why).
    pub fn cached_range(
        &self,
        cache: Option<&KvCache>,
        role: &str,
        company: &str,
        location: &str,
        currency: &str,
    ) -> Option<SalaryRange> {
        if role_is_missing(role) {
            return None;
        }
        let role = truncate_input(role.trim());
        let company = truncate_input(company.trim());
        let location = truncate_input(location.trim());
        let currency = truncate_input(currency.trim());
        let key = cache_key(&role, &company, &location, &currency);
        let json = cache?.get(CACHE_NS, &key, TTL_SECS)?;
        let range = parse_and_validate(&json)?;
        reconcile_expected_currency(range, &currency)
    }
}

/// Fail-safe backstop behind [`commands::ai_provider::research::salary_system`]'s
/// prompt-level pin: even if the model's own JSON slips a stray/wrong currency
/// code past [`parse_and_validate`]'s shape check, this catches it — but it
/// **drops** the range rather than relabeling it, since relabeling would put
/// the wrong-currency numbers under the right symbol (more misleading than an
/// obviously-wrong one). Returns `Some(range)` unchanged when
/// `expected_currency` is empty (unknown country — preserves today's
/// unconstrained behavior) or already matches; returns `None` when a known
/// expected currency doesn't match the parsed one. Pure + unit-tested.
fn reconcile_expected_currency(range: SalaryRange, expected_currency: &str) -> Option<SalaryRange> {
    let expected = expected_currency.trim();
    if expected.is_empty() || expected.eq_ignore_ascii_case(&range.currency) {
        return Some(range);
    }
    None
}

/// Cap `s` to [`MAX_INPUT_CHARS`] (char-boundary safe — never splits a
/// multi-byte character). Pure + unit-tested. `pub(crate)` — see
/// [`MAX_INPUT_CHARS`].
pub(crate) fn truncate_input(s: &str) -> String {
    s.chars().take(MAX_INPUT_CHARS).collect()
}

/// Whether `role` is missing/whitespace-only — [`SalaryResearch::enrich`] has
/// nothing to search for without it. Pure + unit-tested.
fn role_is_missing(role: &str) -> bool {
    role.trim().is_empty()
}

/// Build the cache key for a (role, company, location, expected currency)
/// lookup — case-folded so "Berlin"/"berlin" (or "Acme"/"ACME") land on the
/// same cache entry, cutting avoidable cache misses (and duplicate paid
/// provider calls) on the only difference being capitalization. `currency` is
/// part of the key (not just baked into `location`/`country`) so two postings
/// that share role/company/location but resolve to different expected
/// currencies — including an unknown-currency ("") job vs. a known-currency
/// one — never collide on the same cache row. Pure + unit-tested.
fn cache_key(role: &str, company: &str, location: &str, currency: &str) -> String {
    format!(
        "{}|{}|{}|{}",
        role.to_lowercase(),
        company.to_lowercase(),
        location.to_lowercase(),
        currency.to_lowercase()
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
mod tests;
