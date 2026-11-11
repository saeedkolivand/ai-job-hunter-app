//! Tests for the `autopilot_list` summary (`reshape/autopilot_summary.rs`, #1495).

use super::super::reshape::*;
use super::super::*;
use crate::autopilot::{Autopilot, FoundJob};

/// The MCP server's own output cap (`mcp/results.rs`, 256 KiB). Restated, not imported: that
/// constant is private to the MCP module, and this test's premise is the number a caller hits.
const MCP_OUTPUT_CAP: usize = 256 * 1024;

fn found_job(i: usize) -> FoundJob {
    serde_json::from_value(json!({
        "title": format!("Backend Engineer {i}"),
        "company": "Acme",
        "url": format!("https://boards.example.com/jobs/{i}"),
        "description": "d".repeat(2_000),
        "foundAt": 1_700_000_000u64,
    }))
    .expect("a minimal FoundJob deserializes")
}

fn autopilot(i: usize, jobs: usize, resume: Option<&str>) -> Autopilot {
    serde_json::from_value(json!({
        "_id": format!("ap-{i}"),
        "name": format!("Autopilot {i}"),
        "status": "active",
        "target": { "boards": ["linkedin"], "query": "engineer", "pages": 1, "topN": 3 },
        "filter": { "minMatchScore": 0.0 },
        "schedule": "daily",
        "resumeText": resume,
        "totalFound": 1,
        "totalApplied": 0,
        "foundJobs": (0..jobs).map(|j| serde_json::to_value(found_job(j)).unwrap()).collect::<Vec<_>>(),
        "createdAt": 1u64,
        "updatedAt": 2u64,
    }))
    .expect("a minimal Autopilot deserializes")
}

fn list_reply(records: &[Autopilot]) -> Value {
    serde_json::to_value(records).expect("serializes")
}

#[test]
fn the_summary_swaps_both_heavy_fields_for_a_count_and_a_flag() {
    let mut data = list_reply(&[
        autopilot(1, 3, Some("Jane Doe, engineer")),
        autopilot(2, 0, Some("   ")),
        autopilot(3, 1, None),
    ]);
    // The premise, read off the real struct: these ARE the keys the wire carries.
    assert!(data[0].get("foundJobs").is_some() && data[0].get("resumeText").is_some());

    summarize_autopilot_list("autopilot_list", &mut data);

    for row in data.as_array().unwrap() {
        assert!(row.get("foundJobs").is_none() && row.get("resumeText").is_none());
    }
    assert_eq!(data[0]["foundJobsTotal"], 3);
    assert_eq!(data[0]["hasResumeText"], true);
    assert_eq!(data[1]["foundJobsTotal"], 0);
    assert_eq!(data[1]["hasResumeText"], false, "blank text is not set");
    assert_eq!(data[2]["hasResumeText"], false);
    assert_eq!(
        data[0]["name"], "Autopilot 1",
        "every other field is untouched"
    );
}

#[test]
fn only_autopilot_list_is_summarised() {
    let mut one = serde_json::to_value(autopilot(1, 2, Some("text"))).unwrap();
    let before = one.clone();
    summarize_autopilot_list("autopilot_get", &mut one);
    assert_eq!(one, before, "autopilot_get keeps its full record");
    let mut null = Value::Null;
    summarize_autopilot_list("autopilot_list", &mut null);
    assert_eq!(null, Value::Null);
}

/// The issue itself: a handful of autopilots with real-sized stored lists overflowed the MCP
/// output cap. Measured on the whole reshape pipeline, so it also covers the fence step.
#[test]
fn a_list_of_many_large_autopilots_fits_the_mcp_output_cap() {
    let resume = "r".repeat(60_000);
    let records: Vec<Autopilot> = (0..12).map(|i| autopilot(i, 150, Some(&resume))).collect();
    let raw = list_reply(&records);
    let raw_bytes = serde_json::to_string(&raw).unwrap().len();
    assert!(
        raw_bytes > MCP_OUTPUT_CAP,
        "fixture too small to show the problem ({raw_bytes} B)"
    );

    let out = reshape_reply("autopilot_list", raw, None);
    let out_bytes = serde_json::to_string(&out).unwrap().len();
    assert!(
        out_bytes < MCP_OUTPUT_CAP / 8,
        "summary is {out_bytes} B for 12 autopilots"
    );
    assert_eq!(out[0]["foundJobsTotal"], 150);
}
