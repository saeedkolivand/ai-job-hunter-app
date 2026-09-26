//! Round-4 advisory-finding tests: retroactive board-remote, undecided-remote exclusion,
//! applied-store-absent envelope marker, cross-host applied identity, and infallible projection.

use super::super::super::job::project_value;
use super::super::*;
use super::support::*;
use crate::autopilot::FoundJob;
use std::collections::HashSet;

/// T1 — `board_remote` is only ever WRITTEN by `build_found_job` at
/// find-time, so a `FoundJob` persisted before that field existed (or from a
/// board that only started setting it later) deserializes with
/// `boardRemote: false` (`#[serde(default)]`) even when its `board` is one
/// of the all-remote feeds. The `remote` filter must still recognize it,
/// derived RETROACTIVELY off the stored `board` id through
/// `crate::scraping::boards::is_all_remote_board` — the SAME registry
/// `Scraper::is_all_remote` declares — not just the stored bit.
#[test]
fn found_jobs_remote_filter_derives_board_remote_retroactively_from_the_board_registry() {
    let legacy_json = {
        let mut v = serde_json::to_value(FoundJob {
            board: Some("wwr".to_string()),
            board_remote: true,
            location: None,
            ..numbered_job(1)
        })
        .unwrap();
        // Simulate a record written before `boardRemote` existed at all.
        v.as_object_mut().unwrap().remove("boardRemote");
        v
    };
    let legacy: FoundJob = serde_json::from_value(legacy_json).unwrap();
    assert!(
        !legacy.board_remote,
        "boardRemote must default false when the key is absent from the stored JSON"
    );

    let records = vec![autopilot_with_jobs("ap-1", vec![legacy])];
    let remote_only = FoundJobsFilters::from_payload(&json!({ "remote": true })).unwrap();
    let out =
        resolve_found_jobs(&records, Some("ap-1"), &remote_only, &no_applied(), 0, 20).unwrap();
    assert_eq!(
        out["total"], 1,
        "an all-remote board's pre-fix record must still pass --remote true"
    );
}

/// T2 — a row with no location text, a board that is NOT all-remote, and no
/// `board_remote` bit is genuinely UNDECIDED, not a confident "not remote".
/// It must match neither `remote: true` nor `remote: false`.
#[test]
fn found_jobs_remote_filter_excludes_an_undecided_row_from_both_directions() {
    let undecided = FoundJob {
        location: None,
        board: Some("adzuna".to_string()),
        board_remote: false,
        ..numbered_job(1)
    };
    let onsite = FoundJob {
        location: Some("Berlin, Germany".to_string()),
        board_remote: false,
        ..numbered_job(2)
    };
    let onsite_url = onsite.url.clone();

    let records = vec![autopilot_with_jobs(
        "ap-1",
        vec![undecided.clone(), onsite.clone()],
    )];
    let remote_only = FoundJobsFilters::from_payload(&json!({ "remote": true })).unwrap();
    let out =
        resolve_found_jobs(&records, Some("ap-1"), &remote_only, &no_applied(), 0, 20).unwrap();
    assert_eq!(
        out["total"], 0,
        "an undecided row must not match remote: true"
    );

    let records = vec![autopilot_with_jobs("ap-1", vec![undecided, onsite])];
    let onsite_only = FoundJobsFilters::from_payload(&json!({ "remote": false })).unwrap();
    let out =
        resolve_found_jobs(&records, Some("ap-1"), &onsite_only, &no_applied(), 0, 20).unwrap();
    assert_eq!(
        out["total"], 1,
        "an undecided row must not match remote: false either"
    );
    assert_eq!(out["jobs"][0]["url"], onsite_url);
}

/// T3 — when the applications store is unavailable, every row must OMIT its
/// `applied` key (never a confident `false`) and the envelope must carry
/// `appliedUnavailable: true`. Store present stays byte-for-byte unchanged.
#[test]
fn found_jobs_omits_applied_key_and_flags_the_envelope_when_the_store_is_absent() {
    let records = vec![autopilot_with_jobs("ap-1", vec![numbered_job(1)])];

    let absent = resolve_found_jobs_for_store(
        &records,
        Some("ap-1"),
        &no_filters(),
        &no_applied(),
        0,
        20,
        false,
    )
    .unwrap();
    assert!(
        absent["jobs"][0]
            .as_object()
            .unwrap()
            .get("applied")
            .is_none(),
        "applied must be ABSENT, not false, when the store is unavailable"
    );
    assert_eq!(absent["appliedUnavailable"], true);

    let present = resolve_found_jobs_for_store(
        &records,
        Some("ap-1"),
        &no_filters(),
        &no_applied(),
        0,
        20,
        true,
    )
    .unwrap();
    assert_eq!(present["jobs"][0]["applied"], false);
    assert!(present
        .as_object()
        .unwrap()
        .get("appliedUnavailable")
        .is_none());
}

/// T4 — `applied` must be derived by IDENTITY, not a byte-exact normalized
/// string compare: a job stored under a regional LinkedIn host must still
/// read as applied when the recorded application normalized to the bare
/// `linkedin.com` host for the SAME numeric id.
#[test]
fn found_jobs_applied_matches_by_identity_across_a_regional_linkedin_host() {
    let job = FoundJob {
        url: "https://de.linkedin.com/jobs/view/4185657072".to_string(),
        ..numbered_job(1)
    };
    let records = vec![autopilot_with_jobs("ap-1", vec![job])];
    let mut applied_urls = HashSet::new();
    applied_urls.insert(crate::applications::normalize_job_url(
        "https://www.linkedin.com/jobs/view/4185657072",
    ));
    let out =
        resolve_found_jobs(&records, Some("ap-1"), &no_filters(), &applied_urls, 0, 20).unwrap();
    assert_eq!(out["jobs"][0]["applied"], true);
}

/// T5-cont (PR #1182 round-5) — `project_found_job_row`'s fallback branch
/// (`unwrap_or_else` + `debug_assert!`, replacing a prior `.expect()` that
/// would abort the whole process in a release build on a future field-type
/// drift) is unreachable for any real `FoundJob` today, but `project_value`
/// itself genuinely can return `None` for a shape that doesn't satisfy
/// `FoundJobSlice`'s required fields — this pins that the primitive the
/// fallback guards against is real, not dead code by construction.
#[test]
fn project_value_returns_none_for_a_shape_missing_a_required_found_job_slice_field() {
    assert!(project_value::<Value, FoundJobSlice>(&json!({})).is_none());
}
