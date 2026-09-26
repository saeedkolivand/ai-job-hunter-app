//! Tests for dropping a dead reply field (`reshape/drop_fields.rs`).

use super::super::super::agent_cli::policy::Effect;
use super::super::policy_lookup::find_policy;
use super::super::reshape::*;
use super::super::*;

/// The audited const pinned against a hand-written literal, same reasoning
/// as [`the_base64_byte_fields_are_exactly_this_one_audited_pair`], plus
/// both rows' own policy check — `autopilot_list`/`autopilot_get` must stay
/// `Effect::Read` (the raw, unprojected dispatch this reshape step exists to
/// cover) for this to be reachable at all.
#[test]
fn the_drop_fields_are_exactly_these_two_audited_pairs() {
    assert_eq!(
        DROP_FIELDS,
        &[
            ("autopilot_list", "totalApplied"),
            ("autopilot_get", "totalApplied"),
        ]
    );
    for (command, _) in DROP_FIELDS {
        let entry = find_policy("autopilot", command)
            .unwrap_or_else(|| panic!("{command} is a real POLICY row"));
        assert_eq!(entry.effect, Effect::Read);
    }
}

/// The FIELD half of the audited pair, pinned against the struct it was
/// audited against rather than a second copy of the literal — same
/// reasoning as
/// [`the_audited_field_is_the_key_the_real_export_struct_serializes_its_bytes_under`].
/// Renaming `Autopilot.total_applied` (or dropping `#[serde(rename_all)]`)
/// makes this fail instead of `DROP_FIELDS` silently pointing at a key no
/// reply carries.
#[test]
fn the_audited_field_is_the_key_the_real_autopilot_struct_serializes_its_dead_counter_under() {
    use crate::autopilot::{Autopilot, AutopilotFilter, AutopilotStatus, AutopilotTarget};

    let ap = Autopilot {
        id: "ap-1".into(),
        name: "Test AP".into(),
        status: AutopilotStatus::Active,
        target: AutopilotTarget {
            boards: vec!["linkedin".into()],
            query: "engineer".into(),
            location: None,
            country_code: None,
            work_types: None,
            pages: 1,
            date_filter: None,
            top_n: 3,
            watched_companies_only: None,
        },
        filter: AutopilotFilter {
            min_match_score: 0.0,
            keywords: None,
            exclude_keywords: None,
        },
        schedule: "daily".into(),
        schedule_hour: None,
        schedule_minute: None,
        resume_text: None,
        cover_letter: None,
        assistant: false,
        assistant_provider: None,
        assistant_model: None,
        assistant_base_url: None,
        total_found: 0,
        total_applied: 0,
        found_jobs: Vec::new(),
        run_status: None,
        last_run_summaries: Vec::new(),
        last_run_at: None,
        created_at: 0,
        updated_at: 0,
    };
    let value = serde_json::to_value(ap).expect("Autopilot serializes");
    assert!(
        value.get("totalApplied").is_some(),
        "`totalApplied` is no longer a key of Autopilot's wire shape — \
         DROP_FIELDS now points at nothing: {value}"
    );
}

#[test]
fn drop_dead_fields_strips_total_applied_from_a_single_autopilot_object() {
    let mut data = json!({ "id": "ap-1", "totalApplied": 0, "totalFound": 3 });
    drop_dead_fields("autopilot_get", &mut data);
    assert!(data.get("totalApplied").is_none());
    assert_eq!(data["totalFound"], json!(3));
}

#[test]
fn drop_dead_fields_strips_total_applied_from_every_row_of_an_autopilot_list() {
    let mut data = json!([
        { "id": "ap-1", "totalApplied": 0 },
        { "id": "ap-2", "totalApplied": 0 },
    ]);
    drop_dead_fields("autopilot_list", &mut data);
    assert!(data[0].get("totalApplied").is_none());
    assert!(data[1].get("totalApplied").is_none());
    assert_eq!(data[0]["id"], json!("ap-1"));
    assert_eq!(data[1]["id"], json!("ap-2"));
}

/// `autopilot_get` on an unknown id replies with a bare `null`
/// (`commands::autopilot::autopilot_get`'s own `json!(ap)` over an
/// `Option`) — there is no object to strip a field from, so this must not
/// panic and must leave the reply exactly `null`.
#[test]
fn drop_dead_fields_leaves_a_null_autopilot_get_reply_untouched() {
    let mut data = json!(null);
    drop_dead_fields("autopilot_get", &mut data);
    assert_eq!(data, json!(null));
}

/// Mutation-check the `(command, field)` pair the same way
/// [`base64_byte_fields_leaves_every_other_command_untouched`] does: the
/// identical payload under a different command name must survive
/// byte-for-byte. Deleting the `*cmd != command` check makes this fail.
#[test]
fn drop_dead_fields_leaves_every_other_command_untouched() {
    let original = json!({ "id": "ap-1", "totalApplied": 0 });
    let mut data = original.clone();
    drop_dead_fields("jobs_list", &mut data);
    assert_eq!(data, original);
}

/// This is the actual reshape it exists to fix: a raw `autopilot_list`
/// reply, run through [`reshape_reply`] the same way `dispatch_direct`
/// really calls it, must not carry `totalApplied` on the wire.
#[test]
fn reshape_reply_drops_total_applied_from_autopilot_list() {
    let data = json!([{ "id": "ap-1", "totalApplied": 0, "status": "active" }]);
    let out = reshape_reply("autopilot_list", data, None);
    assert!(out.as_array().unwrap()[0].get("totalApplied").is_none());
    assert_eq!(out[0]["status"], json!("active"));
}

// ── Per-document truncation marker (`B1-r3-ACLI-R7-5`) ────────────────────
