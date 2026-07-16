//! Pure email parsing/fingerprinting — decode a fetched header/body, decide
//! whether it LOOKS like an application-confirmation email, and pull rough
//! company/title candidates out of it. No IMAP/Tauri/network coupling, and
//! every fn here is total (never panics on malformed input — a hostile or
//! merely weird email is just a non-match, never a crash).
//!
//! **Privacy**: never logs subject/sender/body content — content stays
//! in-process, consumed only by [`crate::email_watch::matcher`] to produce an
//! application id (or nothing). Callers must not log the return values of
//! [`parse_header`]/[`parse_body_text`] either.

use std::sync::LazyLock;

use mail_parser::MessageParser;
use regex::Regex;

/// Decoded fields from a fetched `HEADER.FIELDS (FROM SUBJECT DATE
/// MESSAGE-ID)` block. `subject` is already RFC2047-decoded (mail-parser
/// handles encoded-word decoding as part of parsing).
#[derive(Debug, Clone, Default)]
pub struct EmailHeader {
    pub subject: String,
    pub from_name: Option<String>,
    /// Lowercased domain part of the `From` address (e.g. `"greenhouse.io"`).
    pub from_domain: Option<String>,
    pub message_id: Option<String>,
}

/// Parse a raw header-only byte block (as returned by
/// `imap_client::fetch_headers_since`) into [`EmailHeader`]. `None` only if
/// mail-parser can't construct even an empty message from the bytes (should
/// not happen for real server responses, but never trusted blindly).
pub fn parse_header(raw: &[u8]) -> Option<EmailHeader> {
    let message = MessageParser::default().parse(raw)?;
    let subject = message.subject().unwrap_or_default().to_string();
    let from = message.from().and_then(|addr| addr.first());
    let from_name = from
        .and_then(|a| a.name())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let from_domain = from
        .and_then(|a| a.address())
        .and_then(|addr| addr.rsplit_once('@'))
        .map(|(_, domain)| domain.to_lowercase());
    let message_id = message.message_id().map(str::to_string);
    Some(EmailHeader {
        subject,
        from_name,
        from_domain,
        message_id,
    })
}

/// Parse a raw FULL message (`BODY.PEEK[]`, as returned by
/// `imap_client::fetch_bodies`) and return its plain-text body (mail-parser
/// converts an HTML-only body to text automatically when no text/plain part
/// exists). `None` if unparseable or the message truly has no body part.
pub fn parse_body_text(raw: &[u8]) -> Option<String> {
    let message = MessageParser::default().parse(raw)?;
    message.body_text(0).map(|cow| cow.into_owned())
}

// ── Fingerprint (the subject-regex gate; domain is a boost, never a gate) ──

/// Subject substrings/phrases (EN + DE) that mark a message as a plausible
/// application-confirmation email. Case-insensitive, Unicode-aware (so
/// `(?i)für` matches `FÜR`/`Für`). This is the ONLY gate — a hit here is
/// required before any body is fetched or any matching is attempted.
static SUBJECT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)thank you for applying",
        r"(?i)application (?:was |has been )?(?:received|submitted)",
        r"(?i)your application to",
        r"(?i)ihre bewerbung",
        r"(?i)bewerbung (?:ist )?(?:eingegangen|erhalten)",
        r"(?i)danke für ihre bewerbung",
    ]
    .iter()
    .map(|p| Regex::new(p).expect("static subject pattern is valid"))
    .collect()
});

/// Sender domains known to be ATS/job-board confirmation senders. Only
/// `greenhouse.io`/`greenhouse-mail.io` are independently verified (real
/// Greenhouse confirmation emails); the rest are commonly-cited folklore for
/// other ATS/board vendors — kept anyway since a hint only ever BOOSTS score,
/// never gates a match, so an unverified/wrong entry here can't create a
/// false positive on its own.
const DOMAIN_HINTS: &[&str] = &[
    "greenhouse.io",
    "greenhouse-mail.io",
    "lever.co",
    "myworkday.com",
    "linkedin.com",
    "indeed.com",
];

/// The result of fingerprinting one [`EmailHeader`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Fingerprint {
    subject_matched: bool,
    /// Sender domain is a known ATS hint. Informational/boost-only — see
    /// [`crate::email_watch::matcher::best_match`], which adds a small,
    /// capped nudge to the company score when this is `true` and NEVER lets
    /// it substitute for a real company-token overlap.
    pub domain_hint: bool,
}

