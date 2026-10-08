//! A thin JSON-LD `description` yields to a much longer body of the same posting (#1400).

use super::generic::{main_content_text, readability_content_text};

/// A JSON-LD description with fewer plain-text chars than this may be a thin summary.
const THIN_JSON_LD_MAX_CHARS: usize = 1000;
/// ...and is replaced only when the page body has more than this many times its chars.
const THIN_JSON_LD_BODY_RATIO: usize = 4;
/// A real posting's heading sits at the top: the title must appear in this many leading body chars.
const TITLE_HEAD_CHARS: usize = 300;
/// Chars of the JSON-LD description that must reappear in the body as proof it is the same posting.
const SAME_POSTING_PROBE_CHARS: usize = 40;

/// Lowercased alphanumerics only, so markdown/whitespace/punctuation differences don't matter.
fn normalized(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Lowercase alphanumeric words joined by single spaces and padded with one, so a
/// `contains` only matches whole words ("engineer" never matches "engineering").
fn words(s: &str) -> String {
    let lower = s.to_lowercase();
    let w: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    format!(" {} ", w.join(" "))
}

/// The page body to use instead of a thin JSON-LD `description` (#1400), or `None` to keep it.
/// Requires the body to be much longer AND to show evidence of being the same posting
/// (the JSON-LD title, or — for a readability extraction only — the description's opening),
/// so nav, related-jobs or listing text can never replace a clean summary.
pub(super) fn fuller_body(url: &str, html: &str, jl_title: &str, desc: &str) -> Option<String> {
    let desc_len = desc.chars().count();
    if desc_len >= THIN_JSON_LD_MAX_CHARS {
        return None;
    }
    let (body, from_readability) = match readability_content_text(url, html) {
        Some(b) => (b, true),
        None => (main_content_text(html)?, false),
    };
    if body.chars().count() <= desc_len * THIN_JSON_LD_BODY_RATIO {
        return None;
    }
    let body_norm = normalized(&body);
    let title = words(jl_title);
    let head: String = words(&body).chars().take(TITLE_HEAD_CHARS).collect();
    let title_match = !title.trim().is_empty() && head.contains(&title);
    let probe: String = normalized(desc)
        .chars()
        .take(SAME_POSTING_PROBE_CHARS)
        .collect();
    let desc_match = !probe.is_empty() && body_norm.contains(&probe);
    (title_match || (from_readability && desc_match)).then_some(body)
}
