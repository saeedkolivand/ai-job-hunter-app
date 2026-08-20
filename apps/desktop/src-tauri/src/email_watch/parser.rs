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

use mail_parser::{HeaderForm, MessageParser};
use regex::Regex;

/// Decoded fields from a fetched `HEADER.FIELDS (FROM SUBJECT DATE
/// MESSAGE-ID AUTHENTICATION-RESULTS)` block. `subject` is already
/// RFC2047-decoded (mail-parser handles encoded-word decoding as part of
/// parsing).
#[derive(Debug, Clone, Default)]
pub struct EmailHeader {
    pub subject: String,
    pub from_name: Option<String>,
    /// Lowercased domain part of the `From` address (e.g. `"greenhouse.io"`).
    pub from_domain: Option<String>,
    pub message_id: Option<String>,
    /// Whether AT LEAST ONE `Authentication-Results` header on this message
    /// reports a DMARC `pass` result whose `header.from=` domain matches
    /// `from_domain` — see [`dmarc_pass_aligned`]'s doc for exactly what
    /// this does and does not prove, and
    /// [`crate::email_watch::auto_write::apply_matched_intent`]'s doc for
    /// why this (never `Fingerprint::domain_hint`) is the write gate.
    /// `false` when the header is absent, present but unparseable, or
    /// present without a `pass` — fails closed by construction (there is no
    /// "unknown" state; anything other than a confirmed pass is `false`).
    pub dmarc_pass: bool,
}

/// Cap on how many bytes of a decoded subject are kept before fingerprinting
/// or extraction ever sees it. The `regex` crate is itself immune to ReDoS
/// (no backtracking), so this isn't a catastrophic-backtracking concern —
/// it's the same "bound unbounded input" discipline as [`BODY_SNIPPET_BYTES`]
/// below, applied to a pathological/hostile subject header.
pub(super) const SUBJECT_MAX_BYTES: usize = 500;

/// Parse a raw header-only byte block (as returned by
/// `imap_client::fetch_headers_since`) into [`EmailHeader`]. `None` only if
/// mail-parser can't construct even an empty message from the bytes (should
/// not happen for real server responses, but never trusted blindly).
pub fn parse_header(raw: &[u8]) -> Option<EmailHeader> {
    let message = MessageParser::default().parse(raw)?;
    let subject = safe_prefix(message.subject().unwrap_or_default(), SUBJECT_MAX_BYTES).to_string();
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
    let auth_results: Vec<String> = message
        .header_as("Authentication-Results", HeaderForm::Raw)
        .into_iter()
        .filter_map(|hv| hv.as_text().map(str::to_string))
        .collect();
    let dmarc_pass = dmarc_pass_aligned(&auth_results, from_domain.as_deref());
    Some(EmailHeader {
        subject,
        from_name,
        from_domain,
        message_id,
        dmarc_pass,
    })
}

// ── DMARC authentication (the write-gate half — see `Fingerprint::write_gate_domain`) ──

/// Whether the TOPMOST `Authentication-Results` header on this message (and
/// ONLY the topmost — see below) reports a DMARC `pass` result ALIGNED to
/// `from_domain` (the visible `From:` domain, already lowercased by
/// [`parse_header`]). `auth_results` is in DOCUMENT order (verified against
/// `mail-parser`'s own header-collection code, which pushes each header as
/// it scans forward through the byte stream — never reversed, never
/// deduped; see this fn's test module for the citation).
///
/// **Only `auth_results.first()` is EVER consulted — never `.any()`.** Per
/// RFC 8601 §5, a consumer must trust ONLY the `Authentication-Results`
/// header added by its OWN receiving MTA, and must ignore every other
/// occurrence. The receiving MTA PREPENDS its stamp on arrival rather than
/// stripping what is already present, so in document order the FIRST
/// (topmost) occurrence is the one the user's own provider just added;
/// anything below it is an earlier hop's stamp OR attacker-composed text
/// that arrived as part of the message body/headers, indistinguishable from
/// the real thing by content alone. A previous version of this fn used
/// `.any()` over every occurrence — an attacker could satisfy the gate
/// simply by including their OWN forged `Authentication-Results` header
/// naming a write-gate domain, since nothing distinguished it from a
/// genuine stamp. **`.any()` must never come back here** — see this fn's
/// `uses_only_the_topmost_stamp`/`a_genuine_topmost_fail_is_not_overridden`
/// tests, which pin the exploit this closes as permanent regressions.
///
/// **The topmost entry itself is now read by a real RFC 8601 tokeniser**
/// ([`super::auth_results::dmarc_verdict`], a sibling module — not this
/// function, and not a substring scan). Three rounds of substring-scanning
/// each found a NEW way to be wrong, the last of which needed no forged
/// second header at all: a genuine, single, correctly-folded header could
/// still be misread, because an attacker-chosen envelope-from local part —
/// echoed VERBATIM by the receiving server's own authentic SPF evaluation,
/// inside a `(...)` comment or the `smtp.mailfrom=` value in that SAME
/// header — could contain the literal text `dmarc=pass header.from=...`,
/// and a scan has no way to know it is reading a comment's or a quoted
/// string's CONTENT rather than a real methodspec. `dmarc_verdict` tracks
/// comment nesting and quoted-string state as it walks, so text inside
/// either is never surfaced as a token — see that module's own doc and
/// test suite (including a permanent regression pinning this exact
/// exploit) for the structural fix.
///
/// **This gate is BEST-EFFORT, not a closed one — read this before trusting
/// the summary below it; re-derive it, do not just believe it.** This
/// file's own history is that a confident sentence here hid a live defect
/// through three prior review rounds. State the mechanism, not the
/// conclusion:
///
/// [`host_is_known_to_stamp`] proves exactly one thing: the account's
/// configured IMAP host is known to emit AT LEAST ONE
/// `Authentication-Results` header on every message it delivers. That is
/// ALL it proves. It does NOT prove the host emits a `dmarc=` clause on
/// every message, for every `From:` domain a sender might choose — DMARC
/// evaluation can legitimately produce a header with no `dmarc=` section
/// at all (a domain outside what the receiving server evaluates, an
/// alignment edge case, or simply a provider whose stamping does not cover
/// every method for every sender). A determined sender picks the `From:`
/// domain specifically so the GENUINE stamp the known-stamping host adds
/// carries no `dmarc=` clause of its own for it, then supplies the ONLY
/// `dmarc=` text the header ends up containing themselves — via the SAME
/// echo mechanism [`super::auth_results::dmarc_verdict`]'s own doc
/// describes (an envelope local part echoed verbatim into another
/// section's comment or property value). One clause, one section, the
/// counts this fn and [`super::auth_results`] both check agree — because
/// by the time it reaches either of them, it genuinely IS one well-formed
/// `dmarc=` section. There is no forged second header, no truncation, no
/// grammar violation to detect: `host_is_known_to_stamp` answered a
/// different, narrower question than "did THIS message get a genuine
/// DMARC evaluation," and content inspection has no way to ask the real
/// one. Two candidate fixes were measured against this and both failed —
/// see the fix-forward history for what was tried.
///
/// What actually mitigates this, in order: (1) [`crate::email_watch::
/// EmailWatchStore::auto_write_enabled`] defaults OFF — the gate is opt-in,
/// so nobody is exposed to it without deliberately turning it on; (2) every
/// write this whole pipeline can ever produce lands UNCONFIRMED (see
/// [`crate::applications::StatusEvent::confirmed`]) and requires the user's
/// own adjudication before it's trusted — that backstop does not depend on
/// this fn, `host_is_known_to_stamp`, or anything upstream of it being
/// correct. Closing this PROPERLY needs verification that does not trust
/// the header at all — independent DKIM/SPF/DMARC re-verification against
/// DNS, performed by this crate itself — which was investigated (a
/// `mail-auth`-based design) and explicitly NOT built: it is a new
/// dependency and a new network-egress class this feature's design
/// deliberately avoids, a decision for the product owner, not something to
/// default into by writing a fourth parser round.
///
/// `false` (fail closed) if `from_domain` is `None`, if `auth_results` is
/// empty, or if the topmost entry does not parse to a `pass` aligned with
/// `from_domain` — but a `true` here is a best-effort signal, not a proof,
/// and the caller's OWN combination with `host_is_known_to_stamp` narrows
/// the exposed population without eliminating it.
fn dmarc_pass_aligned(auth_results: &[String], from_domain: Option<&str>) -> bool {
    let Some(from_domain) = from_domain else {
        return false;
    };
    let Some(topmost) = auth_results.first() else {
        return false;
    };
    super::auth_results::dmarc_verdict(topmost).is_some_and(|(result, header_from)| {
        result.eq_ignore_ascii_case("pass") && header_from.eq_ignore_ascii_case(from_domain)
    })
}

