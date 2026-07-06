//! Company-research brief: the shared prompt spec + helpers used by every
//! provider's [`AiProvider::research`](super::AiProvider::research) impl.
//!
//! One brief *spec*, two prompt *shapes*:
//! * **native** — providers with their own web search (OpenAI/Anthropic/Gemini,
//!   CLI agents) get a single instruction; the model searches and writes.
//! * **synthesize** — Ollama has no model-side search, so we fetch snippets via
//!   the Ollama Web Search API and ask the model to synthesize them.
//!
//! Both shapes cover the same facets so the *one* brief serves cover letters
//! **and** application-question answers. The brief is reference-only context —
//! it is fenced as untrusted downstream and never a source of candidate facts.

/// A single web-search result snippet (the Ollama web-search shape).
pub struct SearchResult {
    pub title: String,
    pub snippet: String,
    #[allow(dead_code)]
    pub url: String,
}

/// Facets every brief covers. `{role}` is substituted by the callers' prompt
/// text. Broadened beyond "what they do / size / products" to also serve
/// application questions (mission, values, culture, recent news).
const FACETS: &str = "what the company does; approximate size or stage; \
notable products or customers; mission and values; culture and what they are known for; \
and any recent news or milestones relevant to the candidate";

/// System prompt for the **synthesize** path (Ollama): turn snippets into a brief.
pub const SYNTH_SYSTEM: &str = "You are a company research assistant. \
Given search result snippets about a company, produce a factual, concise brief. \
Return ONLY the brief — no headers, no caveats, no markdown, no citations.";

/// System prompt for the **native** path: the model searches the web itself.
pub const NATIVE_SYSTEM: &str = "You are a company research assistant with web search. \
Search the web for current, factual information about the company, then produce a \
concise brief. Return ONLY the brief — no headers, no caveats, no markdown, no citations.";

/// The web-search query for the explicit-query path (Ollama). Kept broad (no
/// `site:` filter) so mission/culture/recent-news surface alongside the overview.
pub fn search_query(company: &str) -> String {
    format!("{company} company overview mission culture products recent news")
}

/// User prompt for the **native** path (the provider's model searches + writes).
pub fn native_user(company: &str, role: &str) -> String {
    let role = role_or_default(role);
    format!(
        "Research the company \"{company}\" (currently hiring for a {role}). \
         Search the web for current information and write a 120-150 word factual brief covering: \
         {facets}. Be precise — only state facts you can verify from search results.",
        facets = FACETS.replace("the candidate", &format!("a {role} candidate"))
    )
}

/// User prompt for the **synthesize** path (Ollama): write a brief from snippets.
pub fn synth_user(company: &str, role: &str, results: &[SearchResult]) -> String {
    let role = role_or_default(role);
    let snippets = results
        .iter()
        .enumerate()
        .map(|(i, r)| format!("[{}] {} — {}", i + 1, r.title, r.snippet))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Company: {company}\n\
         Role being filled: {role}\n\n\
         Search result snippets:\n{snippets}\n\n\
         Write a 120-150 word factual company brief covering: {facets}. \
         Be precise — do not invent facts not present in the snippets.",
        facets = FACETS.replace("the candidate", &format!("a {role} candidate"))
    )
}

fn role_or_default(role: &str) -> &str {
    let r = role.trim();
    if r.is_empty() {
        "candidate"
    } else {
        r
    }
}