impl Fingerprint {
    /// Whether this message clears the fingerprint gate at all — the ONLY
    /// signal that decides whether a body fetch + match attempt happens.
    /// `domain_hint` never contributes to this — a hint alone is not enough.
    pub fn is_candidate(&self) -> bool {
        self.subject_matched
    }
}

pub fn fingerprint(header: &EmailHeader) -> Fingerprint {
    let subject_matched = SUBJECT_PATTERNS
        .iter()
        .any(|re| re.is_match(&header.subject));
    let domain_hint = header.from_domain.as_deref().is_some_and(|d| {
        DOMAIN_HINTS
            .iter()
            .any(|hint| d == *hint || d.ends_with(&format!(".{hint}")))
    });
    Fingerprint {
        subject_matched,
        domain_hint,
    }
}

// ── Candidate extraction (company/title guesses — matcher does the real gating) ──

/// Rough company/title guesses pulled from the subject, a body snippet, or
/// the sender's display name. Deliberately best-effort: [`crate::email_watch::
/// matcher`] does the real (token-Jaccard, thresholded) matching against the
/// user's saved applications, so an imperfect extraction here just means a
/// missed match, never a wrong one.
#[derive(Debug, Clone, Default)]
pub struct Candidates {
    pub company: Option<String>,
    pub title: Option<String>,
}

/// "title at/bei company" and "company only" phrase patterns, EN then DE,
/// tried in order — the first pattern that captures a company wins.
///
/// `regex` has no lookaround, so every capture is lazy (`{1,60}?`) and
/// terminated by an explicit trailing boundary — end of text, punctuation, or
/// a common continuation word (`was received`, `ist eingegangen`, …) — so a
/// greedy/lazy capture never swallows the rest of the sentence. The boundary
/// itself sits OUTSIDE the named group, so it never pollutes the captured
/// text.
static TITLE_COMPANY_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    const EN_BOUNDARY: &str = r"(?:$|[.,!?;:]|\s+(?:was|is|has|will|being|which|and)\b)";
    const DE_BOUNDARY: &str = r"(?:$|[.,!?;:]|\s+(?:ist|war|wurde|wird|und)\b)";
    [
        // EN, title+company: "applying for/to (the) Software Engineer position at Acme Corp"
        format!(
            r"(?i)\bapplying\s+(?:for|to)\s+(?:the\s+)?(?P<title>[\p{{L}}][\p{{L}}\p{{N}} &/,.'-]{{1,60}}?)\s+(?:position\s+|role\s+)?at\s+(?P<company>[\p{{L}}][\p{{L}}\p{{N}} &/,.'-]{{1,60}}?){EN_BOUNDARY}"
        ),
        // EN, company only: "Your application to Acme Corp" / "application with Acme Corp"
        format!(
            r"(?i)\bapplication\s+(?:to|with|for)\s+(?P<company>[\p{{L}}][\p{{L}}\p{{N}} &/,.'-]{{1,60}}?){EN_BOUNDARY}"
        ),
        // DE, title+company: "Bewerbung als/für Software Engineer bei Acme GmbH"
        format!(
            r"(?i)\bbewerbung\s+(?:als|für)\s+(?P<title>[\p{{L}}][\p{{L}}\p{{N}} &/,.'-]{{1,60}}?)\s+bei\s+(?P<company>[\p{{L}}][\p{{L}}\p{{N}} &/,.'-]{{1,60}}?){DE_BOUNDARY}"
        ),
        // DE, company only: "Ihre Bewerbung bei Acme GmbH"
        format!(
            r"(?i)\bbewerbung\s+bei\s+(?P<company>[\p{{L}}][\p{{L}}\p{{N}} &/,.'-]{{1,60}}?){DE_BOUNDARY}"
        ),
    ]
    .iter()
    .map(|p| Regex::new(p).expect("static title/company pattern is valid"))
    .collect()
});

fn clean_capture(s: &str) -> String {
    s.trim()
        .trim_end_matches(['.', '!', ',', ':', ';'])
        .trim()
        .to_string()
}

fn extract_from_text(text: &str) -> Candidates {
    for re in TITLE_COMPANY_PATTERNS.iter() {
        if let Some(caps) = re.captures(text) {
            let company = caps.name("company").map(|m| clean_capture(m.as_str()));
            if company.is_some() {
                let title = caps.name("title").map(|m| clean_capture(m.as_str()));
                return Candidates { company, title };
            }
        }
    }
    Candidates::default()
}

