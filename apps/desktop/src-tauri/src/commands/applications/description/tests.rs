use super::draft_description;
use crate::autopilot::tests::support::{autopilot_fixture, found_job};
use crate::autopilot::Autopilot;

const AD: &str = "Build and run payment services in Rust. 5+ years of backend experience.";

fn records(url: &str, description: Option<&str>) -> Vec<Autopilot> {
    let mut job = found_job(url, 1);
    job.description = description.map(str::to_string);
    vec![Autopilot {
        found_jobs: vec![job],
        ..autopilot_fixture()
    }]
}

#[test]
fn resolves_the_description_from_the_persisted_found_job() {
    let r = records("https://example.com/jobs/1?utm=x", Some(AD));
    assert_eq!(
        draft_description(None, "https://www.example.com/jobs/1/", &r, ""),
        AD
    );
}

#[test]
fn blank_found_job_description_yields_empty() {
    let r = records("https://example.com/jobs/1", Some("   "));
    assert_eq!(
        draft_description(None, "https://example.com/jobs/1", &r, ""),
        ""
    );
}

#[test]
fn unknown_posting_yields_empty() {
    let r = records("https://example.com/jobs/1", Some(AD));
    assert_eq!(
        draft_description(None, "https://example.com/jobs/2", &r, ""),
        ""
    );
}

#[test]
fn request_text_wins_over_the_found_job() {
    let r = records("https://example.com/jobs/1", Some(AD));
    let got = draft_description(
        Some("typed ad".into()),
        "https://example.com/jobs/1",
        &r,
        "",
    );
    assert_eq!(got, "typed ad");
}

#[test]
fn existing_non_empty_description_is_never_replaced() {
    let r = records("https://example.com/jobs/1", Some(AD));
    assert_eq!(
        draft_description(None, "https://example.com/jobs/1", &r, "my edit"),
        ""
    );
}