// ── Salary-range research (C2) ───────────────────────────────────────────────────
//
// Same native/synthesize split as the company brief above, but the contract is a
// compact JSON object instead of prose: `salary_research::SalaryResearch` parses
// + strictly validates it, so no unvalidated web text ever reaches a prompt (the
// model's own words never survive past that JSON boundary).

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
/// `country` (when known) alongside `location` — a geo-targeting hint, since
/// `location` can be vague ("Remote") while the job's country is still
/// resolved.
pub fn salary_search_query(role: &str, company: &str, location: &str, country: &str) -> String {
    let role = role_or_default(role);
    let mut q = format!("{role} salary range annual");
    if !company.trim().is_empty() {
        q.push_str(&format!(" {}", company.trim()));
    }
    if !location.trim().is_empty() {
        q.push_str(&format!(" {}", location.trim()));
    }
    if !country.trim().is_empty() {
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
    let country_line = if country.trim().is_empty() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_includes_company_and_facets() {
        let q = search_query("Acme Corp");
        assert!(q.contains("Acme Corp"));
        assert!(q.to_lowercase().contains("mission"));
        assert!(q.to_lowercase().contains("news"));
    }

    #[test]
    fn native_user_names_company_and_role() {
        let p = native_user("Acme", "Backend Engineer");
        assert!(p.contains("Acme"));
        assert!(p.contains("Backend Engineer"));
        assert!(p.to_lowercase().contains("web"));
    }

    #[test]
    fn synth_user_lists_snippets_and_falls_back_on_empty_role() {
        let results = vec![
            SearchResult {
                title: "Acme — Wikipedia".into(),
                snippet: "Acme makes widgets.".into(),
                url: "https://example.com".into(),
            },
            SearchResult {
                title: "Acme careers".into(),
                snippet: "Series B, 200 employees.".into(),
                url: "https://example.com/careers".into(),
            },
        ];
        let p = synth_user("Acme", "  ", &results);
        assert!(p.contains("[1] Acme — Wikipedia — Acme makes widgets."));
        assert!(p.contains("[2] Acme careers — Series B, 200 employees."));
        assert!(p.contains("Role being filled: candidate"));
    }

    #[test]
    fn salary_user_names_role_company_and_location_and_demands_json() {
        let p = salary_user("Backend Engineer", "Acme", "Berlin, Germany", "", "");
        assert!(p.contains("Backend Engineer"));
        assert!(p.contains("Acme"));
        assert!(p.contains("Berlin, Germany"));
        assert!(p.to_lowercase().contains("json"));
    }

    #[test]
    fn salary_user_omits_company_and_location_clauses_when_blank() {
        let p = salary_user("Backend Engineer", "  ", "  ", "", "");
        assert!(!p.contains(" at \""));
        // No where-clause inserted: the role is followed directly by the
        // instruction sentence, not by an " in <location>" clause.
        assert!(p.starts_with(
            "Search the web for the typical annual gross salary range for a Backend Engineer. "
        ));
    }

    #[test]
    fn salary_user_pins_the_currency_when_country_and_currency_are_known() {
        let p = salary_user("Backend Engineer", "Acme", "", "DE", "EUR");
        assert!(p.contains("The role is based in DE"));
        assert!(p.contains("report the salary range in EUR"));
        assert!(p.contains("do not use any other currency"));
    }

    #[test]
    fn salary_user_currency_clause_is_empty_when_currency_is_unknown() {
        // Unknown-country guard: no clause at all, not even a bare country
        // mention — today's unconstrained behavior.
        let p = salary_user("Backend Engineer", "Acme", "", "", "");
        assert!(!p.contains("report the salary range in"));
        assert!(!p.contains("based in"));
    }

    #[test]
    fn salary_search_query_includes_role_company_and_location() {
        let q = salary_search_query("Backend Engineer", "Acme", "Berlin", "");
        assert!(q.contains("Backend Engineer"));
        assert!(q.contains("Acme"));
        assert!(q.contains("Berlin"));
        assert!(q.to_lowercase().contains("salary"));
    }

    #[test]
    fn salary_search_query_includes_country_when_known() {
        let q = salary_search_query("Backend Engineer", "", "Remote", "DE");
        assert!(q.contains("DE"));
    }

    #[test]
    fn salary_synth_user_lists_snippets_and_requests_json_with_fallback_labels() {
        let results = vec![SearchResult {
            title: "Levels.fyi".into(),
            snippet: "Backend Engineer $120k-$150k".into(),
            url: "https://example.com".into(),
        }];
        let p = salary_synth_user("Backend Engineer", "  ", "  ", "", "", &results);
        assert!(p.contains("[1] Levels.fyi — Backend Engineer $120k-$150k"));
        assert!(p.contains("Company: unspecified"));
        assert!(p.contains("Location: unspecified"));
        assert!(p.to_lowercase().contains("json"));
        // Unknown-country guard preserves the original unconstrained wording.
        assert!(p.contains("in the local currency for that location"));
    }

    #[test]
    fn salary_synth_user_pins_the_currency_and_notes_the_country_when_known() {
        let results = vec![SearchResult {
            title: "Levels.fyi".into(),
            snippet: "Backend Engineer €65k-€80k".into(),
            url: "https://example.com".into(),
        }];
        let p = salary_synth_user("Backend Engineer", "Acme", "Berlin", "DE", "EUR", &results);
        assert!(p.contains("Country: DE"));
        assert!(p.contains("in EUR"));
        assert!(p.contains("do not report any other currency"));
        assert!(!p.contains("in the local currency for that location"));
    }

    #[test]
    fn salary_system_states_the_json_contract_using_the_local_currency_by_default() {
        let s = salary_system("");
        assert!(s.to_lowercase().contains("json"));
        assert!(s.contains("using the local currency for that location"));
    }

    #[test]
    fn salary_system_pins_the_currency_when_known() {
        let s = salary_system("EUR");
        assert!(s.contains("EUR"));
        assert!(s.contains("do not report any other currency"));
        assert!(!s.contains("using the local currency for that location"));
    }
}
