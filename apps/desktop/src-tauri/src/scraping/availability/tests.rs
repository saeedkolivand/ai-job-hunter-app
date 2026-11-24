use super::{classify, classify_read, Availability as A, AvailabilityEvidence as E};

const OPEN_PAGE: &str = "<html><h1>Senior Rust Engineer</h1><button>Apply now</button></html>";
const CLOSED_LINKEDIN: &str =
    "<html><h1>Engineer</h1><span>No longer accepting applications</span></html>";
const CLOSED_DE: &str =
    "<p>Diese Stelle ist leider nicht da.</p><p>Stelle ist nicht mehr verfügbar</p>";
const LOGIN_WALL: &str = "<html>Sign in to view this job. This job is no longer available?</html>";

#[test]
fn open_page_is_open() {
    let v = classify(Some(200), OPEN_PAGE);
    assert_eq!((v.availability, v.evidence), (A::Open, E::Ok));
}

#[test]
fn closed_markers_in_english_and_german_are_closed() {
    for body in [CLOSED_LINKEDIN, CLOSED_DE] {
        let v = classify(Some(200), body);
        assert_eq!((v.availability, v.evidence), (A::Closed, E::ClosedMarker));
    }
}

#[test]
fn http_404_and_410_are_removed() {
    for s in [404, 410] {
        let v = classify(Some(s), "");
        assert_eq!((v.availability, v.evidence), (A::Removed, E::HttpGone));
    }
}

#[test]
fn blocked_statuses_are_unknown_never_closed() {
    for s in [401, 403, 429, 999] {
        let v = classify(Some(s), CLOSED_LINKEDIN);
        assert_eq!((v.availability, v.evidence), (A::Unknown, E::Blocked));
    }
}

#[test]
fn login_wall_with_stray_closed_text_stays_unknown() {
    assert_eq!(classify(Some(200), LOGIN_WALL).availability, A::Unknown);
}

#[test]
fn timeout_and_server_errors_are_unknown() {
    assert_eq!(classify(None, "").availability, A::Unknown);
    assert_eq!(classify(Some(503), "").availability, A::Unknown);
    assert_eq!(classify(Some(302), "").availability, A::Unknown);
}

#[test]
fn closure_string_inside_script_or_style_is_not_page_text() {
    let live = "<html><script>var i18n={\"expired\":\"This job has expired\"};</script>\
        <STYLE>/* no longer accepting applications */</STYLE>\
        <template><p>This position has been filled</p></template>\
        <h1>Engineer</h1><button>Apply</button></html>";
    assert_eq!(classify(Some(200), live).availability, A::Open);
    let closed_with_script = format!("<script>x</script>{CLOSED_LINKEDIN}");
    assert_eq!(
        classify(Some(200), &closed_with_script).availability,
        A::Closed
    );
    // An unterminated script swallows the rest rather than panicking.
    assert_eq!(
        classify(Some(200), "<script>this job has expired").availability,
        A::Open
    );
}

#[test]
fn size_limited_read_is_open_only_when_complete() {
    assert_eq!(
        classify_read(Some(200), OPEN_PAGE, false).availability,
        A::Unknown
    );
    assert_eq!(
        classify_read(Some(200), OPEN_PAGE, true).availability,
        A::Open
    );
    let v = classify_read(Some(200), CLOSED_LINKEDIN, false);
    assert_eq!((v.availability, v.evidence), (A::Closed, E::ClosedMarker));
}

#[test]
fn only_closed_and_removed_are_dead() {
    assert!(A::Closed.is_dead() && A::Removed.is_dead());
    assert!(!A::Open.is_dead() && !A::Unknown.is_dead());
}
