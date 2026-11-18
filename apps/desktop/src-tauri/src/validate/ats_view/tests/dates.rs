//! Localized date-range parsing, plus how it surfaces on a role.

use super::super::dates::{end_value, find_range, point_value, End};
use super::super::{ats_view_from_text, AtsHints, AtsReason, AtsRole, AtsStatus};

fn range(s: &str) -> (Option<String>, Option<String>) {
    let r = find_range(s).unwrap_or_else(|| panic!("no range in {s:?}"));
    (r.from.map(point_value), r.to.map(end_value))
}

fn pair(a: &str, b: &str) -> (Option<String>, Option<String>) {
    (Some(a.to_string()), Some(b.to_string()))
}

#[test]
fn english_and_german_forms_parse() {
    assert_eq!(range("Mar 2021 – Present"), pair("2021-03", "present"));
    assert_eq!(range("March 2019 - June 2021"), pair("2019-03", "2021-06"));
    assert_eq!(range("03/2021 - 02/2023"), pair("2021-03", "2023-02"));
    assert_eq!(range("3.2018 – 12.2020"), pair("2018-03", "2020-12"));
    assert_eq!(range("März 2021 – heute"), pair("2021-03", "present"));
    assert_eq!(range("Okt. 2015 bis Dez. 2018"), pair("2015-10", "2018-12"));
    assert_eq!(range("2021–heute"), pair("2021", "present"));
    assert_eq!(range("2013 - 2017"), pair("2013", "2017"));
    assert_eq!(range("2021-03 – 2022-05"), pair("2021-03", "2022-05"));
    assert_eq!(range("seit 03/2021"), pair("2021-03", "present"));
    assert_eq!(range("Since 2020"), pair("2020", "present"));
}

/// A point inside a day-first date is not its own date, and an invalid month is never
/// silently re-read as a bare year.
#[test]
fn day_first_dates_are_one_token() {
    assert_eq!(range("01.03.2021 – 31.12.2022"), pair("2021-03", "2022-12"));
    assert_eq!(range("01/03/2021 - 05.12.2022"), pair("2021-03", "2022-12"));
    let line = "Acme 01.03.2021 – 31.12.2022";
    assert_eq!(
        &line[find_range(line).unwrap().span],
        "01.03.2021 – 31.12.2022"
    );
    assert_eq!(range("13/2021 - 14/2021"), (None, None));
    assert_eq!(
        range("45.13.2021 - 05.12.2022"),
        (None, Some("2022-12".into()))
    );
}

#[test]
fn span_excludes_the_surrounding_text() {
    let line = "Acme Corp Mar 2021 - Present";
    let r = find_range(line).unwrap();
    assert_eq!(&line[r.span], "Mar 2021 - Present");
}

#[test]
fn date_shaped_but_unparseable_is_reported_not_dropped() {
    assert_eq!(range("Jan '21 – Mar '22"), (None, None));
    assert_eq!(range("03/21 - 05/22"), (None, None));
    // A real range with one bad side keeps the good side.
    assert_eq!(range("2019 - Mar '22"), (Some("2019".into()), None));
}

#[test]
fn prose_and_non_dates_are_not_ranges() {
    for s in [
        "Marketing 2021 campaign",
        "Led 5-7 engineers",
        "Reduced cost 20% - 30%",
    ] {
        assert!(find_range(s).is_none(), "{s:?} must not be a range");
    }
}

#[test]
fn present_marker_is_an_open_end() {
    assert_eq!(find_range("2020 - Current").unwrap().to, Some(End::Present));
}

fn role_view(role_line: &str) -> AtsRole {
    let text = format!("Jane Doe\nWORK EXPERIENCE\n{role_line}\nSenior Engineer\n");
    ats_view_from_text(&text, AtsHints::default())
        .roles
        .remove(0)
}

