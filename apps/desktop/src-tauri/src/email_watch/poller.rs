//! Tauri-free IMAP tick orchestration: search, then header fetch, then
//! fingerprint filter, then body fetch (matched candidates only), then
//! company/title extraction plus intent classification, then match against
//! a caller-supplied application snapshot. Candidacy is decided entirely by
//! [`matcher::best_match`] (live statuses, plus terminal ones the caller
//! marks via `unconfirmed_email_write_ids`) — this file does not filter by
//! status itself, it only threads that set through.
//!
//! No `AppHandle`/notification/write concern here — that is
//! `email_watch_scheduler`'s job (L2), the ONE place in this module family
//! with the upward reach into `commands::notifications` AND the one that
//! calls `email_watch::auto_write::apply_matched_intent`. Everything in this
//! file is either a synchronous IMAP round trip or cheap in-process regex
//! work, so [`run_tick`] is safe to call from inside a `spawn_blocking`
//! closure.
//!
//! **Privacy**: returns only uids, application ids/scores, and a classified
//! [`crate::email_watch::intent::EmailIntent`] variant — the raw
//! subject/sender/body text is parsed and discarded here, never surfaced in
//! [`TickResult`].

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;

use crate::applications::Application;
use crate::email_watch::imap_client;
use crate::email_watch::intent::{self, EmailIntent};
use crate::email_watch::matcher;
use crate::email_watch::parser;
use crate::error::AppResult;

/// One considered message's outcome. `matched_application_id` is `None` for a
/// message that was fetched (and possibly fingerprint-checked) but didn't
/// resolve to a saved application — the caller still marks it `seen` so it is
/// never re-considered on a later tick.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageOutcome {
    pub uid: u32,
    pub matched_application_id: Option<String>,
    /// The classified intent for this message — `None` covers "did not
    /// fingerprint", "no body/header could be parsed", and "classified but
    /// no discriminating phrase decided anything" uniformly. The caller
    /// (`email_watch_scheduler`) only ever acts on `Some`, and
    /// [`crate::email_watch::auto_write::apply_matched_intent`] treats
    /// `None` as its own no-op — see that fn's doc.
    pub intent: Option<EmailIntent>,
    /// **HIGH-2 fix**: whether this message clears the auto-write gate —
    /// [`parser::Fingerprint::write_gate_domain`] (the narrower ATS-tenant
    /// domain list) AND [`parser::EmailHeader::dmarc_pass`] (DMARC `pass`,
    /// aligned to the visible `From:` domain), BOTH required. Deliberately a
    /// SEPARATE signal from the score-boost `domain_hint` fed to
    /// [`matcher::best_match`] below (the wider, UNauthenticated sender-domain
    /// list — a boost only, never gates anything) — the two used to be the
    /// SAME boolean, which meant an unauthenticated sender-domain string
    /// match alone authorized a write. Carried through (independent of
    /// `matched_application_id`) so the auto-write path's gate has it
    /// without re-parsing the header.
    pub write_authorized: bool,
}

#[derive(Debug, Clone)]
pub struct TickResult {
    pub uidvalidity: u32,
    /// Whether the live `UIDVALIDITY` differs from `stored_uidvalidity` — the
    /// caller must reset its persisted watermark when this is `true` (see
    /// `EmailWatchStore::reset_on_uidvalidity_change`).
    pub uidvalidity_changed: bool,
    /// Only messages ABOVE the effective watermark — see [`run_tick`].
    pub outcomes: Vec<MessageOutcome>,
}

/// Whether the LIVE `UIDVALIDITY` (just read via `EXAMINE`) differs from the
/// previously stored one — a mailbox renumbering, meaning the old watermark
/// may refer to entirely different messages now. Pure so it is directly
/// unit-testable without a network round trip.
fn has_uidvalidity_changed(stored_uidvalidity: Option<u32>, live_uidvalidity: u32) -> bool {
    stored_uidvalidity != Some(live_uidvalidity)
}

/// The `last_uid` bound to filter fetched headers (or, equally, to seed a
/// post-tick watermark advance) against — `None` (everything counts as new)
/// after a UIDVALIDITY change, otherwise the caller's stored watermark
/// unchanged. `pub(crate)`: [`run_tick`] uses it internally, and
/// `email_watch_scheduler` reuses the SAME fn (rather than reimplementing the
/// identical if/else) to seed its post-tick `advance_last_uid` call — one
/// decision, one place, inheriting these tests.
pub(crate) fn effective_last_uid(
    uidvalidity_changed: bool,
    stored_last_uid: Option<u32>,
) -> Option<u32> {
    if uidvalidity_changed {
        None
    } else {
        stored_last_uid
    }
}

