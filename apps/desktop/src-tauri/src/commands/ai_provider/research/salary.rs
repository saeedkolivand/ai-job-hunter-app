//! Salary-range research (C2) — the same native/synthesize split as the
//! company brief in the parent module, but the contract is a compact JSON
//! object instead of prose: `salary_research::SalaryResearch` parses +
//! strictly validates it, so no unvalidated web text ever reaches a prompt
//! (the model's own words never survive past that JSON boundary). Split out
//! of `research.rs` (R8 line-budget split): a distinct facet of the shared
//! research spec, self-contained apart from [`super::SearchResult`] and
//! [`super::role_or_default`].

use super::{role_or_default, SearchResult};

/// System prompt for the salary-range **native** path: the model searches the
/// web itself and must reply with JSON only — no prose to parse out. Pins the
/// report currency when the caller knows one (resolved client-side from the
/// job's validated ISO country via `countryToCurrency`) — the primary defense
/// against the model defaulting to USD/hallucinating a currency on a
/// blank/weak location. Falls back to the original unconstrained "local
/// currency for that location" wording when `currency` is empty (unknown
/// country — today's behavior, unchanged).
pub fn salary_system(currency: &str) -> String {
    let currency_phrase = currency_phrase(currency);
    format!(
        "You are a compensation research assistant with web search. \
         Search the web for the typical ANNUAL gross salary range for the specified role — at \
         the specified company when reliable company-specific data exists, otherwise for the \
         broader market in the specified location. Respond with ONLY a compact JSON object in \
         the exact form {{\"min\":<integer>,\"max\":<integer>,\"currency\":\"<ISO-4217 code>\"}}, \
         using {currency_phrase}. If you cannot find reliable data, respond with {{}}. No \
         prose, no markdown, no code fences, no commentary — JSON only."
    )
}

/// User prompt for the salary-range **native** path (the provider's model
/// searches + writes the JSON itself). Appends an authoritative currency-pin
/// clause when `currency` is known — see [`salary_system`].
pub fn salary_user(
    role: &str,
    company: &str,
    location: &str,
    country: &str,
    currency: &str,
) -> String {
    let role = role_or_default(role);
    let mut where_clause = String::new();
    if !company.trim().is_empty() {
        where_clause.push_str(&format!(" at \"{}\"", company.trim()));
    }
    if !location.trim().is_empty() {
        where_clause.push_str(&format!(" in {}", location.trim()));
    }
    let currency_clause = currency_pin_clause(country, currency);
    format!(
        "Search the web for the typical annual gross salary range for a {role}{where_clause}.\
         {currency_clause} Respond with ONLY the JSON object described in your instructions — no prose."
    )
}

/// The web-search query for the salary explicit-query path (Ollama). Includes
/// `country` alongside `location` — a geo-targeting hint, since `location` can
/// be vague ("Remote") while the job's country is still resolved — but only
/// once `currency` is resolved too, mirroring [`currency_pin_clause`]'s
/// gating on the native path: an unresolved currency means the country
/// couldn't be trusted enough to pin a currency, so it shouldn't leak into
/// the query ungated either.
pub fn salary_search_query(
    role: &str,
    company: &str,
    location: &str,
    country: &str,
    currency: &str,
) -> String {
    let role = role_or_default(role);
    let mut q = format!("{role} salary range annual");
    if !company.trim().is_empty() {
        q.push_str(&format!(" {}", company.trim()));
    }
    if !location.trim().is_empty() {
        q.push_str(&format!(" {}", location.trim()));
    }
    if !currency.trim().is_empty() && !country.trim().is_empty() {
        q.push_str(&format!(" {}", country.trim()));
    }
    q
}

/// User prompt for the salary-range **synthesize** path (Ollama): turn search
/// snippets into the same compact JSON contract as [`salary_user`]. Pins the
/// currency the same way [`salary_system`] does.
pub fn salary_synth_user(
    role: &str,
    company: &str,
    location: &str,
    country: &str,
    currency: &str,
    results: &[SearchResult],
) -> String {
    let role = role_or_default(role);
    let company = if company.trim().is_empty() {
        "unspecified"
    } else {
        company.trim()
    };
    let location = if location.trim().is_empty() {
        "unspecified"
    } else {
        location.trim()
    };
    // Gated on a *resolved* currency (mirrors [`currency_pin_clause`]'s native-
    // path condition), not merely a non-empty `country` — a country the
    // caller couldn't resolve a currency for shouldn't be interpolated
    // ungated into the prompt either.
    let country_line = if currency.trim().is_empty() || country.trim().is_empty() {
        String::new()
    } else {
        format!("\nCountry: {}", country.trim())
    };
    let currency_phrase = currency_phrase(currency);
    let snippets = results
        .iter()
        .enumerate()
        .map(|(i, r)| format!("[{}] {} — {}", i + 1, r.title, r.snippet))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Role: {role}\nCompany: {company}\nLocation: {location}{country_line}\n\n\
         Search result snippets:\n{snippets}\n\n\
         From these snippets, estimate the typical ANNUAL gross salary range in {currency_phrase}. \
         Respond with ONLY a compact JSON object in the exact form \
         {{\"min\":<integer>,\"max\":<integer>,\"currency\":\"<ISO-4217 code>\"}}. If the \
         snippets don't support a reliable estimate, respond with {{}}. No prose."
    )
}

/// Shared currency wording for [`salary_system`] and [`salary_synth_user`]:
/// "the local currency for that location" (today's unconstrained default) or
/// an authoritative pin naming the confirmed currency. Pure + unit-tested.
fn currency_phrase(currency: &str) -> String {
    let currency = currency.trim();
    if currency.is_empty() {
        "the local currency for that location".to_string()
    } else {
        format!(
            "{currency} — the confirmed currency for this role's location; do not report any \
             other currency"
        )
    }
}

/// Authoritative currency-pinning sentence appended to [`salary_user`] — empty
/// (no-op) when `currency` is unknown, so a job with no resolvable country
/// gets today's unconstrained prompt. Pure + unit-tested.
fn currency_pin_clause(country: &str, currency: &str) -> String {
    let currency = currency.trim();
    if currency.is_empty() {
        return String::new();
    }
    let country = country.trim();
    if country.is_empty() {
        format!(" Report the salary range in {currency} — do not use any other currency.")
    } else {
        format!(
            " The role is based in {country}; report the salary range in {currency} — do not \
             use any other currency."
        )
    }
}
