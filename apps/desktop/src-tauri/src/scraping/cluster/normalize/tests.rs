//! Fold/normalize-company/normalize-title unit tests.

use super::*;

// ── fold ────────────────────────────────────────────────────────────────

#[test]
fn fold_lowercases_and_expands_german_umlauts() {
    // Precomposed umlaut → ASCII digraph, so Müller ≡ Mueller.
    assert_eq!(fold("Müller"), "mueller");
    assert_eq!(fold("Mueller"), "mueller");
    assert_eq!(fold("Größe"), "groesse"); // ö→oe AND ß→ss
    assert_eq!(fold("Über"), "ueber");
}

#[test]
fn fold_strips_combining_marks_to_base_letter() {
    // Decomposed acute (e + U+0301) → base letter, no accent left behind.
    assert_eq!(fold("Cafe\u{0301}"), "cafe");
    // Precomposed accents map to their base letter too.
    assert_eq!(fold("Peña"), "pena");
    assert_eq!(fold("Škoda"), "skoda");
}

// ── normalize_company ─────────────────────────────────────────────────────

#[test]
fn company_strips_compound_legal_suffix() {
    assert_eq!(normalize_company("Acme GmbH & Co. KG"), "acme");
    assert_eq!(normalize_company("Acme GmbH"), "acme");
    assert_eq!(normalize_company("Acme, Inc."), "acme");
    assert_eq!(normalize_company("Beispiel Verein e.V."), "beispiel verein");
}

#[test]
fn company_strips_stacked_suffixes_iteratively() {
    // "ag" then "gmbh" both peel off, longest tail first.
    assert_eq!(normalize_company("Foo GmbH AG"), "foo");
}

#[test]
fn company_keeps_suffix_that_is_not_a_whole_token() {
    // "co" is a suffix, but "cisco" must not lose its tail.
    assert_eq!(normalize_company("Cisco"), "cisco");
    // "adecco" ends in "co" mid-token — kept whole.
    assert_eq!(normalize_company("Adecco"), "adecco");
}

// ── is_agency ─────────────────────────────────────────────────────────────

#[test]
fn agency_matches_builtin_names_tokens_and_extras() {
    // Exercise the production seam directly: normalize the extras once, then
    // check each company against the pre-normalized set.
    let check =
        |company: &str, extra: &[String]| is_agency_with(company, &normalize_agency_extras(extra));
    // Built-in company names.
    assert!(check("Hays", &[]));
    assert!(check("Michael Page", &[]));
    assert!(check("Randstad", &[]));
    // Token signal (German + English), even with a legal suffix present.
    assert!(check("Mustermann Personalberatung GmbH", &[]));
    assert!(check("Acme Recruiting", &[]));
    // User-supplied extra, normalized the same way.
    assert!(check(
        "Talent Partners AG",
        &["talent partners".to_string()]
    ));
    // A real employer is not an agency.
    assert!(!check("Acme", &[]));
    assert!(!check("", &[]));
}

// ── normalize_title: gender tags ─────────────────────────────────────────

#[test]
fn title_strips_all_gender_tag_variants() {
    for variant in [
        "Rust Developer (m/w/d)",
        "Rust Developer (w/m/d)",
        "Rust Developer (m/w/x)",
        "Rust Developer (d/m/w)",
        "Rust Developer (all genders)",
        "Rust Developer (gn)",
        "Rust Developer m/w/d",
    ] {
        assert_eq!(
            normalize_title(variant),
            "rust developer",
            "variant `{variant}` must fold to the bare title"
        );
    }
}

#[test]
fn title_keeps_a_role_qualifier_parenthetical() {
    // A parenthetical role qualifier is NOT a gender tag and NOT a remote
    // marker, so it is preserved — dropping it would merge distinct roles
    // ("(Backend)" vs "(Frontend)") at the same company.
    assert_eq!(
        normalize_title("Developer (Backend)"),
        "developer (backend)"
    );
    // But an explicit remote-marker parenthetical is stripped.
    assert_eq!(normalize_title("Developer (Remote)"), "developer");
}

// ── normalize_title: seniority + location ────────────────────────────────

#[test]
fn title_keeps_seniority_words() {
    assert_eq!(
        normalize_title("Senior Rust Developer"),
        "senior rust developer"
    );
    assert_eq!(
        normalize_title("Junior Rust Developer"),
        "junior rust developer"
    );
    // A seniority word in a trailing segment is never stripped.
    assert_eq!(
        normalize_title("Rust Developer - Senior Team"),
        "rust developer - senior team"
    );
}

#[test]
fn title_strips_trailing_location_and_remote() {
    assert_eq!(
        normalize_title("Senior Rust Developer (m/w/d) – Berlin"),
        "senior rust developer"
    );
    assert_eq!(
        normalize_title("Backend Engineer | Remote"),
        "backend engineer"
    );
    assert_eq!(normalize_title("Data Engineer (Remote)"), "data engineer");
    assert_eq!(
        normalize_title("Platform Engineer - Home Office"),
        "platform engineer"
    );
}

#[test]
fn title_first_token_drives_the_block() {
    assert_eq!(title_first_token("Senior Rust Developer (m/w/d)"), "senior");
    assert_eq!(title_first_token("Junior Rust Developer"), "junior");
    assert_eq!(title_first_token(""), "");
}