/// IMAP hosts independently known to stamp an `Authentication-Results`
/// header on EVERY message they deliver, regardless of the evaluated
/// result. Closes the residual [`dmarc_pass_aligned`]'s own doc names: a
/// message where the array has EXACTLY ONE entry and it happens to be
/// attacker-supplied. That residual exists only because a host that does
/// not stamp anything cannot be told apart, by CONTENT alone, from one
/// that stamped a single genuine header. For a host on this list, that
/// ambiguity cannot arise: per the receiving-MTA-prepends guarantee
/// [`dmarc_pass_aligned`]'s own doc relies on, EVERY message delivered
/// through one of these hosts carries at least the host's own genuine
/// stamp — either alongside a lower forged one (topmost-only already
/// ignores it) or alone (because the host's own anti-spoofing already
/// stripped an inbound forgery before this code ever sees the message).
/// Either way, a lone entry for a host on this list is never a raw,
/// unmodified attacker forgery.
///
/// **This does NOT rely on trusting the header's own claimed
/// `authserv-id`** — an attacker can write any string they like there, so
/// comparing text against text proves nothing (an authserv-id check was
/// considered and rejected for exactly this reason — see
/// [`dmarc_pass_aligned`]'s doc). This list only needs "does this host
/// stamp something, unconditionally" to be true, which is a substantially
/// weaker and more broadly verifiable claim across major providers than
/// "does this host's own stamp survive a sophisticated forger."
///
/// **`127.0.0.1`/`localhost` (ProtonMail Bridge, and any other local-bridge
/// IMAP proxy) is deliberately NOT here**, even though Proton Mail itself
/// is known to stamp DMARC results: the bridge's loopback address carries
/// no signal about which real provider sits behind it, so this list cannot
/// vouch for it. A Proton-via-Bridge account gets the topmost-only
/// protection (real, and closes the two forged-second-header cases) but
/// not this additional narrowing — a documented, accepted residual, not an
/// oversight.
const HOSTS_KNOWN_TO_STAMP: &[&str] = &[
    "imap.gmail.com",
    "outlook.office365.com",
    "imap-mail.outlook.com",
    "imap.mail.yahoo.com",
    "imap.fastmail.com",
];

/// Whether `host` — the account's CONFIGURED IMAP host, locally-stored data
/// no attacker can influence (unlike anything inside the message itself) —
/// is on [`HOSTS_KNOWN_TO_STAMP`]. Case-insensitive exact match only (these
/// are fixed, well-known hostnames, not a domain family to wildcard).
pub(crate) fn host_is_known_to_stamp(host: &str) -> bool {
    HOSTS_KNOWN_TO_STAMP
        .iter()
        .any(|known| host.eq_ignore_ascii_case(known))
}

/// Parse a raw FULL message (`BODY.PEEK[]`, as returned by
/// `imap_client::fetch_bodies`) and return its plain-text body (mail-parser
/// converts an HTML-only body to text automatically when no text/plain part
/// exists). `None` if unparseable or the message truly has no body part.
pub fn parse_body_text(raw: &[u8]) -> Option<String> {
    let message = MessageParser::default().parse(raw)?;
    message.body_text(0).map(|cow| cow.into_owned())
}

// ── Fingerprint (the subject-regex gate; SCORE_HINTS is a boost, never a gate) ──

