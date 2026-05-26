use crate::cover_letter::llm::LlmProvider;
use crate::cover_letter::research::search::SearchResult;

const SYSTEM: &str = "\
You are a company research assistant. \
Given search result snippets about a company, produce a factual, concise brief. \
Return ONLY the brief — no headers, no caveats, no markdown.";

/// Synthesise search snippets into a ~150-word company brief using the fast LLM.
pub async fn synthesise(
    llm: &dyn LlmProvider,
    company: &str,
    role: &str,
    results: &[SearchResult],
) -> Result<String, String> {
    if results.is_empty() {
        return Ok(String::new());
    }

    let snippets = results
        .iter()
        .enumerate()
        .map(|(i, r)| format!("[{}] {} — {}", i + 1, r.title, r.snippet))
        .collect::<Vec<_>>()
        .join("\n");

    let user = format!(
        "Company: {company}\n\
         Role being filled: {role}\n\n\
         Search result snippets:\n{snippets}\n\n\
         Write a 120-150 word factual company brief covering: \
         what the company does, approximate size or stage, \
         notable products or customers, and any recent news relevant to a {role} candidate. \
         Be precise — do not invent facts not present in the snippets."
    );

    llm.complete(SYSTEM, &user).await
}
