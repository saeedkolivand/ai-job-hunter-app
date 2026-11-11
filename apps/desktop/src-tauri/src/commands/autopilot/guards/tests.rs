use super::*;

#[test]
fn date_filter_accepts_every_known_value_and_none() {
    assert!(check_date_filter(None).is_ok());
    for v in DATE_FILTER_OPTIONS {
        assert!(check_date_filter(Some(v)).is_ok(), "{v}");
    }
}

#[test]
fn date_filter_rejects_unknown_value_naming_allowed_ones() {
    let err = check_date_filter(Some("past_week"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("dateFilter") && err.contains("week") && err.contains("24h"));
}

#[test]
fn own_resume_wins_when_non_blank() {
    assert_eq!(
        effective_resume(Some("mine"), Some("doc")).as_deref(),
        Some("mine")
    );
}

#[test]
fn blank_or_absent_own_resume_falls_back_to_default_document() {
    assert_eq!(effective_resume(None, Some("doc")).as_deref(), Some("doc"));
    assert_eq!(
        effective_resume(Some("  \n"), Some("doc")).as_deref(),
        Some("doc")
    );
}

#[test]
fn no_resume_anywhere_is_none() {
    assert_eq!(effective_resume(Some(" "), Some("")), None);
    assert_eq!(effective_resume(None, None), None);
}
