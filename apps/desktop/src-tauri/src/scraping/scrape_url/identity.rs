//! `(board, id)` posting identity (issue #1166) — split out of `scrape_url`'s
//! own `mod.rs` purely to keep that file under the R8 LOC hard cap
//! (`docs/architecture-rules.md`); re-exported from there (`pub use
//! identity::job_identity`) so every caller still writes
//! `crate::scraping::scrape_url::job_identity`. This is the SAME per-board
//! url knowledge `canonical_job_url` owns, just answering a different
//! question — see [`job_identity`]'s own doc for the split.

/// The trailing numeric id from a LinkedIn `/jobs/view/…` path segment — the
/// plain numeric form (`/jobs/view/12345`) or LinkedIn's slugged form
/// (`/jobs/view/senior-engineer-at-acme-4464018189`, where the id is always
/// the hyphen-delimited digit run LinkedIn appends after the human-readable
/// slug). `None` when the path isn't a `/jobs/view/` page or its last
/// segment carries no id in either shape.
fn linkedin_view_id(path: &str) -> Option<String> {
    if !path.contains("/jobs/view/") {
        return None;
    }
    let last = path.trim_end_matches('/').rsplit('/').next()?;
    if !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) {
        return Some(last.to_string());
    }
    let digits = last.rsplit('-').next()?;
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then(|| digits.to_string())
}

/// Canonical `(board, id)` identity for a posting url, on the boards that own
/// a stable id space (issue #1166). Sibling to
/// [`super::canonical_job_url`] but answers a different question: that
/// function REWRITES a shell/search url into the direct view url and
/// deliberately returns `None` once the input is already the direct form
/// (nothing left to rewrite); this extracts the SAME id from either shape,
/// so a caller comparing two postings can match by identity instead of by
/// the byte-exact url string. `None` means "no stable id for this board" —
/// the caller falls back to
/// [`crate::applications::normalize_job_url`]'s string compare.
///
/// Tolerates a missing or `http` scheme (retries with an `https://` prefix
/// when the bare parse fails) — unlike [`super::canonical_job_url`], every
/// existing caller of which always hands it a url that already carries a
/// scheme, this is asked to compare a url a caller may have pasted without
/// one.
pub fn job_identity(url: &str) -> Option<(&'static str, String)> {
    let u = reqwest::Url::parse(url)
        .or_else(|_| reqwest::Url::parse(&format!("https://{url}")))
        .ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    let query = |key: &str| {
        u.query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    };

    if host == "linkedin.com" || host == "www.linkedin.com" || host.ends_with(".linkedin.com") {
        if let Some(id) = linkedin_view_id(u.path()) {
            return Some(("linkedin", id));
        }
        if let Some(id) = query("currentJobId") {
            if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
                return Some(("linkedin", id));
            }
        }
        return None;
    }

    if host == "indeed.com" || host.ends_with(".indeed.com") {
        let id = query("jk").or_else(|| query("vjk"))?;
        if !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Some(("indeed", id));
        }
        return None;
    }

    None
}