/// Hard cap on how many above-the-watermark headers a single tick processes
/// (fingerprint + candidate-body-fetch + match). A burst of new mail since
/// the last check (a long-disconnected mailbox, a mass-application day) is
/// bounded rather than processed unboundedly in one blocking pass; the
/// watermark only advances to the highest uid actually processed, so any
/// remainder is picked up on the NEXT tick — nothing is permanently skipped,
/// just spread across ticks. Lowest-uid-first (oldest unprocessed mail),
/// never highest-first, so the remainder is always the newer half.
const MAX_HEADERS_PER_TICK: usize = 200;

/// Sort `headers` ascending by uid and cap to at most `cap` entries —
/// oldest-first, so any never-reached remainder this tick is picked up on
/// the next one. Pure/network-free, factored out of [`run_tick`] so the
/// ordering+cap behavior itself is directly unit-testable.
fn cap_oldest_first(
    mut headers: Vec<&imap_client::HeaderCandidate>,
    cap: usize,
) -> Vec<&imap_client::HeaderCandidate> {
    headers.sort_unstable_by_key(|h| h.uid);
    headers.truncate(cap);
    headers
}

/// HIGH-2 fix: whether one message clears the auto-write gate —
/// [`parser::Fingerprint::write_gate_domain`] (the narrower ATS-tenant
/// domain list) AND [`parser::EmailHeader::dmarc_pass`] (DMARC `pass`,
/// aligned to the visible `From:` domain), BOTH required. Pure/factored out
/// of [`run_tick`] so the AND-combination itself — not just its two inputs
/// in isolation — is directly unit-testable across all four truth-table
/// cases, rather than only reachable through a live IMAP round trip.
fn compute_write_authorized(fp: &parser::Fingerprint, header: &parser::EmailHeader) -> bool {
    fp.write_gate_domain && header.dmarc_pass
}

