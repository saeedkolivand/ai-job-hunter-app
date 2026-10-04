//! `ats.header_in_body`: when a second contact block is a defect and when it is a referee,
//! a salary range or an address in a bullet.

use super::{support::*, *};

/// A résumé that pastes the candidate's own header block a second time.
const REPEATED_HEADER: &str = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                               Acme | 2021 - Present\n- Led the migration\n\n\
                               CONTACT\n\nJane Doe\njane@example.com\n";

/// A second contact block in the body is Critical; a body line that merely
/// mentions an address is not. The second half is the false positive that
/// would make the check unusable.
///
/// ⚠️ The positive fixture was `REFERENCES / John Smith / john.smith@…` until
/// R13-W1: that is a REFERENCES LIST, not a second header, and this test was
/// pinning the false positive as the expected behaviour. The block is now the
/// candidate's own header repeated, which is what the Critical claims. The
/// referee shape has its own test —
/// `a_reference_block_is_not_a_second_contact_block`.
#[test]
fn header_in_body_needs_a_contact_cluster_not_just_an_email() {
    let with_cluster = REPEATED_HEADER;
    let report = report_against(with_cluster, with_cluster);
    let hits = fired(&report, ATS_HEADER_IN_BODY);
    assert_eq!(hits[0].severity, Severity::Critical);

    let mentions_an_email = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                             Acme | 2021 - Present\n\
                             - Ran the support alias support@acme.example.com for the whole team\n";
    let report = report_against(mentions_an_email, mentions_an_email);
    assert!(
        !codes(&report).contains(&ATS_HEADER_IN_BODY),
        "a bullet mentioning an address is not a contact block; got {:?}",
        codes(&report)
    );
}

/// H6 — the contact-cluster Critical needs a REAL address, and the name-like
/// line has to be the section's first. A stray `@` in a bullet and a short line
/// anywhere next to it were enough to claim the document had two headers.
#[test]
fn header_in_body_needs_a_real_address_directly_under_the_heading() {
    // An `@` that is not an address.
    let handle = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                  Acme | 2021 - Present\n\
                  On call\nOwned the @payments rotation for two years\n";
    silent(&report_against(handle, handle), ATS_HEADER_IN_BODY);

    // A real address deeper inside a section, under a body line rather than at
    // the top of it.
    let body_address = "Jane Doe\njane@example.com\n\nPUBLICATIONS\n\n\
                        Scaling ledgers under load\n\
                        Rust Conf\n\
                        Recordings are available from talks@rustconf.example.com\n";
    silent(
        &report_against(body_address, body_address),
        ATS_HEADER_IN_BODY,
    );

    // The real thing — the CANDIDATE's name on the section's first line, their
    // own address under it. (Was a referee's until R13-W1; see
    // `a_reference_block_is_not_a_second_contact_block` for why that shape is
    // not a second header.)
    let cluster = REPEATED_HEADER;
    let report = report_against(cluster, cluster);
    let hits = fired(&report, ATS_HEADER_IN_BODY);
    assert_eq!(hits[0].severity, Severity::Critical);
}

/// R8-F2 — `ats.header_in_body` is a CRITICAL, and its phone test was
/// `export::parser`'s `PHONE_RE`, which accepts ANY run of seven or more
/// digits, spaces and hyphens. A salary range in ordinary prose ("150 - 200")
/// satisfies it, so a body line under any short line was reported as a second
/// contact block.
#[test]
fn a_salary_range_in_the_body_is_not_a_second_contact_block() {
    let doc = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
               Acme | 2021 - Present\n- Led the migration\n\n\
               VENDOR MANAGEMENT\n\n\
               Rate negotiation\n\
               Renegotiated agency rates from 150 - 200 EUR per hour across twelve suppliers\n";
    silent(&report_against(doc, doc), ATS_HEADER_IN_BODY);

    // A grouped-figure budget line is the same shape in German.
    let de = "Jana Mustermann\njana@example.com\n\nBERUFSERFAHRUNG\n\n\
              Acme | 2021 - Heute\n- Die Abrechnung betreut\n\n\
              BUDGETVERANTWORTUNG\n\n\
              Jahresbudget\n\
              Das Jahresbudget von 90 000 - 110 000 EUR eigenverantwortlich gesteuert\n";
    silent(&report_against(de, de), ATS_HEADER_IN_BODY);

    // …and a real phone number under the CANDIDATE's own name still IS a second
    // contact block, in every form a résumé header writes one. (The name was a
    // referee's until R13-W1 — see
    // `a_reference_block_is_not_a_second_contact_block`.)
    for phone in [
        "+49 30 1234567",
        "+49 (0)30 1234567",
        "(030) 12345678",
        "0176 12345678",
        "+1 (555) 123-4567",
    ] {
        let cluster = format!(
            "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
             Acme | 2021 - Present\n- Led the migration\n\n\
             CONTACT\n\nJane Doe\n{phone}\n"
        );
        let report = report_against(&cluster, &cluster);
        let hits = fired(&report, ATS_HEADER_IN_BODY);
        assert_eq!(hits[0].severity, Severity::Critical, "{phone}");
    }
}

/// R13-W1 — the round-12 `i == 1` narrowing's own named false positive survives
/// at exactly that index. A conventional references block is a NAME on the
/// section's first line and that person's address under it: the heading goes to
/// `Section::heading` rather than into `lines`, `Blank` is filtered out, so the
/// reference lands at index 1 and satisfies every clause. The candidate is told
/// their document has two headers because they listed a referee.
///
/// What the Critical actually claims is that the candidate's OWN header appears
/// twice, so the discriminator is whose details these are.
#[test]
fn a_reference_block_is_not_a_second_contact_block() {
    let referees = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                    Acme | 2021 - Present\n- Led the migration\n\n\
                    REFERENCES\n\nMaria Lang\nmaria.lang@acme.example.com\n";
    silent(&report_against(referees, referees), ATS_HEADER_IN_BODY);

    // A contact PERSON under a heading no list knows is the same shape and the
    // same non-defect.
    let ansprechpartner = "Jana Mustermann\njana@example.com\n\nBERUFSERFAHRUNG\n\n\
                           Acme | 2021 - Heute\n- Die Abrechnung betreut\n\n\
                           ANSPRECHPARTNER\n\nMaria Lang\nmaria.lang@acme.example.com\n";
    silent(
        &report_against(ansprechpartner, ansprechpartner),
        ATS_HEADER_IN_BODY,
    );

    // The genuine defect — the candidate's own header block, pasted a second
    // time — is still Critical.
    let repeated = REPEATED_HEADER;
    let report = report_against(repeated, repeated);
    assert_eq!(
        fired(&report, ATS_HEADER_IN_BODY)[0].severity,
        Severity::Critical
    );
}