#[test]
fn unparseable_role_dates_are_suspicious() {
    let r = role_view("Acme Corp Jan '21 – Mar '22");
    assert_eq!(r.start.status, AtsStatus::Suspicious);
    assert_eq!(r.start.reason, Some(AtsReason::Unparseable));
    assert_eq!(r.start.value.as_deref(), Some("Jan '21"));
    assert_eq!(r.end.status, AtsStatus::Suspicious);
}

#[test]
fn inverted_range_flags_the_end() {
    let r = role_view("Acme Corp 2022 - 2019");
    assert_eq!(r.start.status, AtsStatus::Found);
    assert_eq!(r.end.reason, Some(AtsReason::DatesInverted));
}

#[test]
fn german_role_line_parses_end_to_end() {
    let r = role_view("Beispiel GmbH  März 2021 – heute");
    assert_eq!(r.company.value.as_deref(), Some("Beispiel GmbH"));
    assert_eq!(r.title.value.as_deref(), Some("Senior Engineer"));
    assert_eq!(r.start.value.as_deref(), Some("2021-03"));
    assert_eq!(r.end.value.as_deref(), Some("present"));
}

#[test]
fn pipe_form_splits_title_and_company() {
    let text = "Jane Doe\nWORK EXPERIENCE\nSenior Engineer | Acme Corp | 2018 - 2021\n";
    let r = ats_view_from_text(text, AtsHints::default())
        .roles
        .remove(0);
    assert_eq!(r.title.value.as_deref(), Some("Senior Engineer"));
    assert_eq!(r.company.value.as_deref(), Some("Acme Corp"));
}

#[test]
fn bullet_dates_are_not_role_anchors() {
    let text = "Jane Doe\nWORK EXPERIENCE\n• Cut costs between 2019 - 2021\n";
    assert!(ats_view_from_text(text, AtsHints::default())
        .roles
        .is_empty());
}

#[test]
fn month_less_end_in_the_same_year_is_not_inverted() {
    // `Mar 2021 – 2021`: the end has no month, so only the years are compared.
    let r = role_view("Acme Corp Mar 2021 – 2021");
    assert_eq!(r.end.status, AtsStatus::Found);
    assert_eq!(r.end.value.as_deref(), Some("2021"));
    // Both months present and reversed in one year IS inverted.
    let r = role_view("Acme Corp Mar 2021 – Jan 2021");
    assert_eq!(r.end.reason, Some(AtsReason::DatesInverted));
}

#[test]
fn two_digit_end_year_is_unparseable_not_dropped() {
    let r = role_view("Acme Corp 2021 – 22");
    assert_eq!(r.start.value.as_deref(), Some("2021"));
    assert_eq!(r.end.status, AtsStatus::Suspicious);
    assert_eq!(r.end.reason, Some(AtsReason::Unparseable));
    assert_eq!(r.end.value.as_deref(), Some("22"));
}

#[test]
fn wrapped_bullet_continuations_do_not_open_phantom_roles() {
    let text = "Jane Doe\nWORK EXPERIENCE\nAcme Corp 2020 - 2022\nSenior Engineer\n\
        Reduced infrastructure cost across the whole platform migration programme between 2019 - 2021 for every region\n\
        in 2019 - 2021 for Acme.\n\
        seit 2020 haben wir die Plattform für alle Regionen und Teams vollständig umgebaut\n\
        from 2019 we migrated everything to the new platform\n";
    let roles = ats_view_from_text(text, AtsHints::default()).roles;
    assert_eq!(roles.len(), 1, "{roles:?}");
    assert_eq!(roles[0].company.value.as_deref(), Some("Acme Corp"));
}

#[test]
fn short_open_ended_role_lines_still_anchor() {
    let text = "Jane Doe\nWORK EXPERIENCE\nAcme Corp seit 03/2020\nSenior Engineer\n";
    let r = ats_view_from_text(text, AtsHints::default()).roles;
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].end.value.as_deref(), Some("present"));
}