/// Suffixes stripped from a sender display name before treating what's left
/// as a company candidate — e.g. `"Acme Corp Careers"` → `"Acme Corp"`.
const SENDER_NAME_SUFFIXES: &[&str] = &[
    " careers",
    " recruiting",
    " talent acquisition",
    " talent team",
    " hr team",
    " hr",
    " jobs",
    " recruitment",
    " team",
];

fn company_from_sender_name(name: Option<&str>) -> Option<String> {
    let name = name?.trim();
    if name.is_empty() {
        return None;
    }
    let lower = name.to_lowercase();
    let mut cut = name.len();
    for suffix in SENDER_NAME_SUFFIXES {
        if lower.ends_with(suffix) {
            cut = cut.min(name.len() - suffix.len());
        }
    }
    let trimmed = name[..cut].trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("no-reply")
        || trimmed.eq_ignore_ascii_case("noreply")
    {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Byte-boundary-safe prefix (never splits a multi-byte UTF-8 char) — mirrors
/// `applications::clamp_job_description`'s truncation approach.
fn safe_prefix(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// How much of a body snippet is scanned for a phrase-pattern match (the
/// subject is tried first and is usually enough — this is a bounded
/// fallback, not a full-body scan).
const BODY_SNIPPET_BYTES: usize = 500;

/// Extract company/title candidates: try the subject, then (only if the
/// subject yielded no company) a bounded body snippet, then fall back to a
/// company guess derived from the sender's display name.
pub fn extract_candidates(
    subject: &str,
    body_text: Option<&str>,
    from_name: Option<&str>,
) -> Candidates {
    let mut candidates = extract_from_text(subject);
    if candidates.company.is_none() {
        if let Some(body) = body_text {
            let snippet = extract_from_text(safe_prefix(body, BODY_SNIPPET_BYTES));
            if candidates.company.is_none() {
                candidates.company = snippet.company;
            }
            if candidates.title.is_none() {
                candidates.title = snippet.title;
            }
        }
    }
    if candidates.company.is_none() {
        candidates.company = company_from_sender_name(from_name);
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── fingerprint: positive (EN + DE) ─────────────────────────────────────

    fn header(subject: &str, from_domain: Option<&str>) -> EmailHeader {
        EmailHeader {
            subject: subject.to_string(),
            from_name: None,
            from_domain: from_domain.map(str::to_string),
            message_id: None,
        }
    }

    #[test]
    fn fingerprint_matches_each_en_phrase() {
        for subject in [
            "Thank you for applying!",
            "Your application has been received",
            "Your application was submitted",
            "Your application to Acme Corp",
        ] {
            assert!(
                fingerprint(&header(subject, None)).is_candidate(),
                "expected a match for {subject:?}"
            );
        }
    }

    #[test]
    fn fingerprint_matches_each_de_phrase() {
        for subject in [
            "Ihre Bewerbung bei Acme GmbH",
            "Ihre Bewerbung ist eingegangen",
            "Bewerbung erhalten",
            "Danke für Ihre Bewerbung!",
        ] {
            assert!(
                fingerprint(&header(subject, None)).is_candidate(),
                "expected a match for {subject:?}"
            );
        }
    }

    #[test]
    fn fingerprint_is_case_insensitive_including_umlauts() {
        assert!(fingerprint(&header("DANKE FÜR IHRE BEWERBUNG", None)).is_candidate());
    }

    // ── fingerprint: negative / near-miss ───────────────────────────────────

    #[test]
    fn fingerprint_rejects_unrelated_subjects() {
        for subject in [
            "Your weekly newsletter",
            "Meeting rescheduled to 3pm",
            "Your order has shipped",
        ] {
            assert!(
                !fingerprint(&header(subject, None)).is_candidate(),
                "did not expect a match for {subject:?}"
            );
        }
    }

    #[test]
    fn fingerprint_rejects_a_near_miss_that_only_mentions_applying_in_passing() {
        // Contains "applying" but not the gated phrase "thank you for applying".
        assert!(!fingerprint(&header("Tips for applying to jobs this year", None)).is_candidate());
    }

    #[test]
    fn domain_hint_boosts_but_never_gates() {
        // A known-ATS domain with a subject that does NOT match any fingerprint
        // phrase must still be rejected outright.
        let fp = fingerprint(&header("Your weekly digest", Some("greenhouse.io")));
        assert!(fp.domain_hint);
        assert!(!fp.is_candidate());
    }

    #[test]
    fn domain_hint_true_for_verified_and_folklore_domains_false_for_unknown() {
        assert!(fingerprint(&header("x", Some("greenhouse.io"))).domain_hint);
        assert!(fingerprint(&header("x", Some("mail.greenhouse-mail.io"))).domain_hint);
        assert!(fingerprint(&header("x", Some("lever.co"))).domain_hint);
        assert!(!fingerprint(&header("x", Some("example.com"))).domain_hint);
    }

    // ── extract_candidates ───────────────────────────────────────────────────

    #[test]
    fn extracts_title_and_company_from_an_en_subject() {
        let c = extract_candidates(
            "Thank you for applying for the Software Engineer position at Acme Corp!",
            None,
            None,
        );
        assert_eq!(c.company.as_deref(), Some("Acme Corp"));
        assert_eq!(c.title.as_deref(), Some("Software Engineer"));
    }

    #[test]
    fn extracts_company_only_from_an_en_subject() {
        let c = extract_candidates("Your application to Acme Corp was received", None, None);
        assert_eq!(c.company.as_deref(), Some("Acme Corp"));
    }

    #[test]
    fn extracts_title_and_company_from_a_de_subject() {
        let c = extract_candidates(
            "Ihre Bewerbung als Software Engineer bei Acme GmbH",
            None,
            None,
        );
        assert_eq!(c.company.as_deref(), Some("Acme GmbH"));
        assert_eq!(c.title.as_deref(), Some("Software Engineer"));
    }

    #[test]
    fn extracts_company_only_from_a_de_subject() {
        let c = extract_candidates("Ihre Bewerbung bei Acme GmbH ist eingegangen", None, None);
        assert_eq!(c.company.as_deref(), Some("Acme GmbH"));
    }

    #[test]
    fn falls_back_to_the_body_snippet_when_the_subject_has_no_company() {
        let c = extract_candidates(
            "Application received",
            Some("Thanks for applying! Your application to Acme Corp is being reviewed."),
            None,
        );
        assert_eq!(c.company.as_deref(), Some("Acme Corp"));
    }

    #[test]
    fn falls_back_to_a_stripped_sender_display_name() {
        let c = extract_candidates("Application received", None, Some("Acme Corp Careers"));
        assert_eq!(c.company.as_deref(), Some("Acme Corp"));
    }

    #[test]
    fn sender_name_fallback_rejects_a_bare_noreply() {
        let c = extract_candidates("Application received", None, Some("no-reply"));
        assert_eq!(c.company, None);
    }

    #[test]
    fn extract_candidates_is_none_when_nothing_matches_anywhere() {
        let c = extract_candidates("Application received", Some("Please wait."), None);
        assert_eq!(c.company, None);
        assert_eq!(c.title, None);
    }

    // ── parse_header / parse_body_text ──────────────────────────────────────

    #[test]
    fn parse_header_decodes_rfc2047_subject_and_lowercases_the_domain() {
        let raw = b"From: Acme Careers <Careers@ACME.example.com>\r\n\
Subject: =?UTF-8?B?VGhhbmsgeW91IGZvciBhcHBseWluZyE=?=\r\n\
Message-ID: <abc123@example.com>\r\n\
\r\n";
        let header = parse_header(raw).expect("should parse a minimal header block");
        assert_eq!(header.subject, "Thank you for applying!");
        assert_eq!(header.from_name.as_deref(), Some("Acme Careers"));
        assert_eq!(header.from_domain.as_deref(), Some("acme.example.com"));
        assert_eq!(header.message_id.as_deref(), Some("abc123@example.com"));
    }

    #[test]
    fn parse_body_text_extracts_plain_text() {
        let raw = b"From: Acme <careers@acme.example.com>\r\n\
Subject: Your application to Acme Corp\r\n\
Content-Type: text/plain; charset=\"us-ascii\"\r\n\
\r\n\
Thanks for applying to Acme Corp!\r\n";
        let text = parse_body_text(raw).expect("should parse a plain-text body");
        assert!(text.contains("Thanks for applying to Acme Corp!"));
    }
}
