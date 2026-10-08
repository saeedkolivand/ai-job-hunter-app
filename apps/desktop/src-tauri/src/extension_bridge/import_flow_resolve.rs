//! Pure posting-resolution helpers for `import.request` — split from `import_flow.rs` (R8
//! relief): persistence + usability/merge decisions with no `AppHandle`, directly unit-testable
//! without a Tauri app. See `import_flow`'s own module doc for the whole import flow.

use crate::applications::{ApplicationMeta, ApplicationOrigin, ApplicationStore};
use crate::error::AppResult;

/// Persist a parsed [`crate::scraping::types::JobPosting`] from an import as a
/// Saved Application and return `(application_id, status_id)`. This is the
/// *entire* persistence side effect of an import: it touches the
/// [`ApplicationStore`] only and has **no access to the `PostingsCache`**, so
/// an import can never enter the Jobs/discovery feed. Split out of
/// [`handle_import`] (which needs an `AppHandle` for event/notification
/// plumbing) so the import → Application contract is unit-testable without a
/// Tauri app — see `import_flow_resolve/tests/persist.rs`.
pub(super) fn persist_import_application(
    store: &ApplicationStore,
    normalized_url: &str,
    posting: &crate::scraping::types::JobPosting,
    applied: Option<bool>,
) -> AppResult<(String, String)> {
    let meta = ApplicationMeta {
        company: posting.company.clone(),
        title: posting.title.clone(),
        job_description: posting.description.clone().unwrap_or_default(),
        ..Default::default()
    };
    let id = store.upsert_for_origin(
        normalized_url,
        &posting.source,
        &meta,
        ApplicationOrigin::Saved,
        applied,
    )?;
    let status = store
        .get(&id)
        .map(|a| a.status.as_id().to_string())
        .unwrap_or_else(|| "saved".to_string());
    Ok((id, status))
}

/// A posting is usable for an import only if it carries a real title; an
/// empty-title parse means the extractor degraded (blocked fetch / unknown page).
pub(super) fn usable(p: &crate::scraping::types::JobPosting) -> bool {
    !p.title.trim().is_empty()
}

/// Refusal shown by the extension when a generic-fallback page carries no job signal.
pub(super) const NOT_A_JOB_MSG: &str =
    "This page doesn't look like a job posting. Open the job's own page and try again.";

/// Minimum description length (chars) for a no-JSON-LD careers page to count as a job.
const MIN_JOB_DESCRIPTION_CHARS: usize = 300;

/// Longest description (chars) the prose-only fallback accepts: a posting is a few thousand
/// chars, an encyclopedia article or company page is far longer (#1408).
const MAX_JOB_DESCRIPTION_CHARS: usize = 50_000;

/// The prose-only fallback also needs a stem density of at least one hit per this many tokens
/// (4 per 1000): real postings measure 7-15 per 1000, long articles 1-3 (#1408).
const MIN_STEM_TOKENS_PER_HIT: usize = 250;

/// The prose-only fallback also needs a requirements-section signal: a company/about page says
/// "join"/"careers"/"responsibility" in its values copy, but only a posting lists what it asks of
/// the candidate (#1408). Token prefixes (en/de/fr/es/it/nl/pl) ...
const REQUIREMENT_PREFIXES: &[&str] = &[
    "requirement",
    "qualification",
    "qualifica",
    "kwalificatie",
    "kwalifikac",
    "anforderung",
    "requisit",
    "exigence",
    "competence",
    "compétence",
    "competenz",
    "kompetenz",
    "vereist",
    "wymagan",
    "oczekiwan",
    "aufgaben",
];

/// ... exact tokens and whole-token phrases (the text is joined with single spaces).
const REQUIREMENT_TOKENS: &[&str] = &["skills", "eisen", "profiel", "profil"];
const REQUIREMENT_PHRASES: &[&str] = &[
    "must have",
    "nice to have",
    "you have",
    "you ll bring",
    "you will bring",
    "you bring",
    "experience with",
];

/// Whether `text` (already lowercase) contains a requirements-section signal.
fn has_requirement(text: &str) -> bool {
    let toks: Vec<&str> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    let joined = format!(" {} ", toks.join(" "));
    toks.iter().any(|t| {
        REQUIREMENT_TOKENS.contains(t) || REQUIREMENT_PREFIXES.iter().any(|p| t.starts_with(p))
    }) || REQUIREMENT_PHRASES
        .iter()
        .any(|ph| joined.contains(&format!(" {ph} ")))
}

/// Job-ish words (en/de/fr/es/it/nl/pt/pl) that must match a WHOLE token (plus a
/// plural `s`), because as raw substrings they hit "joint", "composition",
/// "appliance", "poster".
const JOB_WORDS: &[&str] = &[
    "job", "join", "opening", "position", "apply", "hiring", "stelle", "stellen", "emploi",
    "offre", "poste", "empleo", "trabajo", "oferta", "vacante", "lavoro", "vaga", "praca", "pracy",
];

/// Job-ish prefixes matched at the START of a token.
const JOB_PREFIXES: &[&str] = &[
    "career",
    "vacanc",
    "responsibilit",
    "qualification",
    "requirement",
    "stellenangebot",
    "karriere",
    "bewerb",
    "aufgaben",
    "anforderungen",
    "recrut",
    "candidat",
    "carri",
    "requisitos",
    "posizione",
    "vacature",
    "solliciteer",
    "rekrut",
    "kariera",
];

