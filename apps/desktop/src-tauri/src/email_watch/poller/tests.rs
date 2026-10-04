use super::*;
use crate::applications::ApplicationStatus;
use crate::email_watch::tests::support::{application, email_header};

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
    let apps = vec![application(
        "a1",
        "Acme Corp",
        "Engineer",
        ApplicationStatus::Saved,
    )];
    let candidates = parser::Candidates {
        company: Some("Acme Corp".to_string()),
        title: None,
    };
    assert_eq!(
        matcher::best_match(&candidates, &apps, false, &HashSet::new()).map(|s| s.application_id),
        Some("a1".to_string())
    );
}
