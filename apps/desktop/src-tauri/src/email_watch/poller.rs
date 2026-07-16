//! Tauri-free IMAP tick orchestration: search → header fetch → fingerprint
//! filter → body fetch (matched candidates only) → company/title extraction
//! → match against a caller-supplied `saved` application snapshot.
//!
//! No `AppHandle`/notification concern here — that is
//! `email_watch_scheduler`'s job (L2), the ONE place in this module family
//! with the upward reach into `commands::notifications`. Everything in this
//! file is either a synchronous IMAP round trip or cheap in-process regex
//! work, so [`run_tick`] is safe to call from inside a `spawn_blocking`
//! closure.
//!
//! **Privacy**: returns only uids and application ids/scores — the raw
//! subject/sender/body text is parsed and discarded here, never surfaced in
//! [`TickResult`].

use std::collections::HashMap;

use chrono::NaiveDate;

use crate::applications::Application;
use crate::email_watch::imap_client;
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

/// Whether the LIVE `UIDVALIDITY` (just read via `SELECT`) differs from the
/// previously stored one — a mailbox renumbering, meaning the old watermark
/// may refer to entirely different messages now. Pure so it is directly
/// unit-testable without a network round trip.
fn has_uidvalidity_changed(stored_uidvalidity: Option<u32>, live_uidvalidity: u32) -> bool {
    stored_uidvalidity != Some(live_uidvalidity)
}

/// The `last_uid` bound [`run_tick`] filters fetched headers against —
/// `None` (every header counts as new) after a UIDVALIDITY change, otherwise
/// the caller's stored watermark unchanged.
fn effective_last_uid(uidvalidity_changed: bool, stored_last_uid: Option<u32>) -> Option<u32> {
    if uidvalidity_changed {
        None
    } else {
        stored_last_uid
    }
}

/// Run one IMAP tick: fetch headers since `since`, drop anything at or below
/// the watermark (recomputed against the LIVE `UIDVALIDITY`, since a stale
/// `stored_last_uid` is meaningless after a mailbox renumbering), fingerprint
/// each remaining header, fetch the body ONLY for a fingerprint hit, and
/// match against `saved_applications`.
///
/// Blocking (real network I/O) — call only from `spawn_blocking`.
pub fn run_tick(
    host: &str,
    port: u16,
    address: &str,
    app_password: &str,
    since: NaiveDate,
    stored_uidvalidity: Option<u32>,
    stored_last_uid: Option<u32>,
    saved_applications: &[Application],
) -> AppResult<TickResult> {
    let header_fetch = imap_client::fetch_headers_since(host, port, address, app_password, since)?;
    let uidvalidity_changed = has_uidvalidity_changed(stored_uidvalidity, header_fetch.uidvalidity);
    let effective_last_uid = effective_last_uid(uidvalidity_changed, stored_last_uid);

    let relevant: Vec<&imap_client::HeaderCandidate> = header_fetch
        .headers
        .iter()
        .filter(|h| effective_last_uid.is_none_or(|lu| h.uid > lu))
        .collect();

    // Parse + fingerprint every relevant header up front (cheap, in-process,
    // no network) so we know exactly which uids need a body fetch.
    let mut parsed: Vec<(u32, Option<parser::EmailHeader>, bool, bool)> =
        Vec::with_capacity(relevant.len());
    let mut candidate_uids = Vec::new();
    for h in &relevant {
        match parser::parse_header(&h.raw_header) {
            Some(header) => {
                let fp = parser::fingerprint(&header);
                if fp.is_candidate() {
                    candidate_uids.push(h.uid);
                }
                parsed.push((h.uid, Some(header), fp.is_candidate(), fp.domain_hint));
            }
            None => parsed.push((h.uid, None, false, false)),
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
        .map(|(uid, header, is_candidate, domain_hint)| {
            let matched_application_id = header.filter(|_| is_candidate).and_then(|header| {
                let body_text = bodies
                    .get(&uid)
                    .and_then(|raw| parser::parse_body_text(raw));
                let candidates = parser::extract_candidates(
                    &header.subject,
                    body_text.as_deref(),
                    header.from_name.as_deref(),
                );
                matcher::best_match(&candidates, saved_applications, domain_hint)
                    .map(|scored| scored.application_id)
            });
            MessageOutcome {
                uid,
                matched_application_id,
            }
        })
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
            matcher::best_match(&candidates, &apps, false).map(|s| s.application_id),
            Some("a1".to_string())
        );
    }
}