/// Run one IMAP tick: fetch headers since `since`, drop anything at or below
/// the watermark (recomputed against the LIVE `UIDVALIDITY`, since a stale
/// `stored_last_uid` is meaningless after a mailbox renumbering), fingerprint
/// each remaining header, fetch the body ONLY for a fingerprint hit, then
/// match against `candidate_applications` AND classify the 4-way intent from
/// the SAME decoded subject/body — see [`MessageOutcome::intent`].
/// `unconfirmed_email_write_ids` is passed straight through to
/// [`matcher::best_match`] — see that fn's doc for why a terminal status can
/// still be a candidate.
///
/// Blocking (real network I/O) — call only from `spawn_blocking`.
#[allow(clippy::too_many_arguments)] // house convention (see clippy.toml threshold=8) — every param is a distinct required IMAP/watermark/candidacy input, not a bundling smell
pub fn run_tick(
    host: &str,
    port: u16,
    address: &str,
    app_password: &str,
    since: NaiveDate,
    stored_uidvalidity: Option<u32>,
    stored_last_uid: Option<u32>,
    candidate_applications: &[Application],
    unconfirmed_email_write_ids: &HashSet<String>,
) -> AppResult<TickResult> {
    let header_fetch = imap_client::fetch_headers_since(
        host,
        port,
        address,
        app_password,
        since,
        stored_uidvalidity,
        stored_last_uid,
    )?;
    let uidvalidity_changed = has_uidvalidity_changed(stored_uidvalidity, header_fetch.uidvalidity);
    let effective_last_uid = effective_last_uid(uidvalidity_changed, stored_last_uid);

    let relevant: Vec<&imap_client::HeaderCandidate> = header_fetch
        .headers
        .iter()
        .filter(|h| effective_last_uid.is_none_or(|lu| h.uid > lu))
        .collect();
    let relevant = cap_oldest_first(relevant, MAX_HEADERS_PER_TICK);

    // Parse + fingerprint every relevant header up front (cheap, in-process,
    // no network) so we know exactly which uids need a body fetch.
    // `domain_hint` (score boost, unauthenticated) and `write_authorized`
    // (HIGH-2: the authenticated write gate) are deliberately two SEPARATE
    // fields — see [`MessageOutcome::write_authorized`]'s doc.
    struct ParsedHeader {
        uid: u32,
        header: Option<parser::EmailHeader>,
        is_candidate: bool,
        domain_hint: bool,
        write_authorized: bool,
    }
    let mut parsed: Vec<ParsedHeader> = Vec::with_capacity(relevant.len());
    let mut candidate_uids = Vec::new();
    for h in &relevant {
        match parser::parse_header(&h.raw_header) {
            Some(header) => {
                let fp = parser::fingerprint(&header);
                if fp.is_candidate() {
                    candidate_uids.push(h.uid);
                }
                let write_authorized = compute_write_authorized(&fp, &header);
                parsed.push(ParsedHeader {
                    uid: h.uid,
                    header: Some(header),
                    is_candidate: fp.is_candidate(),
                    domain_hint: fp.domain_hint,
                    write_authorized,
                });
            }
            None => parsed.push(ParsedHeader {
                uid: h.uid,
                header: None,
                is_candidate: false,
                domain_hint: false,
                write_authorized: false,
            }),
        }
    }

    let bodies: HashMap<u32, Vec<u8>> = if candidate_uids.is_empty() {
        HashMap::new()
    } else {
        imap_client::fetch_bodies(host, port, address, app_password, &candidate_uids)?
            .into_iter()
            .collect()
    };

    let outcomes = parsed
        .into_iter()
        .map(
            |ParsedHeader {
                 uid,
                 header,
                 is_candidate,
                 domain_hint,
                 write_authorized,
             }| {
                let header = header.filter(|_| is_candidate);
                // The fetch itself is already bounded at the protocol level to
                // `imap_client::MAX_BODY_BYTES` (a partial-octet FETCH, see
                // `imap_client::body_fetch_item_spec`) — this second cap is
                // defense-in-depth only, for a non-compliant server that ignores
                // the partial-fetch hint and returns the whole message anyway.
                // Computed ONCE and reused for both matching and intent
                // classification below, so a fingerprint hit never fetches or
                // decodes the body twice.
                let body_text = header.as_ref().and_then(|_| {
                    bodies.get(&uid).and_then(|raw| {
                        let capped = &raw[..raw.len().min(imap_client::MAX_BODY_BYTES)];
                        parser::parse_body_text(capped)
                    })
                });
                let matched_application_id = header.as_ref().and_then(|header| {
                    let candidates = parser::extract_candidates(
                        &header.subject,
                        body_text.as_deref(),
                        header.from_name.as_deref(),
                    );
                    matcher::best_match(
                        &candidates,
                        candidate_applications,
                        domain_hint,
                        unconfirmed_email_write_ids,
                    )
                    .map(|scored| scored.application_id)
                });
                let intent = header.as_ref().and_then(|header| {
                    intent::classify_intent(&header.subject, body_text.as_deref())
                });
                MessageOutcome {
                    uid,
                    matched_application_id,
                    intent,
                    write_authorized,
                }
            },
        )
        .collect();

    Ok(TickResult {
        uidvalidity: header_fetch.uidvalidity,
        uidvalidity_changed,
        outcomes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::applications::ApplicationStatus;

    fn saved_app(id: &str, company: &str, title: &str) -> Application {
        Application {
            id: id.to_string(),
            status: ApplicationStatus::Saved,
            applied_at: None,
            created_at: 0,
            updated_at: 0,
            job_url: String::new(),
            board: String::new(),
            company: company.to_string(),
            title: title.to_string(),
            candidate: String::new(),
            answers: Vec::new(),
            brief: String::new(),
            job_description: String::new(),
            notes: String::new(),
            next_action_at: None,
            next_action_notified_at: None,
            comp: String::new(),
            contact_name: String::new(),
            contact_email: String::new(),
            job_summary: String::new(),
            recipient_name: String::new(),
            recipient_email: String::new(),
            salary_min: None,
            salary_max: None,
            salary_currency: None,
        }
    }

    // `run_tick` itself needs a live IMAP server (documented gap, mirrors
    // `imap_client`'s own network-round-trip functions) — but the pure
    // watermark decisions it delegates to are tested directly here.

    #[test]
    fn unchanged_uidvalidity_keeps_the_stored_watermark() {
        assert!(!has_uidvalidity_changed(Some(7), 7));
        assert_eq!(effective_last_uid(false, Some(100)), Some(100));
    }

    #[test]
    fn a_uidvalidity_change_resets_the_watermark_to_none() {
        assert!(has_uidvalidity_changed(Some(7), 8));
        assert_eq!(effective_last_uid(true, Some(100)), None);
    }

    #[test]
    fn no_stored_uidvalidity_yet_also_counts_as_changed() {
        // First-ever connect: nothing stored yet, so `Some(_)` never matches
        // and every fetched header is treated as new.
        assert!(has_uidvalidity_changed(None, 7));
    }

    // ── cap_oldest_first (rust-backend-architect advisory #4) ───────────────

    fn header_with_uid(uid: u32) -> imap_client::HeaderCandidate {
        imap_client::HeaderCandidate {
            uid,
            raw_header: Vec::new(),
        }
    }

    #[test]
    fn cap_oldest_first_sorts_ascending_and_caps_the_count() {
        let (h5, h1, h3) = (header_with_uid(5), header_with_uid(1), header_with_uid(3));
        let capped = cap_oldest_first(vec![&h5, &h1, &h3], 2);
        let uids: Vec<u32> = capped.iter().map(|h| h.uid).collect();
        assert_eq!(uids, vec![1, 3], "keeps the lowest (oldest) uids first");
    }

    #[test]
    fn cap_oldest_first_is_a_no_op_under_the_cap() {
        let (h2, h1) = (header_with_uid(2), header_with_uid(1));
        let capped = cap_oldest_first(vec![&h2, &h1], 200);
        let uids: Vec<u32> = capped.iter().map(|h| h.uid).collect();
        assert_eq!(uids, vec![1, 2]);
    }

    // A write-gate-eligible domain (`greenhouse.io`) vs one that is not
    // (`example.com`) — going through the real `parser::fingerprint` (its
    // `subject_matched`/`domain_hint` fields are private, so a direct
    // struct literal isn't constructible from here; a subject that never
    // matches a fingerprint phrase is irrelevant to what this test checks).
    fn fp(write_gate_eligible_domain: bool) -> parser::Fingerprint {
        let domain = if write_gate_eligible_domain {
            "greenhouse.io"
        } else {
            "example.com"
        };
        parser::fingerprint(&email_header("irrelevant subject", Some(domain), false))
    }

    fn header_with_dmarc(dmarc_pass: bool) -> parser::EmailHeader {
        email_header("irrelevant subject", None, dmarc_pass)
    }

    fn email_header(
        subject: &str,
        from_domain: Option<&str>,
        dmarc_pass: bool,
    ) -> parser::EmailHeader {
        parser::EmailHeader {
            subject: subject.to_string(),
            from_name: None,
            from_domain: from_domain.map(str::to_string),
            message_id: None,
            dmarc_pass,
        }
    }

    #[test]
    fn compute_write_authorized_requires_both_the_write_gate_domain_and_dmarc_pass() {
        // HIGH-2: the full truth table — neither signal alone is
        // sufficient, matching `apply_matched_intent`'s doc.
        assert!(
            compute_write_authorized(&fp(true), &header_with_dmarc(true)),
            "both true -> authorized"
        );
        assert!(
            !compute_write_authorized(&fp(true), &header_with_dmarc(false)),
            "write-gate domain alone (no DMARC) must not authorize"
        );
        assert!(
            !compute_write_authorized(&fp(false), &header_with_dmarc(true)),
            "DMARC pass alone (not a write-gate domain, e.g. linkedin.com) must not authorize"
        );
        assert!(!compute_write_authorized(
            &fp(false),
            &header_with_dmarc(false)
        ));
    }

    #[test]
    fn saved_app_helper_starts_out_matchable_by_matcher() {
        // Sanity seam: confirms the fixture helper used by `run_tick`'s own
        // (network-gapped) integration is wired to a real `Saved` row the
        // matcher would actually consider.
        let apps = vec![saved_app("a1", "Acme Corp", "Engineer")];
        let candidates = parser::Candidates {
            company: Some("Acme Corp".to_string()),
            title: None,
        };
        assert_eq!(
            matcher::best_match(&candidates, &apps, false, &HashSet::new())
                .map(|s| s.application_id),
            Some("a1".to_string())
        );
    }
}
