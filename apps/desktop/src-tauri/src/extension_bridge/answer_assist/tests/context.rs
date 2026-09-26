//! `scraped_salary_range` — the matched Application's own scraped range.

use crate::salary_research::SalaryRange;

use super::super::context::scraped_salary_range;
use super::support::app_with_salary;

#[test]
fn scraped_salary_range_none_without_a_matched_application() {
    assert!(scraped_salary_range(None).is_none());
}

#[test]
fn scraped_salary_range_none_when_salary_unknown() {
    let a = app_with_salary(None, None, None);
    assert!(scraped_salary_range(Some(&a)).is_none());
}

#[test]
fn scraped_salary_range_converts_the_scraped_figures() {
    let a = app_with_salary(Some(65_000.0), Some(80_000.0), Some("EUR"));
    let range = scraped_salary_range(Some(&a)).expect("scraped range present");
    assert_eq!(
        range,
        SalaryRange {
            min: 65_000,
            max: 80_000,
            currency: "EUR".to_string()
        }
    );
}

#[test]
fn scraped_salary_range_defaults_currency_to_empty_when_unknown() {
    let a = app_with_salary(Some(1.0), Some(2.0), None);
    let range = scraped_salary_range(Some(&a)).expect("scraped range present");
    assert_eq!(range.currency, "");
}
