use super::*;

#[test]
fn completes_a_body_only_de_letter() {
    let body = "Ich schreibe Ihnen, um mein Interesse an der Stelle als \
                 Softwareentwickler auszudrücken.";
    let out = complete_letter_text(body, "de", "Max Müller");
    assert!(
        out.starts_with("Sehr geehrte Damen und Herren,\n\n"),
        "got: {out:?}"
    );
    assert!(out.contains(body));
    assert!(
        out.trim_end()
            .ends_with("Mit freundlichen Grüßen\nMax Müller"),
        "got: {out:?}"
    );
}

#[test]
fn completes_a_body_only_us_letter_with_the_english_pair() {
    let body = "I am writing to express my interest in the Software Engineer role.";
    let out = complete_letter_text(body, "us", "Jane Smith");
    assert!(out.starts_with("Dear Hiring Manager,\n\n"), "got: {out:?}");
    assert!(out.contains(body));
    assert!(
        out.trim_end().ends_with("Sincerely,\nJane Smith"),
        "got: {out:?}"
    );
}

/// The double-add guard: a letter that already carries its own salutation
/// and sign-off (the shape the TS fast-path prompt always emits) must
/// round-trip byte-for-byte — AIGeneratePage still produces full letters
/// through this same export path.
#[test]
fn is_a_noop_on_an_already_complete_letter() {
    let complete =
        "Dear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\nJane Smith\n";
    assert_eq!(complete_letter_text(complete, "us", "Jane Smith"), complete);
}

#[test]
fn blank_input_is_returned_unchanged() {
    assert_eq!(complete_letter_text("", "us", "Jane Smith"), "");
    assert_eq!(complete_letter_text("   \n\t ", "de", "Max"), "   \n\t ");
}

/// No candidate name known: the sign-off is still added, but with no
/// dangling blank name line under it.
#[test]
fn skips_the_name_line_when_the_name_is_blank() {
    let out = complete_letter_text("Body text here.", "us", "");
    assert!(out.trim_end().ends_with("Sincerely,"), "got: {out:?}");
}

/// Only the missing half is added when the letter already has one of the
/// two — the existing salutation/sign-off must not be duplicated.
#[test]
fn adds_only_the_missing_half() {
    let has_salutation_only = "Dear Hiring Manager,\n\nBody text here.";
    let out = complete_letter_text(has_salutation_only, "us", "Jane Smith");
    assert_eq!(out.matches("Dear Hiring Manager,").count(), 1);
    assert!(
        out.trim_end().ends_with("Sincerely,\nJane Smith"),
        "got: {out:?}"
    );

    let has_signoff_only = "Body text here.\n\nSincerely,\nJane Smith";
    let out2 = complete_letter_text(has_signoff_only, "us", "Jane Smith");
    assert_eq!(out2.matches("Sincerely,").count(), 1);
    assert!(
        out2.starts_with("Dear Hiring Manager,\n\n"),
        "got: {out2:?}"
    );
}

// ── salutation-placement regression: `14bd60c3` taught `letter_system` to
//    have the model open a market letter with a subject line and/or a
//    date BEFORE the body; the salutation must land AFTER that furniture,
//    not at line 0, or it becomes unreachable pre-salutation furniture for
//    `parse_cover_letter` (see that module's own end-to-end tests). ────

#[test]
fn inserts_the_salutation_after_a_leading_subject_line() {
    let body = "Betreff: Bewerbung als Software Engineer\n\n\
                 Ich bringe sechs Jahre Erfahrung mit.";
    let out = complete_letter_text(body, "de", "Max Müller");
    assert!(
        out.starts_with(
            "Betreff: Bewerbung als Software Engineer\n\nSehr geehrte Damen und Herren,\n\n"
        ),
        "the subject line must stay ahead of the salutation: {out:?}"
    );
    assert!(out.contains("Ich bringe sechs Jahre Erfahrung mit."));
}

#[test]
fn inserts_the_salutation_after_a_leading_date_line() {
    let body = "Frankfurt, 12. Januar 2025\n\nIch bringe sechs Jahre Erfahrung mit.";
    let out = complete_letter_text(body, "de", "Max Müller");
    assert!(
        out.starts_with("Frankfurt, 12. Januar 2025\n\nSehr geehrte Damen und Herren,\n\n"),
        "the date line must stay ahead of the salutation: {out:?}"
    );
}

#[test]
fn inserts_the_salutation_after_subject_and_date_in_either_order() {
    let subject_then_date = "Betreff: Bewerbung als Software Engineer\n\
                              \n12. Januar 2025\n\nIch bringe Erfahrung mit.";
    let out = complete_letter_text(subject_then_date, "de", "Max Müller");
    assert!(
        out.starts_with(
            "Betreff: Bewerbung als Software Engineer\n\n12. Januar 2025\n\n\
             Sehr geehrte Damen und Herren,\n\n"
        ),
        "got: {out:?}"
    );

    let date_then_subject = "12. Januar 2025\n\nBetreff: Bewerbung als Software Engineer\n\n\
                              Ich bringe Erfahrung mit.";
    let out2 = complete_letter_text(date_then_subject, "de", "Max Müller");
    assert!(
        out2.starts_with(
            "12. Januar 2025\n\nBetreff: Bewerbung als Software Engineer\n\n\
             Sehr geehrte Damen und Herren,\n\n"
        ),
        "got: {out2:?}"
    );
}

/// `us` never has the model write a subject line, so a body with no
/// leading furniture must still get the salutation at the very top —
/// the furniture skip must never swallow real body prose.
#[test]
fn a_market_with_no_subject_line_convention_is_unaffected() {
    let body = "I am writing to express my interest in the Software Engineer role.";
    let out = complete_letter_text(body, "us", "Jane Smith");
    assert!(out.starts_with("Dear Hiring Manager,\n\n"), "got: {out:?}");
}

/// The same request is re-validated on every preview render AND every
/// export — running the completion twice (furniture-and-all) must equal
/// running it once.
#[test]
fn is_idempotent_with_leading_furniture() {
    let body = "Betreff: Bewerbung als Software Engineer\n\n12. Januar 2025\n\n\
                 Ich bringe sechs Jahre Erfahrung mit.";
    let once = complete_letter_text(body, "de", "Max Müller");
    let twice = complete_letter_text(&once, "de", "Max Müller");
    assert_eq!(once, twice, "once: {once:?}\ntwice: {twice:?}");
}
