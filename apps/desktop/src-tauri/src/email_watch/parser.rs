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
///
/// **"ALIGNED" here is EXACT (`eq_ignore_ascii_case`), STRICTER than
/// DMARC's own relaxed-alignment default** — real DMARC accepts an
/// organizational-domain match (RFC 7489 §3.1's "Organizational Domain"),
/// so a genuine `header.from=greenhouse.io` stamp on a message whose
/// visible `From:` is `careers@mail.greenhouse.io` is legitimately
/// DMARC-aligned but returns `false` HERE — this fn does not implement
/// DMARC's own alignment rule, it implements a narrower one. Deliberate,
/// not an oversight: [`domain_matches_any`] (the sender-domain hint/
/// write-gate check one layer up) DOES accept subdomains, so the two
/// checks are asymmetric on purpose — the direction is safe (a
/// legitimate pass on a subdomain goes UNRECOGNISED here and the write
/// gate simply doesn't fire, never the reverse: nothing this strictness
/// removes could have let a FORGERY through). Loosening this to match
/// DMARC's real relaxed-alignment rule would need to compute each
/// domain's registrable/organizational domain (a public-suffix-list
/// lookup, not a string comparison) — real scope, not a one-line change,
/// and not needed while the strict direction is merely under-matching
/// rather than over-trusting.
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
mod tests;
