//! Tests pinning the ONE order `reshape_reply` runs its steps in (`reshape.rs`).

use super::super::reshape::*;
use super::super::*;

/// Fencing MUST run before the page's byte budget is measured. Each row here
/// is far over `prompt_fence::JOB_CAP`, so fencing TRUNCATES it: fenced, all
/// five rows fit `LIST_PAGE_BYTE_BUDGET` comfortably; unfenced, only two do.
/// Swap steps 1 and 2 in `reshape_reply` and this drops to 2 items.
#[test]
fn reshape_reply_fences_before_it_measures_the_page_byte_budget() {
    let rows: Vec<Value> = (0..5)
        .map(|i| json!({ "id": i, "description": "x".repeat(60_000) }))
        .collect();
    // Measured, not assumed: unfenced, three of these rows already blow the
    // budget while two fit, so an unfenced measurement can only ever yield 2.
    let row_bytes = serde_json::to_string(&rows[0]).unwrap().len();
    assert!(
        2 * row_bytes < LIST_PAGE_BYTE_BUDGET && 3 * row_bytes > LIST_PAGE_BYTE_BUDGET,
        "the fixture no longer straddles the budget ({row_bytes} B/row)"
    );

    let out = reshape_reply("applications_list", Value::Array(rows), Some((0, 40)));

    let items = out["items"].as_array().expect("a paged envelope");
    assert_eq!(
        items.len(),
        5,
        "the budget measured unfenced bytes — fencing truncates each row to \
         prompt_fence::JOB_CAP, so all five fit the bytes actually shipped"
    );
    assert_eq!(out["total"], 5);
    assert!(out["nextCursor"].is_null());
    assert!(
        items[0]["description"]
            .as_str()
            .unwrap()
            .starts_with("<job_posting>"),
        "the rows that shipped must be the fenced ones"
    );
}

/// Step 3 runs last, so it sees whatever paging produced and writes its key at
/// the top level of THAT value. With today's audited lists no payload can
/// observe the step-2-vs-3 order (no command appears in both), which is why
/// the disjointness itself is asserted: the day it stops holding, this fires
/// and a real ordering assertion becomes possible AND necessary.
#[test]
fn reshape_reply_base64_encodes_last_and_the_two_reshape_lists_stay_disjoint() {
    for (command, _) in BASE64_BYTE_FIELDS {
        assert!(
            !PAGINATED_LIST_COMMANDS.contains(command),
            "`{command}` is now both paged and base64-encoded — reshape_reply's step 2/3 \
             order just became observable and needs its own assertion"
        );
    }

    let out = reshape_reply(
        "documents_export_document",
        json!({ "data": [1, 2, 3] }),
        None,
    );
    assert_eq!(out["data"], "AQID");
    assert_eq!(out["dataEncoding"], "base64");

    // A command in neither list is fenced and otherwise untouched: no
    // envelope, no marker key.
    let out = reshape_reply("jobs_list", json!({ "id": "j-1" }), None);
    assert_eq!(out, json!({ "id": "j-1" }));
}

// ── contact_profile_get projection (issue #1180) ───────────────────────────
