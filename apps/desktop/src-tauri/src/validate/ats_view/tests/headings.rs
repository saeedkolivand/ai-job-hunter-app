//! Heading recognition, contact fields and the reading-order heuristic on
//! synthetic extracted text.

use super::super::{ats_view_from_text, AtsHints, AtsReason, AtsSectionKind, AtsStatus, AtsView};

fn view(text: &str) -> AtsView {
    ats_view_from_text(text, AtsHints::default())
}

fn section(v: &AtsView, kind: AtsSectionKind) -> Vec<(Option<&str>, AtsStatus)> {
    v.sections
        .iter()
        .filter(|s| s.kind == kind)
        .map(|s| (s.heading.as_deref(), s.status))
        .collect()
}

#[test]
fn english_and_german_headings_are_standard() {
    let v = view("Jane Doe\nBerufserfahrung\nx\nAUSBILDUNG\nx\nKompetenzen\nx\nProfil\n");
    assert_eq!(
        section(&v, AtsSectionKind::Experience),
        [(Some("Berufserfahrung"), AtsStatus::Found)]
    );
    assert_eq!(
        section(&v, AtsSectionKind::Education),
        [(Some("AUSBILDUNG"), AtsStatus::Found)]
    );
    assert_eq!(
        section(&v, AtsSectionKind::Skills),
        [(Some("Kompetenzen"), AtsStatus::Found)]
    );
    assert_eq!(
        section(&v, AtsSectionKind::Summary),
        [(Some("Profil"), AtsStatus::Found)]
    );
}

#[test]
fn professional_experience_all_caps_is_standard_via_classification() {
    let v = view("Jane Doe\nPROFESSIONAL EXPERIENCE\nx\n");
    assert_eq!(
        section(&v, AtsSectionKind::Experience),
        [(Some("PROFESSIONAL EXPERIENCE"), AtsStatus::Found)]
    );
}

#[test]
fn creative_all_caps_heading_is_suspicious() {
    let v = view("Jane Doe\nMY JOURNEY SO FAR\nx\n");
    let s = v
        .sections
        .iter()
        .find(|s| s.heading.as_deref() == Some("MY JOURNEY SO FAR"))
        .unwrap();
    assert_eq!(s.status, AtsStatus::Suspicious);
    assert_eq!(s.reason, Some(AtsReason::NonstandardHeading));
    assert_eq!(s.kind, AtsSectionKind::Other);
}

#[test]
fn absent_core_sections_are_reported_missing_once() {
    let v = view("Jane Doe\nSKILLS\nRust\n");
    assert_eq!(
        section(&v, AtsSectionKind::Experience),
        [(None, AtsStatus::Missing)]
    );
    assert_eq!(
        section(&v, AtsSectionKind::Education),
        [(None, AtsStatus::Missing)]
    );
    assert_eq!(
        section(&v, AtsSectionKind::Skills),
        [(Some("SKILLS"), AtsStatus::Found)]
    );
}

#[test]
fn heading_fused_to_its_content_is_split_and_found() {
    let v = view(
        "Jane Doe\nWORK EXPERIENCEAcme Corp Mar 2021 - Present\nSenior Engineer\nSKILLS• Rust\n",
    );
    assert_eq!(
        section(&v, AtsSectionKind::Experience),
        [(Some("WORK EXPERIENCE"), AtsStatus::Found)]
    );
    assert_eq!(
        section(&v, AtsSectionKind::Skills),
        [(Some("SKILLS"), AtsStatus::Found)]
    );
    // The role underneath is still read.
    assert_eq!(v.roles[0].company.value.as_deref(), Some("Acme Corp"));
}

#[test]
fn uppercase_name_is_not_a_heading() {
    let v = view("JANE DOE\njane@example.com\nSKILLS\nRust\n");
    assert!(v
        .sections
        .iter()
        .all(|s| s.heading.as_deref() != Some("JANE DOE")));
    assert_eq!(v.contact.name.value.as_deref(), Some("JANE DOE"));
}

#[test]
fn contact_fields_found_and_missing() {
    let v =
        view("Jane Doe\nBerlin, Germany | +49 151 2345 6789 | jane@example.com\nSKILLS\nRust\n");
    assert_eq!(v.contact.email.value.as_deref(), Some("jane@example.com"));
    assert_eq!(v.contact.phone.value.as_deref(), Some("+49 151 2345 6789"));
    assert_eq!(v.contact.location.value.as_deref(), Some("Berlin, Germany"));
    let bare = view("Jane Doe\nSKILLS\nRust\n");
    for f in [
        &bare.contact.email,
        &bare.contact.phone,
        &bare.contact.location,
    ] {
        assert_eq!(
            (f.status, f.reason),
            (AtsStatus::Missing, Some(AtsReason::NotFound))
        );
    }
}

