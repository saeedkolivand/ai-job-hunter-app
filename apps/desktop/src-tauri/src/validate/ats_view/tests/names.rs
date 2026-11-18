//! Name and phone recognition without a source hint (the file command's case),
//! and the hint's whole-word matching.

use super::super::{ats_view_from_text, AtsHints, AtsSectionKind, AtsStatus, AtsView};

fn view(text: &str) -> AtsView {
    ats_view_from_text(text, AtsHints::default())
}

#[test]
fn german_slash_phone_keeps_its_area_code() {
    let v = view("Erika Mustermann\n089/123456 | erika@example.de\nSKILLS\nRust\n");
    assert_eq!(v.contact.phone.value.as_deref(), Some("089/123456"));
    let mobile = view("Erika Mustermann\n0171/1234567\nSKILLS\nRust\n");
    assert_eq!(mobile.contact.phone.value.as_deref(), Some("0171/1234567"));
}

#[test]
fn names_with_particles_credentials_and_uncased_scripts_are_read_without_a_hint() {
    for (first, want) in [
        ("Jan van der Berg", "Jan van der Berg"),
        ("Maria de la Cruz", "Maria de la Cruz"),
        ("Omar bin Salem", "Omar bin Salem"),
        ("Jane Doe, PhD", "Jane Doe, PhD"),
        ("Dr. Jane Doe, M.Sc.", "Dr. Jane Doe, M.Sc."),
        ("山田太郎", "山田太郎"),
        ("محمد علي", "محمد علي"),
    ] {
        let v = view(&format!(
            "{first}\nBerlin | jane@example.com\nWORK EXPERIENCE\nx\n"
        ));
        assert_eq!(v.contact.name.value.as_deref(), Some(want), "{first}");
    }
}

/// The verified repro: a particle name used to be rejected, so the first
/// "name-shaped" line was the heading, which was then skipped (Experience Missing).
#[test]
fn a_particle_name_does_not_turn_the_experience_heading_into_the_name() {
    let text = "Jan van der Berg\nBerlin, Germany | +49 30 1234567 | jan@example.com\n\
                PROFESSIONAL EXPERIENCE\nAcme Corp 2020 - 2022\nEngineer\n";
    let v = view(text);
    assert_eq!(v.contact.name.value.as_deref(), Some("Jan van der Berg"));
    assert!(v
        .sections
        .iter()
        .any(|s| s.kind == AtsSectionKind::Experience && s.status == AtsStatus::Found));
    assert_eq!(v.roles.len(), 1);
}

#[test]
fn an_all_caps_first_heading_is_never_the_name() {
    // No name at all: the first line is a recognised ALL-CAPS heading.
    let v = view("PROFESSIONAL EXPERIENCE\nAcme Corp 2020 - 2022\nEngineer\n");
    assert_eq!(v.contact.name.status, AtsStatus::Missing);
    assert_eq!(v.roles.len(), 1);
    // ...while an all-caps NAME still reads as a name.
    let caps = view("JAN VAN DER BERG\nBerlin | jan@example.com\nWORK EXPERIENCE\nx\n");
    assert_eq!(caps.contact.name.status, AtsStatus::Found);
}

#[test]
fn the_name_scan_stops_at_an_all_caps_standard_heading() {
    // "Acme Corp" is name-shaped; it sits AFTER PROFESSIONAL EXPERIENCE, so it must not be read.
    let v = view("jane@example.com\nPROFESSIONAL EXPERIENCE\nAcme Corp 2020 - 2022\n");
    assert_eq!(v.contact.name.status, AtsStatus::Missing);
}

#[test]
fn a_hinted_name_fused_to_a_lowercase_email_is_still_found() {
    // pdf-extract joins the name line and the next run: `Jane Doejane@example.com`.
    let hints = AtsHints {
        name: Some("Jane Doe"),
    };
    let v = ats_view_from_text("Jane Doejane@example.com | Berlin\nSKILLS\nRust\n", hints);
    assert_eq!(v.contact.name.status, AtsStatus::Found);
    // A single-word hint still needs a whole word.
    let one = AtsHints { name: Some("Li") };
    assert_eq!(
        ats_view_from_text("Lisa Marie\nSKILLS\nRust\n", one)
            .contact
            .name
            .status,
        AtsStatus::Missing
    );
}
