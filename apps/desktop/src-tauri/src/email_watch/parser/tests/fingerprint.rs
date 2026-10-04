use super::*;
use crate::email_watch::tests::support::email_header;

fn header(subject: &str, from_domain: Option<&str>) -> EmailHeader {
    email_header(subject, from_domain, false)
}

// ── fingerprint: positive (EN + DE), broadened recall, and negative / near-miss ──

#[test]
fn fingerprint_matches_each_phrase_family() {
    for subject in [
        // EN
        "Thank you for applying!",
        "Your application has been received",
        "Your application was submitted",
        "Your application to Acme Corp",
        // DE
        "Ihre Bewerbung bei Acme GmbH",
        "Ihre Bewerbung ist eingegangen",
        "Bewerbung erhalten",
        "Danke für Ihre Bewerbung!",
        "DANKE FÜR IHRE BEWERBUNG", // case-insensitive, including umlauts
        // broadened recall (job-match-expert items 8/9), EN
        "Thanks for applying to Acme!",   // informal contraction
        "Thank you for your application", // "application" not "applying"
        "We've received your application",
        "We have received your application",
        "Application confirmation",
        "Confirmation: received your application", // reverse order, no "we"
        // broadened recall (job-match-expert items 8/9), DE
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

#[test]
fn fingerprint_rejects_unrelated_subjects() {
    for subject in [
        "Your weekly newsletter",
        "Meeting rescheduled to 3pm",
        "Your order has shipped",
        // Contains "applying" but not the gated phrase "thank you for applying".
        "Tips for applying to jobs this year",
    ] {
        assert!(
            !fingerprint(&header(subject, None)).is_candidate(),
            "did not expect a match for {subject:?}"
        );
    }
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
