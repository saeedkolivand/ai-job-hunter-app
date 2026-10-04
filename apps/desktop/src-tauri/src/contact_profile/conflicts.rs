//! Resolvable mismatches on import: where the imported résumé's identity fields
//! CONFLICT with what the saved profile already holds.

use super::classify::host_of;
use super::{non_empty, ContactFieldConflict, ContactProfile};

/// Normalize an email for comparison: trim + lowercase.
fn norm_email(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Normalize a phone for comparison: digits only (drops spaces, `()`, `-`, `+`,
/// `.`, and any other formatting), so `+1 (555) 123-4567` == `15551234567`.
fn norm_phone(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

/// Normalize a URL for comparison: lowercase host (via [`host_of`], which already
/// strips scheme + `www.`) joined with the trimmed path (lowercased, trailing
/// slash removed). Keeps `http` vs `https`, a trailing slash, and a `www.` prefix
/// from registering as conflicts, e.g. `linkedin.com/in/x`,
/// `https://www.linkedin.com/in/x/`, and `http://linkedin.com/in/x` all normalize
/// equal.
fn norm_url(s: &str) -> String {
    let host = host_of(s).unwrap_or_default();
    let lower = s.trim().to_lowercase();
    let no_scheme = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let path = no_scheme
        .split(['?', '#'])
        .next()
        .unwrap_or(no_scheme)
        .split_once('/')
        .map(|(_, rest)| rest)
        .unwrap_or("");
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        host
    } else {
        format!("{host}/{path}")
    }
}

/// Normalize a plain text value (full name / location) for comparison: trim +
/// lowercase (case-insensitive).
fn norm_text(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Detect the identity fields where `current` (the saved profile) and `suggested`
/// (the imported résumé's extracted contact) CONFLICT — i.e. BOTH are non-empty
/// after trimming AND their normalized values differ. Compares only the
/// single-valued identity fields (email, phone, linkedin, github, website, and
/// `location.default`); `extra_links` is a list merged by URL and is
/// never reported here. The conflict carries the ORIGINAL values so the UI shows
/// them as written. Conservative by design: scheme, trailing slash, and `www.`
/// differences on URLs are NOT conflicts (see [`norm_url`]).
pub fn detect_contact_conflicts(
    current: &ContactProfile,
    suggested: &ContactProfile,
) -> Vec<ContactFieldConflict> {
    let mut out = Vec::new();

    fn push_if_conflict(
        out: &mut Vec<ContactFieldConflict>,
        field: &str,
        cur: Option<&str>,
        sug: Option<&str>,
        normalize: impl Fn(&str) -> String,
    ) {
        if let (Some(cur), Some(sug)) = (cur, sug) {
            if normalize(cur) != normalize(sug) {
                out.push(ContactFieldConflict {
                    field: field.to_string(),
                    current: cur.to_string(),
                    suggested: sug.to_string(),
                });
            }
        }
    }

    push_if_conflict(
        &mut out,
        "email",
        non_empty(&current.email),
        non_empty(&suggested.email),
        norm_email,
    );
    push_if_conflict(
        &mut out,
        "phone",
        non_empty(&current.phone),
        non_empty(&suggested.phone),
        norm_phone,
    );
    push_if_conflict(
        &mut out,
        "linkedin",
        non_empty(&current.linkedin),
        non_empty(&suggested.linkedin),
        norm_url,
    );
    push_if_conflict(
        &mut out,
        "github",
        non_empty(&current.github),
        non_empty(&suggested.github),
        norm_url,
    );
    push_if_conflict(
        &mut out,
        "website",
        non_empty(&current.website),
        non_empty(&suggested.website),
        norm_url,
    );

    let cur_loc = current
        .location
        .as_ref()
        .map(|l| l.default.trim())
        .filter(|s| !s.is_empty());
    let sug_loc = suggested
        .location
        .as_ref()
        .map(|l| l.default.trim())
        .filter(|s| !s.is_empty());
    push_if_conflict(&mut out, "location", cur_loc, sug_loc, norm_text);

    out
}

#[cfg(test)]
mod tests;
