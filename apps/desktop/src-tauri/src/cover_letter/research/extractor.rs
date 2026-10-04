/// Extract the company name and role title from raw job ad text.
/// Uses lightweight heuristics — no LLM call, no I/O.
pub struct JobAdMeta {
    pub company: String,
    pub role: String,
}

pub fn extract(job_ad: &str) -> JobAdMeta {
    let company = sanitized(extract_company(job_ad));
    let role = sanitized(extract_role(job_ad));
    JobAdMeta { company, role }
}

/// Apply/nav chrome that a scraped ad's first line frequently is. Lowercased,
/// matched whole — these are the exact strings a heuristic returns instead of a
/// title, seen verbatim in a support bundle (`Jetzt bewerben`,
/// `[← Alle offenen Stellen](/karriere)`).
const CHROME: &[&str] = &[
    "jetzt bewerben",
    "bewerben",
    "apply",
    "apply now",
    "apply for this job",
    "back to jobs",
    "alle offenen stellen",
    "view all jobs",
    "all openings",
    "open positions",
    "offene stellen",
    "share this job",
    "solliciteer",
];

/// Drop a candidate that is page chrome, markup, or prose rather than a name or
/// a job title. Empty means "nothing usable found", which every caller already
/// handles by skipping research entirely.
///
/// Both fields feed the same provider search, and a bad value there is worse
/// than none: it produces a confident brief about the wrong subject. Observed
/// values this rejects: `Jetzt bewerben` (an apply button),
/// `[← Alle offenen Stellen](/karriere)` (a nav link), and
/// `Please note: Fluent Dutch language skills are required for this role.`
/// Pure + unit-tested.
fn sanitized(candidate: String) -> String {
    let c = candidate.trim();
    if c.is_empty() {
        return String::new();
    }
    // Markdown/HTML that leaked out of the scraped page.
    if c.contains("](") || c.starts_with('[') || c.contains("**") || c.contains('<') {
        return String::new();
    }
    // A sentence, not a label.
    if c.ends_with('.') || c.ends_with('!') || c.ends_with('?') {
        return String::new();
    }
    let lower = c.to_lowercase();
    if CHROME.contains(&lower.as_str()) {
        return String::new();
    }
    c.to_string()
}

/// Whether byte offset `at` in `text` begins a word — i.e. it is the start of
/// the string or the preceding char is not alphanumeric. `at` must be a char
/// boundary (every caller gets it from [`find_ascii_ci`], which guarantees that).
fn starts_at_word_boundary(text: &str, at: usize) -> bool {
    text[..at]
        .chars()
        .next_back()
        .is_none_or(|c| !c.is_alphanumeric())
}

/// ASCII case-insensitive substring search. `needle` MUST be ASCII.
///
/// Returns a byte offset into `haystack` that is a valid char boundary,
/// and `offset + needle.len()` is also a boundary (the matched window is
/// all-ASCII, since a non-ASCII byte can never `eq_ignore_ascii_case` an
/// ASCII one). Safe to use for slicing `haystack` without panicking on
/// multibyte chars — the returned index comes from `char_indices` so it is
/// always a valid char start, and advancing by `needle.len()` (pure ASCII)
/// stays on a boundary.
fn find_ascii_ci(haystack: &str, needle: &str) -> Option<usize> {
    let (h, n) = (haystack.as_bytes(), needle.as_bytes());
    if n.is_empty() {
        return Some(0);
    }
    haystack.char_indices().find_map(|(i, _)| {
        h.get(i..i + n.len())
            .filter(|w| w.eq_ignore_ascii_case(n))
            .map(|_| i)
    })
}

fn extract_company(text: &str) -> String {
    // Priority 1: explicit labels
    for line in text.lines().take(40) {
        for prefix in &["company:", "employer:", "organization:", "at "] {
            // FIX: use find_ascii_ci so the offset is valid in `line` (not in a
            // temporary lowercased copy), then slice `line` directly.  The old
            // code applied a per-line byte offset (from lower.find) into the
            // FULL `text` string — a completely wrong target — which panics
            // when `text` starts with multibyte chars (e.g. an emoji headline).
            // `starts_at_word_boundary`: without it the bare `"at "` prefix
            // matches INSIDE a word — "Wh(at )You'll Do", a near-universal job-ad
            // heading, yielded a company of "You'll Do". The renderer's own
            // fallback regex had the identical bug; this is the same defect, so
            // it gets the same fix on both sides rather than one patch.
            if let Some(i) =
                find_ascii_ci(line, prefix).filter(|i| starts_at_word_boundary(line, *i))
            {
                let rest = &line[i + prefix.len()..];
                let candidate = rest
                    .split(['|', '\n', ',', '('])
                    .next()
                    .unwrap_or("")
                    .trim();
                if !candidate.is_empty() && candidate.len() < 80 {
                    return candidate.to_string();
                }
            }
        }
    }

    // Priority 2: "X is hiring" / "X is looking for" pattern
    let patterns = [
        " is hiring",
        " is looking for",
        " are hiring",
        " seeks a",
        " seeks an",
    ];
    // FIX: use find_ascii_ci so the offset is valid in `text` directly — the old
    // code computed `idx` from `text.to_lowercase()`, which can be a different
    // byte length from `text` for non-ASCII input (e.g. ẞ→ß contracts, İ→i̇
    // expands), causing the slice to land on a non-char-boundary and panic.
    for pat in &patterns {
        if let Some(idx) = find_ascii_ci(text, pat) {
            // Walk backwards to find the start of the company name phrase.
            let before = &text[..idx];
            let start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let candidate = before[start..].trim();
            if !candidate.is_empty() && candidate.len() < 80 {
                return candidate.to_string();
            }
        }
    }

    // Priority 3: "Join {Company}" / "About {Company}"
    // Safe as-is: prefix is ASCII, starts_with guarantees the first prefix.len()
    // bytes of the trimmed line are the ASCII prefix — a valid char boundary.
    for line in text.lines().take(60) {
        let lower = line.trim().to_lowercase();
        for prefix in &["join ", "about "] {
            if lower.starts_with(prefix) {
                let candidate = line.trim()[prefix.len()..].trim();
                if !candidate.is_empty() && candidate.len() < 80 {
                    return candidate.to_string();
                }
            }
        }
    }

    String::new()
}

fn extract_role(text: &str) -> String {
    // Priority 1: explicit labels on their own line or after a colon
    for line in text.lines().take(20) {
        for prefix in &[
            "job title:",
            "position:",
            "role:",
            "title:",
            "we are hiring a",
            "we are looking for a",
            "we're hiring a",
            "we're looking for a",
        ] {
            // FIX: same as extract_company priority 1 — use find_ascii_ci so the
            // offset is valid in `line`, then slice `line` directly.
            if let Some(i) = find_ascii_ci(line, prefix) {
                let rest = &line[i + prefix.len()..];
                let candidate = rest.split(['\n', '|', '(']).next().unwrap_or("").trim();
                if !candidate.is_empty() && candidate.len() < 100 {
                    return candidate.to_string();
                }
            }
        }
    }

    // Priority 2: first non-empty line (job ads usually start with the title)
    for line in text.lines().take(5) {
        let t = line.trim();
        if !t.is_empty() && t.len() < 100 {
            return t.to_string();
        }
    }

    String::new()
}

#[cfg(test)]
mod tests;
