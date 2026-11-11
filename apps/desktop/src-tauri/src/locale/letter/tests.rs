use super::*;

#[test]
fn fixture_parses_and_covers_key_markets() {
    // de carries the DIN-5008 specifics; us is the only Letter-size market.
    let de = conventions("de");
    assert_eq!(de.country, "Germany");
    assert!(de.subject_line.used);
    assert_eq!(de.subject_line.label, "Betreff");
    assert_eq!(de.date_position, "top-right");
    assert!(de
        .inclusions
        .iter()
        .any(|i| i.contains("salary expectation")));

    assert_eq!(conventions("us").page, "letter");
    assert_eq!(conventions("uk").signoffs[0], "Yours sincerely");
}

#[test]
fn unknown_market_falls_back_to_intl() {
    assert_eq!(conventions("zz").country, "International");
    assert_eq!(conventions("").country, "International");
}

#[test]
fn detects_salutations_across_locales() {
    assert!(is_salutation("Dear Ms. Schmidt,"));
    assert!(is_salutation("Sehr geehrte Frau Müller,"));
    assert!(is_salutation("Madame, Monsieur,"));
    assert!(is_salutation("Estimado Sr. García:"));
    assert!(is_salutation("Gentile Dott. Rossi,"));
    assert!(is_salutation("採用ご担当者様"));
    assert!(is_salutation("홍길동님께,"));
    assert!(!is_salutation(
        "I led the migration of our payments service."
    ));
}

#[test]
fn detects_signoffs_across_locales() {
    assert!(is_signoff("Sincerely,"));
    assert!(is_signoff("Mit freundlichen Grüßen"));
    assert!(is_signoff("Cordialement,"));
    assert!(is_signoff("Distinti saluti,"));
    assert!(is_signoff("С уважением,"));
    assert!(!is_signoff("Best of all, I shipped it on time."));
}

#[test]
fn detects_subject_lines() {
    assert!(is_subject_line("Betreff: Bewerbung als Frontend Engineer"));
    assert!(is_subject_line("Re: Senior Frontend Engineer"));
    assert!(is_subject_line("Objet : Candidature"));
    assert!(is_subject_line("件名：エンジニア応募"));
    assert!(!is_subject_line(
        "Reference architecture I designed at Acme"
    ));
    assert!(!is_subject_line("Dear Hiring Manager,"));
}

#[test]
fn detects_template_placeholder_lines() {
    // Literal tokens, case-insensitive, trailing-punctuation tolerant.
    assert!(is_template_placeholder("Ihr Name"));
    assert!(is_template_placeholder("ihr name,"));
    assert!(is_template_placeholder("Your Name"));
    assert!(is_template_placeholder("Your Full Name."));
    assert!(is_template_placeholder("First Last"));
    // Whole-line bracket/slot syntax.
    assert!(is_template_placeholder("[Your Title]"));
    assert!(is_template_placeholder("<Name>"));
    assert!(is_template_placeholder("{{Name}}"));
    // Real content must never match.
    assert!(!is_template_placeholder("Senior Software Engineer"));
    assert!(!is_template_placeholder("Saeed Kolivand"));
    assert!(!is_template_placeholder(""));
    assert!(!is_template_placeholder(
        "I led the migration of our payments service."
    ));
}

#[test]
fn a_language_resolves_to_its_home_market() {
    assert_eq!(
        conventions_for_language("ja").unwrap().country,
        conventions("jp").country
    );
    assert_eq!(
        conventions_for_language("zh-CN").unwrap().country,
        conventions("cn").country
    );
    // pt is shared by pt and br; the market whose id is the code wins.
    assert_eq!(
        conventions_for_language("pt-BR").unwrap().country,
        conventions("pt").country
    );
    // en is shared by us/uk/intl; the neutral baseline wins.
    assert_eq!(
        conventions_for_language("en").unwrap().country,
        conventions("intl").country
    );
    assert!(conventions_for_language("xx").is_none());
}

#[test]
fn a_japanese_letter_on_the_us_market_gets_the_japanese_convention() {
    let c = conventions_for_letter("us", Some("ja"));
    assert_eq!(c.salutations.generic, "採用ご担当者様");
    // native language or unknown language keeps the market's own.
    assert_eq!(
        conventions_for_letter("de", Some("de-DE"))
            .subject_line
            .label,
        "Betreff"
    );
    assert_eq!(
        conventions_for_letter("de", None).subject_line.label,
        "Betreff"
    );
    assert_eq!(
        conventions_for_letter("de", Some("en")).subject_line.label,
        ""
    );
}
