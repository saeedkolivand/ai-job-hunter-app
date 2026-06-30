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
}