/// Subject substrings/phrases (EN + DE) that mark a message as a plausible
/// application-confirmation email. Case-insensitive, Unicode-aware (so
/// `(?i)für` matches `FÜR`/`Für`). This is the ONLY gate — a hit here is
/// required before any body is fetched or any matching is attempted.
///
/// Recall was broadened per `job-match-expert` review (item 8/9): the
/// informal "thanks for applying"/"thank you for your application"
/// contraction, "we('ve| have) received your application", bare "application
/// confirmation"/"received", reverse-order "received your application", the
/// DE dative "Ihrer Bewerbung", "Eingangsbestätigung", and informal "deine
/// Bewerbung". This intentionally trades some precision for recall — see the
/// `known_false_positive_*` tests below for the accepted risk under a
/// notify-only (never auto-write) model.
static SUBJECT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)thank you for applying",
        r"(?i)thank(?:s| you)(?: you)? for (?:applying|your application)",
        r"(?i)application (?:was |has been )?(?:received|submitted)",
        r"(?i)application (?:confirmation|received)",
        r"(?i)we(?:'ve| have)? received your application",
        r"(?i)received your application",
        r"(?i)your application to",
        r"(?i)ihr(?:e|er) bewerbung",
        r"(?i)bewerbung (?:ist )?(?:eingegangen|erhalten)",
        r"(?i)danke für ihre bewerbung",
        r"(?i)eingangsbestätigung",
        r"(?i)deine bewerbung",
    ]
    .iter()
    .map(|p| Regex::new(p).expect("static subject pattern is valid"))
    .collect()
});

/// Sender domains known to be ATS/job-board confirmation senders — used
/// ONLY to nudge the MATCH score (see [`Fingerprint::domain_hint`]'s doc).
/// Only `greenhouse.io`/`greenhouse-mail.io` are independently verified
/// (real Greenhouse confirmation emails); the rest are commonly-cited
/// folklore for other ATS/board vendors — kept anyway since a hint here only
/// ever BOOSTS score, truly never gates anything (see [`WRITE_GATE_DOMAINS`]
/// below for the SEPARATE, narrower list that gates the write), so an
/// unverified/wrong entry here can't create a false positive on its own.
const SCORE_HINTS: &[&str] = &[
    "greenhouse.io",
    "greenhouse-mail.io",
    "lever.co",
    "myworkday.com",
    "linkedin.com",
    "indeed.com",
];

/// Sender domains eligible to AUTHORIZE an auto-write (alongside a required
/// DMARC `pass` — see [`EmailHeader::dmarc_pass`]; neither alone is
/// sufficient, see [`crate::email_watch::auto_write::apply_matched_intent`]'s
/// doc). Deliberately NARROWER than [`SCORE_HINTS`]: `linkedin.com` and
/// `indeed.com` are dropped — both are messaging/relay platforms that
/// routinely carry ATTACKER-AUTHORED subject/body text from their own
/// genuinely DMARC-valid infrastructure (anyone who can message the victim
/// through either platform satisfies a DMARC check on ITS domain, which
/// proves nothing about the CONTENT). A domain that forwards third-party
/// text is not evidence of anything, so it stays a score boost only, never
/// a write authority. `greenhouse.io`/`greenhouse-mail.io`/`lever.co`/
/// `myworkday.com` remain: unlike an open messaging relay, sending through
/// them requires a registered employer/recruiter tenant — a meaningfully
/// higher bar, though ALSO not independently verified against real
/// multi-tenant-signup abuse (documented, not assumed safe — the DMARC
/// requirement narrows this further but does not fully close a fake-tenant
/// scenario).
const WRITE_GATE_DOMAINS: &[&str] = &[
    "greenhouse.io",
    "greenhouse-mail.io",
    "lever.co",
    "myworkday.com",
];

fn domain_matches_any(domain: &str, hints: &[&str]) -> bool {
    hints
        .iter()
        .any(|hint| domain == *hint || domain.ends_with(&format!(".{hint}")))
}

/// The result of fingerprinting one [`EmailHeader`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Fingerprint {
    subject_matched: bool,
    /// Sender domain is a known ATS hint from [`SCORE_HINTS`] — purely a
    /// MATCH-score signal. See [`crate::email_watch::matcher::best_match`],
    /// which adds a small, capped nudge to the company score when this is
    /// `true` and NEVER lets it substitute for a real company-token
    /// overlap. **Never a write authority** — see [`Self::write_gate_domain`]
    /// for that, an intentionally separate/narrower signal so the two
    /// roles cannot drift back together.
    pub domain_hint: bool,
    /// Sender domain is on the narrower [`WRITE_GATE_DOMAINS`] list — ONE of
    /// the two conditions [`crate::email_watch::auto_write::
    /// apply_matched_intent`] requires before writing (the other is
    /// [`EmailHeader::dmarc_pass`]; both are required, neither alone is
    /// sufficient).
    pub write_gate_domain: bool,
}

impl Fingerprint {
    /// Whether this message clears the fingerprint gate at all — the ONLY
    /// signal that decides whether a body fetch + match attempt happens.
    /// Neither `domain_hint` nor `write_gate_domain` contributes to this — a
    /// hint alone is not enough.
    pub fn is_candidate(&self) -> bool {
        self.subject_matched
    }
}

