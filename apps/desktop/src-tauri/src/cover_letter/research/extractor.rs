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
mod tests {
    use super::*;

    // ── find_ascii_ci unit tests ───────────────────────────────────────────────

    #[test]
    fn find_ascii_ci_basic() {
        // Case-insensitive ASCII hit.
        assert_eq!(find_ascii_ci("Hello World", "world"), Some(6));
        assert_eq!(find_ascii_ci("COMPANY: Acme", "company:"), Some(0));
        // Miss.
        assert_eq!(find_ascii_ci("no match here", "xyz"), None);
        // Empty needle → always 0.
        assert_eq!(find_ascii_ci("anything", ""), Some(0));
    }

    #[test]
    fn find_ascii_ci_multibyte_before_match() {
        // Multibyte chars before the match must not perturb the returned offset.
        // The returned index must be a valid char boundary in `haystack`.
        let s = "🎤 Company: Acme Corp";
        let idx = find_ascii_ci(s, "company:").expect("should match");
        // Verify the offset is valid (slicing must not panic).
        let _ = &s[idx..];
        // Verify the matched bytes reproduce the original casing.
        assert_eq!(&s[idx..idx + "company:".len()], "Company:");
    }

    // ── extract_company priority 1 (L19 panic repro) ──────────────────────────
    //
    // Old bug: `lower.find(prefix)` returned offset `i` into the PER-LINE
    // lowercased string; the old code applied that offset to the FULL `text`
    // string.  On "🎤 Jobs\nat Acme Corp":
    //   • line "at Acme Corp": lower.find("at ") == 0, prefix.len() == 3
    //   • &text[0 + 3..] → text[3] == 0xA4, the last byte of 🎤
    //     (U+1F3A4 = [F0 9F 8E A4]), which is a UTF-8 continuation byte → panic.
    #[test]
    fn company_p1_multiline_emoji_headline_no_panic() {
        let ad = "🎤 Jobs\nat Acme Corp";
        let meta = extract(ad);
        assert!(
            meta.company.contains("Acme Corp"),
            "expected 'Acme Corp', got {:?}",
            meta.company
        );
    }

    // ── extract_company priority 2 (L41 panic repro) ──────────────────────────
    //
    // Old bug: `text.to_lowercase().find(pat)` returned `idx` into the lowercased
    // copy; `&text[..idx]` used that offset on the ORIGINAL string.
    // For "ẞ🎤 is hiring a dev":
    //   • ẞ (U+1E9E, 3 bytes [E1 BA 9E]) → ß (U+00DF, 2 bytes [C3 9F]): −1 byte
    //   • lowercased = "ß🎤 is hiring a dev"; " is hiring" starts at byte 6
    //   • &text[..6] → text[6] == 0xA4, last byte of 🎤 [F0 9F 8E A4]
    //     (a continuation byte) → panic.
    #[test]
    fn company_p2_is_hiring_lowercase_contraction_no_panic() {
        // ẞ→ß contracts by 1 byte, causing the lowercased-string offset to land
        // inside the 4-byte 🎤 in the original text.  Must not panic.
        let ad = "ẞ🎤 is hiring a dev";
        let _meta = extract(ad);
    }

    // ── extract_role priority 1 (L82 panic repro) ─────────────────────────────
    //
    // Old bug: identical to the L19 bug — line-local offset applied to full `text`.
    // For "🎤🎤\nrole: Backend Engineer":
    //   • line "role: Backend Engineer": lower.find("role:") == 0, prefix.len() == 5
    //   • &text[0 + 5..] → 2×🎤 = 8 bytes (00–07), text[5] == 0x9F
    //     (byte 2 of the second 🎤 [F0 9F 8E A4], a continuation byte) → panic.
    #[test]
    fn role_p1_multiline_emoji_headline_no_panic() {
        let ad = "🎤🎤\nrole: Backend Engineer";
        let meta = extract(ad);
        assert!(
            meta.role.contains("Backend Engineer"),
            "expected role containing 'Backend Engineer', got {:?}",
            meta.role
        );
    }

    // ── edge case ─────────────────────────────────────────────────────────────

    /// All-emoji input — no labels match; must return an empty company string
    /// and not panic.
    #[test]
    fn all_emoji_no_panic() {
        let meta = extract("🎉🎊🎈🎁🎀");
        assert!(
            meta.company.is_empty(),
            "expected empty company, got {:?}",
            meta.company
        );
    }

    // ── Chrome / prose rejection (the values a real bundle searched for) ───────
    //
    // Every string here was the `role=` or `company=` a 2026-08-08 support bundle
    // actually sent to the provider's web search. A wrong subject is worse than
    // none: it returns a confident brief about something that is not the employer.

    #[test]
    fn apply_buttons_and_nav_links_are_not_roles() {
        for ad in [
            "Jetzt bewerben\n\nWir suchen einen Entwickler",
            "[← Alle offenen Stellen](/karriere)\n\nSoftware Engineer",
            "Apply now\n\nWe build things",
        ] {
            let meta = extract(ad);
            assert!(
                meta.role.is_empty() || !meta.role.contains("bewerben"),
                "chrome must not become the role, got {:?}",
                meta.role
            );
            assert!(
                !meta.role.contains("]("),
                "a markdown link must not become the role, got {:?}",
                meta.role
            );
        }
    }

    #[test]
    fn a_prose_sentence_is_not_a_role() {
        let ad = "Please note: Fluent Dutch language skills are required for this role.";
        assert_eq!(extract(ad).role, "");
    }

    #[test]
    fn a_real_first_line_title_is_still_extracted() {
        // The gate must not eat the common, correct case.
        let meta = extract("Senior Backend Engineer\n\nWe are a payments company.");
        assert_eq!(meta.role, "Senior Backend Engineer");
    }

    #[test]
    fn labelled_company_and_role_still_win() {
        let meta = extract("Job title: Staff Engineer\nCompany: Codefield\n");
        assert_eq!(meta.role, "Staff Engineer");
        assert_eq!(meta.company, "Codefield");
    }

    // ── The `at` word-boundary bug (identical to the renderer's own regex) ─────

    #[test]
    fn at_inside_a_word_is_not_a_company_marker() {
        // "Wh(at) You'll Do" is a near-universal job-ad heading. The bare `"at "`
        // prefix matched inside it and yielded a company of "You'll Do", which a
        // real session then researched three times.
        let meta = extract("What You'll Do\n\nBuild and ship features.");
        assert_ne!(meta.company, "You'll Do");
        assert_eq!(meta.company, "");
    }

    #[test]
    fn at_as_a_real_word_still_extracts_the_company() {
        // The boundary fix must not break the case the prefix exists for.
        let meta = extract("Senior Engineer at Codefield\n");
        assert_eq!(meta.company, "Codefield");
    }
}