/// Job-stem hits among the alphanumeric tokens of `text` (already lowercase):
/// `(distinct stems, total hits, total tokens)`.
fn job_stem_stats(text: &str) -> (std::collections::HashSet<&'static str>, usize, usize) {
    let mut found = std::collections::HashSet::new();
    let (mut hits, mut tokens) = (0, 0);
    for tok in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
    {
        tokens += 1;
        let singular = tok.strip_suffix('s').unwrap_or(tok);
        let stem = JOB_WORDS
            .iter()
            .find(|w| tok == **w || singular == **w)
            .or_else(|| JOB_PREFIXES.iter().find(|pre| tok.starts_with(**pre)));
        if let Some(s) = stem {
            found.insert(*s);
            hits += 1;
        }
    }
    (found, hits, tokens)
}

/// Whether a usable posting is plausibly a job. Biased toward accepting. Passes when:
/// the posting came from a named board (`source != "url"`), a recognised ATS URL,
/// the page embeds an ATS board (`embedded`, the #1238 wrapper), the HTML carries a
/// `JobPosting` schema or the generic parser marked the posting `company_src: jsonld`
/// (covers URL mode, where there is no captured HTML), a job stem is in the title or
/// URL path, or the description is within [`MIN_JOB_DESCRIPTION_CHARS`]..=[`MAX_JOB_DESCRIPTION_CHARS`],
/// holds at least two distinct job stems plus a requirements signal ([`has_requirement`]), and is dense in them ([`MIN_STEM_TOKENS_PER_HIT`]). A company merely differing from the host is NOT a
/// signal: og:site_name / logo alt make that true of nearly every site.
pub(super) fn looks_like_job(
    p: &crate::scraping::types::JobPosting,
    html: Option<&str>,
    embedded: bool,
) -> bool {
    if embedded
        || p.source != "url"
        || crate::scraping::ats_ref::extract_ats_ref(&p.url).is_some()
        || html.is_some_and(|h| h.contains("JobPosting"))
        || p.extra.get("company_src").and_then(|v| v.as_str()) == Some("jsonld")
    {
        return true;
    }
    let path = reqwest::Url::parse(&p.url)
        .map(|u| u.path().to_lowercase())
        .unwrap_or_default();
    if !job_stem_stats(&format!("{} {path}", p.title.to_lowercase()))
        .0
        .is_empty()
    {
        return true;
    }
    let desc = p.description.as_deref().unwrap_or("").to_lowercase();
    let chars = desc.chars().count();
    let (stems, hits, tokens) = job_stem_stats(&desc);
    (MIN_JOB_DESCRIPTION_CHARS..=MAX_JOB_DESCRIPTION_CHARS).contains(&chars)
        && stems.len() >= 2
        && has_requirement(&desc)
        && hits * MIN_STEM_TOKENS_PER_HIT >= tokens
}

/// The gate `handle_import` applies to a usable posting: `Err(Validation)` with
/// [`NOT_A_JOB_MSG`] when [`looks_like_job`] is false.
pub(super) fn require_job_signal(
    p: &crate::scraping::types::JobPosting,
    html: Option<&str>,
    embedded: bool,
) -> AppResult<()> {
    if looks_like_job(p, html, embedded) {
        Ok(())
    } else {
        Err(crate::error::AppError::Validation(
            NOT_A_JOB_MSG.to_string(),
        ))
    }
}

/// Collapse every whitespace run (incl. embedded newlines) in a title to one space.
pub(super) fn clean_title(title: &str) -> String {
    title.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Fill `resolve`'s title/description from the extension's `[data-ajh-job-root]`
/// HINT ONLY — used by the SPA/list-view (canonical) import branch when the
/// resolve came back unusable or description-less (LinkedIn's anonymous-fetch
/// authwall is the common trigger).
///
/// Deliberately narrower than a full DOM/`parse_from_html` merge: a list-shell
/// page (LinkedIn search/collections) commonly carries its OWN SEO
/// `JobPosting` JSON-LD for an unrelated job (the first list result), and
/// `parse_from_html`'s precedence lets JSON-LD override the hint — so calling
/// it on the whole shell document risks silently importing the wrong job. The
/// caller extracts via [`crate::scraping::scrape_url::job_root_generic_html`]
/// instead, which reads ONLY the hinted subtree, never the document's JSON-LD
/// /`__NEXT_DATA__`/whole-page heuristics.
///
/// `resolve`'s non-empty title/description win; a field it left empty is
/// filled from the hint — never the other way around. `company`/`location`
/// are untouched (the hint doesn't extract them — they stay whatever `resolve`
/// produced, including its own host-based company fallback). Returns `None`
/// when `resolve` is `None` — there is no base posting's identity
/// (id/url/source/company) to attach the hint to, so the stub/partial path
/// covers that case instead of synthesizing a whole posting from a
/// list-shell's hint alone. Pure — no `AppHandle`/network — so it's directly
/// unit-testable.
pub(super) fn merge_resolve_with_hint(
    resolve: Option<crate::scraping::types::JobPosting>,
    hint_title: String,
    hint_description: Option<String>,
) -> Option<crate::scraping::types::JobPosting> {
    let mut base = resolve?;
    if base.title.trim().is_empty() && !hint_title.trim().is_empty() {
        base.title = hint_title;
    }
    if base
        .description
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        base.description = hint_description;
    }
    Some(base)
}

#[cfg(test)]
mod tests;