#[test]
fn year_ranges_are_not_phone_numbers() {
    let v = view("Jane Doe\nEDUCATION\nState University 2013 - 2017\n");
    assert_eq!(v.contact.phone.status, AtsStatus::Missing);
}

#[test]
fn phone_wrapped_across_lines_is_suspicious() {
    let v =
        view("Jane Doe\nBerlin, Germany | +49 151 2345\n6789 | jane@example.com\nSKILLS\nRust\n");
    assert_eq!(v.contact.phone.status, AtsStatus::Suspicious);
    assert_eq!(v.contact.phone.reason, Some(AtsReason::Wrapped));
}

#[test]
fn a_german_address_block_is_not_a_wrapped_phone() {
    let text = "Erika Mustermann\nMusterstraße 12\n10115 Berlin\n+49 30 1234567 | erika@example.de\nSKILLS\nRust\n";
    let v = view(text);
    assert_eq!(v.contact.phone.status, AtsStatus::Found);
    assert_eq!(v.contact.phone.value.as_deref(), Some("+49 30 1234567"));
    // The address alone never produces a phone at all.
    let no_phone = view("Erika Mustermann\nMusterstraße 12\n10115 Berlin\nSKILLS\nRust\n");
    assert_eq!(no_phone.contact.phone.status, AtsStatus::Missing);
}

#[test]
fn hinted_name_matches_whole_words_only() {
    let hints = AtsHints { name: Some("Li") };
    let v = ats_view_from_text("Lisa Marie\nSKILLS\nRust\n", hints);
    assert_eq!(v.contact.name.status, AtsStatus::Missing);
    let ok = ats_view_from_text("Li Wei\nSKILLS\nRust\n", hints);
    assert_eq!(ok.contact.name.status, AtsStatus::Found);
}

#[test]
fn name_fused_to_the_contact_line_is_still_read() {
    let v = view("Jane DoeBerlin, Germany | jane@example.com\nSKILLS\nRust\n");
    assert_eq!(v.contact.name.value.as_deref(), Some("Jane Doe"));
    assert_eq!(v.contact.location.value.as_deref(), Some("Berlin, Germany"));
}

#[test]
fn hinted_name_missing_from_text_is_missing() {
    let hints = AtsHints {
        name: Some("Jane Doe"),
    };
    let v = ats_view_from_text("Someone Else\nSKILLS\nRust\n", hints);
    assert_eq!(v.contact.name.status, AtsStatus::Missing);
}

// ── reading order ────────────────────────────────────────────────────────

fn order(text: &str, name: Option<&str>) -> (AtsStatus, Option<AtsReason>) {
    let v = ats_view_from_text(text, AtsHints { name });
    (v.reading_order.status, v.reading_order.reason)
}

#[test]
fn header_then_sections_is_in_order() {
    let text = "Jane Doe\njane@example.com\nSKILLS\nRust\n";
    assert_eq!(order(text, Some("Jane Doe")).0, AtsStatus::Found);
}

#[test]
fn body_before_header_is_interleaved() {
    // A sidebar extracted first: a heading is the very first line.
    let text = "EDUCATION\nState University\nJane Doe\njane@example.com\nSKILLS\nRust\n";
    assert_eq!(
        order(text, None),
        (AtsStatus::Suspicious, Some(AtsReason::Interleaved))
    );
}

#[test]
fn name_after_first_heading_is_interleaved_when_the_name_is_known() {
    let text = "jane@example.com\nSKILLS\nRust\nJane Doe\nWORK EXPERIENCE\nx\n";
    assert_eq!(
        order(text, Some("Jane Doe")).1,
        Some(AtsReason::Interleaved)
    );
    // Without the source name the same text is not judged (conservative).
    assert_eq!(order(text, None).0, AtsStatus::Found);
}

#[test]
fn email_after_first_heading_is_interleaved() {
    let text = "Jane Doe\nSKILLS\nRust\njane@example.com\n";
    assert_eq!(order(text, None).1, Some(AtsReason::Interleaved));
}

#[test]
fn no_headings_means_no_order_can_be_confirmed() {
    assert_eq!(
        order("Jane Doe\njane@example.com\n", None),
        (AtsStatus::Missing, Some(AtsReason::NotFound))
    );
}

#[test]
fn summary_counts_every_field_once() {
    let s = view("Jane Doe\nSKILLS\nRust\n").summary;
    assert_eq!(s.found + s.suspicious + s.missing, s.total);
    // 4 contact + 1 order + 3 sections (skills found; experience, education missing).
    assert_eq!(s.total, 8);
}
