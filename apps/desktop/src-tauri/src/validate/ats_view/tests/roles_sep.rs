//! A mid-line `·` is a separator, never a bullet; a line-start `·` still is.

use super::super::{ats_view_from_text, AtsField, AtsHints, AtsRole, AtsStatus};

fn roles(body: &str) -> Vec<AtsRole> {
    let text = format!(
        "Jane Doe
WORK EXPERIENCE
{body}
EDUCATION
State University 2013 - 2017
"
    );
    ats_view_from_text(&text, AtsHints::default()).roles
}

fn val(f: &AtsField) -> Option<&str> {
    f.value.as_deref()
}

#[test]
fn title_company_dates_with_middots_is_one_role() {
    let r = roles("Senior Engineer · Acme Corp · Jan 2020 – Present");
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(val(&r[0].title), Some("Senior Engineer"));
    assert_eq!(val(&r[0].company), Some("Acme Corp"));
    assert_eq!(val(&r[0].start), Some("2020-01"));
    assert_eq!(val(&r[0].end), Some("present"));
    assert_eq!(r[0].start.status, AtsStatus::Found);
}

#[test]
fn aria_form_title_and_dates_is_found() {
    let r = roles("Senior Engineer · 2020 – 2022");
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(val(&r[0].start), Some("2020"));
    assert_eq!(val(&r[0].end), Some("2022"));
}

#[test]
fn line_start_middot_bullets_do_not_open_roles() {
    let r = roles(
        "Acme Corp  Mar 2021 - Present
Senior Engineer
· Led 2019 – 2021 migration
· Built billing",
    );
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(val(&r[0].title), Some("Senior Engineer"));
}

#[test]
fn a_name_ending_in_a_month_prefix_is_not_split_into_a_date() {
    let r = roles("Omar 2020 - 2021");
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(val(&r[0].company), Some("Omar"));
    assert_eq!(val(&r[0].start), Some("2020"));
}