#[test]
fn an_impossible_month_is_unparseable_not_silently_ignored() {
    // Month 13/14 never parses as a month. The range still anchors (the role is not
    // dropped) and BOTH sides are unparseable: never re-read as a bare year.
    assert_eq!(range("13/2021 - 14/2021"), (None, None));
}

// ── the generator's other locales (es, fr, it, pt, nl) ───────────────────

#[test]
fn spanish_french_italian_portuguese_dutch_ranges_parse() {
    // The generator's own `date_example`s, plus the present words each locale uses.
    assert_eq!(range("enero 2021 – marzo 2023"), pair("2021-01", "2023-03"));
    assert_eq!(range("Enero 2020 – Marzo 2022"), pair("2020-01", "2022-03"));
    assert_eq!(range("2020 – Actualidad"), pair("2020", "present"));
    assert_eq!(
        range("sept. 2019 – actualmente"),
        pair("2019-09", "present")
    );
    assert_eq!(
        range("janvier 2021 – mars 2023"),
        pair("2021-01", "2023-03")
    );
    assert_eq!(
        range("Janvier 2020 – Aujourd'hui"),
        pair("2020-01", "present")
    );
    assert_eq!(
        range("févr. 2019 – Aujourd’hui"),
        pair("2019-02", "present")
    );
    assert_eq!(
        range("août 2018 – décembre 2020"),
        pair("2018-08", "2020-12")
    );
    assert_eq!(
        range("gennaio 2021 – marzo 2023"),
        pair("2021-01", "2023-03")
    );
    assert_eq!(range("Gennaio 2020 - Presente"), pair("2020-01", "present"));
    assert_eq!(range("2019 – oggi"), pair("2019", "present"));
    assert_eq!(
        range("janeiro 2021 – março 2023"),
        pair("2021-01", "2023-03")
    );
    assert_eq!(range("set. 2018 – atual"), pair("2018-09", "present"));
    assert_eq!(range("2017 – atualmente"), pair("2017", "present"));
    assert_eq!(range("03/2020 – Presente"), pair("2020-03", "present"));
    assert_eq!(
        range("januari 2021 – maart 2023"),
        pair("2021-01", "2023-03")
    );
    assert_eq!(range("mrt 2019 – heden"), pair("2019-03", "present"));
}

#[test]
fn open_ended_openers_in_the_other_locales() {
    assert_eq!(range("desde marzo 2020"), pair("2020-03", "present"));
    assert_eq!(range("depuis 2019"), pair("2019", "present"));
    assert_eq!(range("dal 2018"), pair("2018", "present"));
}

#[test]
fn every_locale_gets_roles_not_silence() {
    for (line, from, to) in [
        ("Acme SL Enero 2020 – Marzo 2022", "2020-01", "2022-03"),
        ("Acme SARL Janvier 2020 – Aujourd'hui", "2020-01", "present"),
        ("Acme Srl Gennaio 2020 - Presente", "2020-01", "present"),
        ("Acme Lda março 2020 – atualmente", "2020-03", "present"),
        ("Acme BV januari 2020 – heden", "2020-01", "present"),
        ("Acme SA 03/2020 – Presente", "2020-03", "present"),
        ("Acme SA 2020 – Actualidad", "2020", "present"),
    ] {
        let r = role_view(line);
        assert_eq!(r.start.value.as_deref(), Some(from), "{line}");
        assert_eq!(r.end.value.as_deref(), Some(to), "{line}");
        assert_eq!(r.start.status, AtsStatus::Found, "{line}");
        assert!(
            r.company.value.as_deref().unwrap().starts_with("Acme"),
            "{line}"
        );
    }
}

#[test]
fn month_words_are_not_read_inside_ordinary_words() {
    // `mars` inside "Marsden", `set` inside "settings", `out` inside "outage".
    for s in [
        "Marsden 2021 – outage",
        "settings 2021 – outage",
        "Outage 2021 – presenting",
    ] {
        assert!(find_range(s).is_none(), "{s:?}");
    }
}