pub fn fingerprint(header: &EmailHeader) -> Fingerprint {
    let subject_matched = SUBJECT_PATTERNS
        .iter()
        .any(|re| re.is_match(&header.subject));
    let (domain_hint, write_gate_domain) =
        header.from_domain.as_deref().map_or((false, false), |d| {
            (
                domain_matches_any(d, SCORE_HINTS),
                domain_matches_any(d, WRITE_GATE_DOMAINS),
            )
        });
    Fingerprint {
        subject_matched,
        domain_hint,
        write_gate_domain,
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
    // NOTE: `and`/`und` deliberately excluded from these boundaries (unlike
    // the other continuation words) — a company name legitimately containing
    // "and"/"und" (e.g. "Johnson and Johnson", "Miller und Frost") would
    // otherwise be truncated at the first one. Both words are already in
    // `matcher::STOPWORDS`, so leaving them IN the captured span (when the
    // capture does run past them) is harmless — the matcher strips them
    // before scoring either way.
    const EN_BOUNDARY: &str = r"(?:$|[.,!?;:]|\s+(?:was|is|has|will|being|which)\b)";
    const DE_BOUNDARY: &str = r"(?:$|[.,!?;:]|\s+(?:ist|war|wurde|wird)\b)";
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
/// `applications::clamp_job_description`'s truncation approach. `pub(super)`
/// so [`crate::email_watch::intent`] can reuse the SAME byte-safe truncation
/// helper for its own subject bound (`SUBJECT_MAX_BYTES` above) — that
/// module has its own, deliberately much larger, body-scan cap instead of
/// [`BODY_SNIPPET_BYTES`] below: that constant sizes a cheap first-pass
/// fingerprint/extraction snippet, a different job with a different cost of
/// being wrong than intent classification. See `intent`'s module doc.
pub(super) fn safe_prefix(s: &str, max_bytes: usize) -> &str {
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
/// fallback, not a full-body scan). Private to this module — `intent`
/// intentionally does NOT reuse this constant (see [`safe_prefix`]'s doc).
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
            dmarc_pass: false,
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

    // ── fingerprint: broadened recall (job-match-expert items 8/9) ──────────

    #[test]
    fn fingerprint_matches_the_broadened_en_recall_phrases() {
        for subject in [
            "Thanks for applying to Acme!",   // informal contraction
            "Thank you for your application", // "application" not "applying"
            "We've received your application",
            "We have received your application",
            "Application confirmation",
            "Confirmation: received your application", // reverse order, no "we"
        ] {
            assert!(
                fingerprint(&header(subject, None)).is_candidate(),
                "expected a match for {subject:?}"
            );
        }
    }

    #[test]
    fn fingerprint_matches_the_broadened_de_recall_phrases() {
        for subject in [
            "Ihrer Bewerbung liegt uns vor", // dative "Ihrer Bewerbung"
            "Eingangsbestätigung Ihrer Bewerbung",
            "Deine Bewerbung ist eingegangen", // informal "du" form
        ] {
            assert!(
                fingerprint(&header(subject, None)).is_candidate(),
                "expected a match for {subject:?}"
            );
        }
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

    // ── formerly-accepted false-positive shapes (job-match-expert item 11) ──
    //
    // The fingerprint gate is a SUBJECT phrase match — on its own it has no
    // way to tell a genuine confirmation from a rejection, an interview
    // invite, or a draft-completion nudge that happens to reuse the same
    // wording. That was an ACCEPTED risk under v1's notify+confirm model
    // (never auto-write). The v2-slice-1 body negative-signal check now
    // exists ([`crate::email_watch::intent::classify_intent`]) — these three
    // tests now assert the CORRECT intent instead of just documenting the
    // risk. It is still not wired into the poller (no auto-write yet; that's
    // a later slice), but the pure classification itself is proven here.

    #[test]
    fn known_false_positive_a_rejection_email_still_fingerprints() {
        // A real ATS rejection often reuses the exact confirmation subject
        // line from earlier in the thread — and a real rejection reply
        // commonly still carries the original confirmation's body wording
        // too (quoted thread history, or a template that opens with a
        // receipt line before the bad news). `fingerprint` (subject-only)
        // still can't tell them apart, but `intent::classify_intent` reads
        // the body and correctly picks Rejection even with a confirmation
        // phrase also present in the same message.
        let subject = "Your application to Acme Corp";
        assert!(fingerprint(&header(subject, None)).is_candidate());
        let body = "If you are among qualified candidates for other roles we will be in touch. \
                     Unfortunately, after careful review we have decided not be moving forward \
                     with your application at this time.";
        assert_eq!(
            crate::email_watch::intent::classify_intent(subject, Some(body)),
            Some(crate::email_watch::intent::EmailIntent::Rejection)
        );
    }

    #[test]
    fn known_false_positive_b_an_interview_invite_still_fingerprints() {
        let subject = "Your application to Acme — Next Steps";
        assert!(fingerprint(&header(subject, None)).is_candidate());
        let body = "We would like to invite you for a job interview next week.";
        assert_eq!(
            crate::email_watch::intent::classify_intent(subject, Some(body)),
            Some(crate::email_watch::intent::EmailIntent::Interview)
        );
    }

    #[test]
    fn known_false_positive_d_a_draft_completion_nudge_still_fingerprints() {
        let subject = "Complete your application to Acme";
        assert!(fingerprint(&header(subject, None)).is_candidate());
        // None of the 4 intents apply to a draft-completion nudge — this is
        // the actual improvement: the classifier correctly stays silent (no
        // intent, so no future write) instead of treating the fingerprint
        // match as any kind of confirmation.
        let body = "You started an application to Acme Corp but haven't submitted it yet. \
                     Click here to finish and submit your application.";
        assert_eq!(
            crate::email_watch::intent::classify_intent(subject, Some(body)),
            None
        );
    }

    // ── and/und no longer truncates a company name (item 10 regression) ─────

    #[test]
    fn extraction_no_longer_truncates_a_company_name_containing_and() {
        let c = extract_candidates(
            "Your application to Johnson and Johnson was received",
            None,
            None,
        );
        assert_eq!(c.company.as_deref(), Some("Johnson and Johnson"));
    }

    #[test]
    fn extraction_no_longer_truncates_a_company_name_containing_und() {
        let c = extract_candidates(
            "Ihre Bewerbung bei Miller und Frost ist eingegangen",
            None,
            None,
        );
        assert_eq!(c.company.as_deref(), Some("Miller und Frost"));
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

    #[test]
    fn write_gate_domain_excludes_the_open_relays_linkedin_and_indeed() {
        // HIGH-2 fix: linkedin.com/indeed.com stay SCORE-only (domain_hint
        // true, matches boost the score) but must NEVER authorize a write —
        // both routinely relay attacker-authored subject/body from their own
        // genuinely DMARC-valid infrastructure, so their domain being
        // authentic proves nothing about the CONTENT.
        let linkedin = fingerprint(&header("x", Some("linkedin.com")));
        assert!(linkedin.domain_hint, "linkedin.com still boosts the score");
        assert!(
            !linkedin.write_gate_domain,
            "linkedin.com must never authorize a write"
        );

        let indeed = fingerprint(&header("x", Some("indeed.com")));
        assert!(indeed.domain_hint);
        assert!(!indeed.write_gate_domain);
    }

    #[test]
    fn write_gate_domain_true_for_the_narrower_ats_list() {
        assert!(fingerprint(&header("x", Some("greenhouse.io"))).write_gate_domain);
        assert!(fingerprint(&header("x", Some("mail.greenhouse-mail.io"))).write_gate_domain);
        assert!(fingerprint(&header("x", Some("lever.co"))).write_gate_domain);
        assert!(fingerprint(&header("x", Some("myworkday.com"))).write_gate_domain);
        assert!(!fingerprint(&header("x", Some("example.com"))).write_gate_domain);
    }

    #[test]
    fn host_is_known_to_stamp_true_for_the_known_providers_case_insensitively() {
        assert!(host_is_known_to_stamp("imap.gmail.com"));
        assert!(host_is_known_to_stamp("IMAP.GMAIL.COM"));
        assert!(host_is_known_to_stamp("outlook.office365.com"));
        assert!(host_is_known_to_stamp("imap-mail.outlook.com"));
        assert!(host_is_known_to_stamp("imap.mail.yahoo.com"));
        assert!(host_is_known_to_stamp("imap.fastmail.com"));
    }

    #[test]
    fn host_is_known_to_stamp_false_for_an_unknown_or_bridge_host() {
        assert!(!host_is_known_to_stamp("mail.example.com"));
        // ProtonMail Bridge's loopback address deliberately does NOT vouch
        // for Proton -- see the const's own doc.
        assert!(!host_is_known_to_stamp("127.0.0.1"));
        assert!(!host_is_known_to_stamp("localhost"));
        // Not a suffix/subdomain match -- an attacker-controlled or
        // coincidentally-similar hostname must not slip through.
        assert!(!host_is_known_to_stamp(
            "notimap.gmail.com.attacker.example"
        ));
    }

    // ── DMARC authentication (HIGH-2 fix) ────────────────────────────────────

    #[test]
    fn dmarc_pass_aligned_true_for_a_realistic_pass_header() {
        let ar = "mx.google.com; \
                   dkim=pass header.i=@greenhouse.io header.s=selector header.b=abc123; \
                   spf=pass smtp.mailfrom=bounce@greenhouse.io; \
                   dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=greenhouse.io"
            .to_string();
        assert!(dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
    }

    #[test]
    fn dmarc_pass_aligned_false_when_result_is_not_pass() {
        let ar = "mx.google.com; dmarc=fail (p=REJECT) header.from=greenhouse.io".to_string();
        assert!(!dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
    }

    #[test]
    fn dmarc_pass_aligned_false_when_header_from_does_not_match_the_visible_from_domain() {
        // A `pass` for a DIFFERENT domain than the visible `From:` does not
        // authenticate THIS sender — e.g. an attacker who controls DKIM for
        // some other domain but is spoofing the visible From: address of
        // a write-gate-eligible one.
        let ar = "mx.google.com; dmarc=pass (p=REJECT) header.from=attacker.example".to_string();
        assert!(!dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
    }

    #[test]
    fn dmarc_pass_aligned_fails_closed_on_a_missing_or_unparseable_header() {
        assert!(!dmarc_pass_aligned(&[], Some("greenhouse.io")));
        assert!(!dmarc_pass_aligned(
            &["not a valid authentication-results header at all".to_string()],
            Some("greenhouse.io")
        ));
        assert!(!dmarc_pass_aligned(
            &["mx.google.com; dmarc=pass (p=REJECT) header.from=greenhouse.io".to_string()],
            None
        ));
    }

    // -- LIVE, ACCEPTED RESIDUAL (not closed -- do not "fix" this test by --
    // -- flipping its assertion; see parser.rs's own doc on dmarc_pass_aligned
    // -- for the full reasoning, and auto_write.rs's doc for the mitigations)

    #[test]
    fn known_residual_a_genuine_stamp_with_no_dmarc_clause_of_its_own_can_still_be_forged() {
        // The gap `host_is_known_to_stamp` does NOT close, found by the
        // re-gate after the envelope-injection/truncation fixes: a GENUINE
        // stamp from a known-stamping host that simply carries no `dmarc=`
        // clause AT ALL for the message (a real, unremarkable outcome --
        // DMARC evaluation can legitimately produce a header with no dmarc
        // section for a given sender). The attacker picks their
        // `From:`/envelope domain specifically so the real stamp has this
        // shape, then supplies the header's ONLY `dmarc=` text themselves.
        //
        // What this test does NOT claim: a specific real-world delivery
        // mechanism. The comment/quoted-string escape this branch's OWN
        // truncation fix already closes (an unescaped `)` inside a quoted
        // local part) does NOT reach this outcome -- it was tried while
        // building this reproduction, and it leaves an orphaned quote
        // character that the truncation fix's "only genuine end-of-input
        // may stop the parse" rule correctly turns into `None`. What DOES
        // reach it, verified here, is an unquoted, unescaped `;` inside a
        // property value that RFC 8601 permits to be an addr-spec
        // (`smtp.mailfrom=`/`smtp.rcptto=`) rather than a strictly-quoted
        // token -- e.g. a verifying server that echoes an envelope
        // local-part into that property without re-quoting a character
        // its OWN grammar treats as structural. Whether any real,
        // deployed provider's stamping code has that specific gap is
        // exactly as unverifiable as the DKIM `i=`-decode question
        // recorded elsewhere in this file: this crate never constructs or
        // decodes that content itself, only reads whatever text a real
        // server already wrote. The STRUCTURAL point survives regardless
        // of which concrete mechanism a real attacker would reach for: an
        // unquoted top-level `;` with no genuine dmarc section to disagree
        // with cannot be told apart from real grammar, because it IS real
        // grammar by the time `dmarc_verdict` reads it -- there is nothing
        // malformed for a parser to reject.
        //
        // This assertion is `true` -- the vulnerable, unfortunate, but
        // CORRECT-for-what-this-function-can-know outcome -- on purpose,
        // matching how the (since-fixed) envelope-injection defect was
        // originally recorded: prove the exploit is real rather than
        // assert it away. Unlike that one, THIS residual is not expected
        // to close via a parser change (two candidate fixes were measured
        // and both failed) -- what changed instead is the write path's OWN
        // default: `EmailWatchStore::auto_write_enabled` now defaults OFF,
        // and every write this whole pipeline can ever produce still lands
        // UNCONFIRMED, requiring the user's own adjudication. Those are
        // the real mitigations; this function's `true` here is not one of
        // them.
        let header = "mx.google.com; \
                       dkim=pass header.i=attacker;dmarc=pass header.from=victim.io"
            .to_string();
        assert!(
            dmarc_pass_aligned(&[header], Some("victim.io")),
            "documents the live residual — see this test's own doc before treating a change \
             here as a fix"
        );
    }

    // -- HIGH fix: only the TOPMOST Authentication-Results is trustworthy ----
    //
    // RFC 8601 SS5: a consumer must only trust the header field ADDED BY ITS
    // OWN receiving MTA and must ignore any that arrived already present in
    // the message -- the final MTA PREPENDS its own stamp, so in document
    // order the FIRST occurrence is the one the user's own provider just
    // added; everything below it is either an earlier hop or attacker text
    // and must never be consulted. `.any()` over every occurrence let an
    // attacker satisfy the gate just by including their own forged header
    // naming a write-gate domain -- these are the permanent regression
    // tests for that exploit; this class returns the moment someone
    // "simplifies" the header scan back to a search.
    //
    // NOT tested here, and NOT closeable by this function (see its own doc
    // for the full reasoning): a message where the ONLY
    // `Authentication-Results` header present is a forged one that already
    // matches the exact string a genuine stamp would contain (e.g. an
    // attacker who correctly writes `mx.google.com` -- the coordinator's
    // own worked example). No text-only parse of a single header can tell
    // that apart from a real one; only cryptographic re-verification (DKIM/
    // SPF, performed independently by this code against DNS) closes it, and
    // that is a substantial new feature, not built here.

    #[test]
    fn dmarc_pass_aligned_uses_only_the_topmost_stamp_ignoring_a_forged_one_below() {
        // A genuine topmost pass, aligned to the write-gate domain, PLUS a
        // forged second header underneath claiming a pass for a completely
        // different domain. Must authorize based on the topmost ONLY -- the
        // forged second header must never be consulted, let alone able to
        // redirect which domain gets authorized.
        let genuine_topmost =
            "mx.google.com; dmarc=pass (p=REJECT) header.from=greenhouse.io".to_string();
        let forged_below = "attacker.example; dmarc=pass header.from=attacker.example".to_string();
        assert!(dmarc_pass_aligned(
            &[genuine_topmost.clone(), forged_below.clone()],
            Some("greenhouse.io")
        ));
        // And the forged domain must NOT be authorized either, even though
        // it is present in the list -- confirms the lower header is truly
        // ignored, not merely "the wrong one happened to lose."
        assert!(!dmarc_pass_aligned(
            &[genuine_topmost, forged_below],
            Some("attacker.example")
        ));
    }

    #[test]
    fn dmarc_pass_aligned_a_genuine_topmost_fail_is_not_overridden_by_a_forged_pass_below() {
        // THE CASE `.any()` GETS EXACTLY BACKWARDS: the AUTHORITATIVE
        // (topmost, genuine) result says `dmarc=fail`. A forged header
        // below it claims `pass` for the same domain. Must NOT authorize --
        // `.any()` would find the forged `pass` and return true regardless
        // of what the real evaluation said.
        let genuine_fail =
            "mx.google.com; dmarc=fail (p=REJECT) header.from=greenhouse.io".to_string();
        let forged_pass = "attacker.example; dmarc=pass header.from=greenhouse.io".to_string();
        assert!(
            !dmarc_pass_aligned(&[genuine_fail, forged_pass], Some("greenhouse.io")),
            "a genuine topmost fail must never be overridden by a forged pass beneath it"
        );
    }

    #[test]
    fn dmarc_pass_aligned_fails_closed_when_the_topmost_header_is_unreadable() {
        // The topmost header parses to `None` (malformed/unexpected shape)
        // -- must fail closed, never fall through to a LOWER header (which
        // would reintroduce the same trust-order violation as `.any()`).
        let unreadable = "not a valid authentication-results header at all".to_string();
        let genuine_below_it =
            "mx.google.com; dmarc=pass (p=REJECT) header.from=greenhouse.io".to_string();
        assert!(!dmarc_pass_aligned(
            &[unreadable, genuine_below_it],
            Some("greenhouse.io")
        ));
    }

    // -- CRITICAL fix: this used to be a byte-index desync between a --------
    // -- lowercased offset and an original-case slice; the new tokeniser ---
    // -- (`super::auth_results`) has no offset arithmetic to desync at all, -
    // -- but re-run the exact reproductions here too, end-to-end through ---
    // -- the actual public entry point every caller uses. -------------------

    #[test]
    fn dmarc_pass_aligned_does_not_panic_on_hostile_unicode_and_fails_closed() {
        // 'İ' (U+0130) is THE ORIGINAL reported reproduction: it
        // lowercases to a byte-length-changing sequence ("i̇", 2 bytes ->
        // 3), which is exactly what desynced the OLD substring scanner's
        // offsets (`panic = "abort"` in release turned that into the whole
        // desktop app crashing, permanently, since the crashing message
        // gets re-fetched every launch). ß/K (Kelvin sign)/Ꭰ (Cherokee)
        // are the other byte-length/codepoint-count-changing cases already
        // pinned. None should panic; none should authorize.
        for hostile in [
            "mx.example.com; İ dmarc=épass header.from=greenhouse.io",
            "mx.example.com; ß dmarc=pass header.from=greenhouse.io",
            "mx.example.com; K dmarc=pass header.from=greenhouse.io",
            "mx.example.com; Ꭰ dmarc=pass header.from=greenhouse.io",
        ] {
            assert!(!dmarc_pass_aligned(
                &[hostile.to_string()],
                Some("greenhouse.io")
            ));
        }
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
    fn parse_header_wires_a_real_authentication_results_header_through_to_dmarc_pass() {
        // End-to-end: the raw fetched bytes -> mail-parser -> dmarc_pass_aligned
        // path, not just the pure helper tested above in isolation.
        // Deliberately unfolded (one physical line for the whole header
        // value) — a `\`-continued Rust byte-string literal strips leading
        // whitespace from the next line, so a folded/indented continuation
        // written the "natural" way here would silently NOT reproduce RFC
        // 5322 folding and would parse as a malformed header instead.
        let raw = b"From: Careers <careers@greenhouse.io>\r\n\
Subject: Thank you for applying!\r\n\
Authentication-Results: mx.google.com; dkim=pass header.i=@greenhouse.io header.s=selector header.b=abc; dmarc=pass (p=REJECT) header.from=greenhouse.io\r\n\
\r\n";
        let header = parse_header(raw).expect("should parse a minimal header block");
        assert_eq!(header.from_domain.as_deref(), Some("greenhouse.io"));
        assert!(
            header.dmarc_pass,
            "a real, aligned dmarc=pass header must wire through"
        );
    }

    #[test]
    fn parse_header_end_to_end_proves_mail_parser_preserves_document_order_for_repeated_headers() {
        // The whole topmost-only fix rests on `mail_parser` collecting
        // repeated headers in DOCUMENT order (never reversed, never
        // deduped) -- verified here against a REAL raw message through the
        // REAL parser, not just by reading the crate's source (which was
        // also checked: `MessageStream::parse_headers` in
        // mail-parser-0.11.6/src/parsers/header.rs scans the byte stream
        // forward and `Vec::push`es each header as it's encountered, and
        // `Message::header_as` in src/core/message.rs iterates that Vec in
        // order and collects matches -- but a live assertion is the
        // authoritative check, not the source read). TWO
        // `Authentication-Results` headers: a genuine topmost pass for
        // greenhouse.io, a forged second one below claiming a pass for
        // attacker.example. If mail-parser ever reversed or reordered
        // repeated headers, this test would start authorizing the WRONG
        // domain and fail loudly, rather than the vulnerability silently
        // reappearing. Built via `.join("\r\n")` rather than a
        // `\`-continued byte-string literal -- that continuation style
        // already bit one earlier test in this file by silently eating
        // the leading whitespace of a folded line.
        let raw = [
            "From: Careers <careers@greenhouse.io>",
            "Subject: Thank you for applying!",
            "Authentication-Results: mx.google.com; dmarc=pass (p=REJECT) header.from=greenhouse.io",
            "Authentication-Results: attacker.example; dmarc=pass header.from=attacker.example",
            "",
            "",
        ]
        .join("\r\n");
        let header = parse_header(raw.as_bytes()).expect("should parse a minimal header block");
        assert!(
            header.dmarc_pass,
            "the genuine topmost header (greenhouse.io, matching the visible From:) must authorize"
        );
    }

    #[test]
    fn parse_header_dmarc_pass_is_false_with_no_authentication_results_header_at_all() {
        // The absent-header half of "fail closed" — a host that never
        // stamps this header (a plain forward, a non-Gmail-shaped IMAP
        // provider) must never accidentally read as authenticated.
        let raw = b"From: Careers <careers@greenhouse.io>\r\n\
Subject: Thank you for applying!\r\n\
\r\n";
        let header = parse_header(raw).expect("should parse a minimal header block");
        assert!(!header.dmarc_pass);
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

    // -- Recall corpus: does the scanner read REAL providers' shapes correctly? --
    //
    // Every fixture on this branch before this corpus was Gmail-shaped, so
    // the false-negative rate on other major providers was never measured.
    // PROVENANCE, stated plainly rather than silently assumed: this
    // environment has no live network/web access, so these are NOT
    // captured from a real inbox. They are reconstructed from each
    // provider's OWN publicly documented `Authentication-Results` header
    // format (Microsoft's own "Anti-spam message headers in Microsoft 365"
    // support article for the M365 shape; the general RFC 8601 `resinfo`
    // conventions each of Yahoo/Fastmail/Proton is documented to follow
    // elsewhere) plus this crate author's general training knowledge of
    // real header samples -- NOT verified against a current live message.
    // Treat this as a reasonable-effort starting corpus, not ground truth;
    // exact authserv-id strings, comment wording, and folding details can
    // drift from a real captured header, and only real captured headers
    // from each provider (from someone with live inbox access) can close
    // that gap. Pin BOTH directions per provider: a genuine pass for a
    // write-gate domain must authorise; a genuine fail (or hostile input)
    // must not -- a gate that silently refuses every legitimate email is
    // also a broken feature.

    #[test]
    fn recall_outlook_m365_genuine_pass_authorises() {
        // Microsoft's documented shape has NO leading `<authserv-id>;` --
        // it starts directly with `spf=`, and carries Microsoft-specific
        // extension properties (`action=`, `compauth=`) RFC 8601 does not
        // define. Confirms clause-selection does not require an
        // authserv-id prefix to work.
        let ar = "spf=pass (sender IP is 40.107.1.1) smtp.mailfrom=greenhouse.io; \
                   dkim=pass (signature was verified) header.d=greenhouse.io header.s=selector1; \
                   dmarc=pass action=none header.from=greenhouse.io; \
                   compauth=pass reason=100"
            .to_string();
        assert!(
            dmarc_pass_aligned(&[ar], Some("greenhouse.io")),
            "a genuine M365 pass must authorise -- Microsoft's authserv-id-less shape must not \
             be silently refused"
        );
    }

    #[test]
    fn recall_outlook_m365_genuine_fail_does_not_authorise() {
        let ar = "spf=softfail (sender IP is 203.0.113.9) smtp.mailfrom=attacker.example; \
                   dkim=none header.d=none; \
                   dmarc=fail action=none header.from=greenhouse.io; \
                   compauth=fail reason=001"
            .to_string();
        assert!(!dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
    }

    #[test]
    fn recall_yahoo_genuine_pass_authorises() {
        // Yahoo is documented to sometimes omit the space before the
        // parenthesized comment (`dmarc=pass(p=REJECT)` not
        // `dmarc=pass (p=REJECT)`).
        let ar = "mtaX.mail.gq1.yahoo.com; \
                   dkim=pass (ok) header.i=@greenhouse.io header.s=s2048 header.b=abcdefgh; \
                   spf=pass smtp.mailfrom=bounce@greenhouse.io; \
                   dmarc=pass(p=REJECT) header.from=greenhouse.io"
            .to_string();
        assert!(dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
    }

    #[test]
    fn recall_yahoo_genuine_fail_does_not_authorise() {
        let ar = "mtaX.mail.gq1.yahoo.com; \
                   dkim=fail header.i=@attacker.example header.s=s2048 header.b=abcdefgh; \
                   spf=fail smtp.mailfrom=bounce@attacker.example; \
                   dmarc=fail(p=REJECT) header.from=greenhouse.io"
            .to_string();
        assert!(!dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
    }

    #[test]
    fn recall_fastmail_genuine_pass_authorises() {
        let ar = "mx-fm-int.internal; \
                   dkim=pass (2048-bit rsa key sha256) header.d=greenhouse.io header.i=@greenhouse.io header.b=abcdef; \
                   dmarc=pass (p=NONE sp=NONE dis=NONE) header.from=greenhouse.io; \
                   spf=pass smtp.mailfrom=bounce@greenhouse.io"
            .to_string();
        assert!(dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
    }

    #[test]
    fn recall_fastmail_genuine_fail_does_not_authorise() {
        let ar = "mx-fm-int.internal; \
                   dkim=none; \
                   dmarc=fail (p=NONE sp=NONE dis=NONE) header.from=greenhouse.io; \
                   spf=none smtp.mailfrom=bounce@attacker.example"
            .to_string();
        assert!(!dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
    }

    #[test]
    fn recall_protonmail_genuine_pass_authorises() {
        // Reached only via ProtonMail Bridge (a LOCAL IMAP proxy on
        // 127.0.0.1) -- this test is about the CONTENT scanner only; see
        // `host_is_known_to_stamp`'s own doc for why the account-level
        // gate deliberately does NOT extend `host_is_known_to_stamp`
        // coverage to Proton (the bridge's loopback address carries no
        // signal about the real provider behind it).
        let ar = "mail.protonmail.ch; \
                   dkim=pass (2048-bit key) header.d=greenhouse.io header.b=abcdef; \
                   dmarc=pass (p=reject sp=reject) header.from=greenhouse.io; \
                   spf=pass smtp.mailfrom=bounce@greenhouse.io"
            .to_string();
        assert!(dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
    }

    #[test]
    fn recall_protonmail_genuine_fail_does_not_authorise() {
        let ar = "mail.protonmail.ch; \
                   dkim=fail header.d=attacker.example header.b=abcdef; \
                   dmarc=fail (p=reject sp=reject) header.from=greenhouse.io; \
                   spf=fail smtp.mailfrom=bounce@attacker.example"
            .to_string();
        assert!(!dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
    }

    // -- FIXED: the envelope-injection exploit that survived two prior ------
    // -- rounds of substring-scanning. This test used to assert the -------
    // -- VULNERABLE outcome (`true`), deliberately, so CI could never claim -
    // -- it was resolved. It is flipped here because the underlying scanner
    // -- was actually replaced (`super::auth_results`, an RFC 8601 --------
    // -- tokeniser that tracks comment/quoted-string state) -- not because -
    // -- the assertion was edited in isolation. --------------------------

    #[test]
    fn envelope_injected_text_no_longer_overrides_a_genuine_fail_verdict() {
        // The exploit: a SINGLE, GENUINE, correctly-folded header -- no
        // forged second header, no non-stamping host. RFC 5321 permits a
        // QUOTED local-part in an envelope MAIL FROM address (`"any text
        // including = and spaces"@domain`); Gmail's SPF evaluation is
        // genuinely authentic (attacker.example is the attacker's OWN
        // domain, so its SPF record can genuinely authorise it), and
        // Gmail's real `Authentication-Results` header echoes that
        // attacker-CHOSEN local-part verbatim inside the SPF section's
        // comment/`smtp.mailfrom=` property. The attacker picks the local
        // part to literally BE `dmarc=pass header.from=greenhouse.io `,
        // injecting exactly the text a SUBSTRING scanner's clause-selection
        // was looking for -- INTO an earlier (SPF) section, ahead of the
        // REAL `dmarc=fail ... header.from=attacker.example` section that
        // comes later in the SAME genuine header.
        //
        // A tokeniser that tracks comment/quoted-string state (see
        // `super::auth_results::dmarc_verdict`) never surfaces either echo
        // as a `dmarc=` methodspec -- the injected text lives inside a
        // DIFFERENT section's OWN comment/quoted pvalue, structurally
        // distinct from a real `dmarc=` token, not merely a sharper
        // heuristic about WHICH occurrence to trust.
        let ar = "mx.google.com; \
                   dkim=pass header.i=@attacker.example header.s=selector header.b=xyz789; \
                   spf=pass (google.com: domain of \"dmarc=pass header.from=greenhouse.io \"@attacker.example designates 5.6.7.8 as permitted sender) smtp.mailfrom=\"dmarc=pass header.from=greenhouse.io \"@attacker.example; \
                   dmarc=fail (p=REJECT sp=REJECT dis=NONE) header.from=attacker.example"
            .to_string();
        assert!(
            !dmarc_pass_aligned(&[ar], Some("greenhouse.io")),
            "the REAL dmarc=fail section must win -- the injected comment/quoted-string text \
             must never be read as a methodspec"
        );
    }
}
