//! Application-answer web-search notes — the per-question sibling of the
//! company brief in the parent module: same native/synthesize split, but
//! scoped to a single application question (combines it with the role +
//! company) rather than a general company overview. The contract is
//! FACTUAL NOTES ONLY — the model must never write the answer itself, so the
//! résumé-grounded answer prompt (which fences this output as untrusted)
//! stays the sole author of the actual answer. Split out of `research.rs`
//! (R8 line-budget split).

use super::role_or_default;

/// System prompt for the **native** path: the model searches the web itself
/// for facts relevant to answering the question, but must not answer it.
pub const ANSWER_SYSTEM: &str = "You are a research assistant with web search, supporting a job \
applicant who is answering an application question. Search the web for current, factual, \
publicly available information relevant to the question's topic (e.g. the company, role, or \
industry) that could help ground a strong answer. Return ONLY concise factual notes — no \
headers, no markdown, no citations. Do NOT write an answer to the question yourself, do NOT \
invent personal experience, and do NOT address the reader directly.";

/// System prompt for the **synthesize** path (Ollama): turn snippets into
/// notes, with the same "never write the answer" guardrail as [`ANSWER_SYSTEM`].
pub const ANSWER_SYNTH_SYSTEM: &str = "You are a research assistant. Given search result \
snippets relevant to a job applicant's application question, produce concise factual notes. \
Return ONLY the notes — no headers, no markdown, no citations. Do NOT write an answer to the \
question yourself, do NOT invent personal experience, and do NOT address the reader directly.";

/// User prompt for the **native** path (the provider's model searches + writes notes).
pub fn answer_user(question: &str, role: &str, company: &str) -> String {
    let role = role_or_default(role);
    let mut where_clause = String::new();
    if !company.trim().is_empty() {
        where_clause.push_str(&format!(" at \"{}\"", company.trim()));
    }
    format!(
        "An applicant for a {role}{where_clause} is answering this application question: \
         \"{question}\". Search the web for current, factual information relevant to this \
         question — about the company, role, or industry as applicable — that could help ground \
         a strong answer. Return only the factual findings, never the answer itself."
    )
}

/// The web-search query for the explicit-query path (Ollama) — combines the
/// question with the role + company for relevance.
pub fn answer_search_query(question: &str, role: &str, company: &str) -> String {
    let role = role_or_default(role);
    let mut q = format!("{question} {role}");
    if !company.trim().is_empty() {
        q.push_str(&format!(" {}", company.trim()));
    }
    q
}

/// User prompt for the **synthesize** path (Ollama): turn snippets into notes.
pub fn answer_synth_user(
    question: &str,
    role: &str,
    company: &str,
    results: &[super::SearchResult],
) -> String {
    let role = role_or_default(role);
    let company = if company.trim().is_empty() {
        "unspecified"
    } else {
        company.trim()
    };
    let snippets = results
        .iter()
        .enumerate()
        .map(|(i, r)| format!("[{}] {} — {}", i + 1, r.title, r.snippet))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Application question: \"{question}\"\nRole: {role}\nCompany: {company}\n\n\
         Search result snippets:\n{snippets}\n\n\
         From these snippets, write concise factual notes relevant to answering the question. \
         Do not write the answer itself, do not invent facts not present in the snippets, and do \
         not address the reader directly."
    )
}
