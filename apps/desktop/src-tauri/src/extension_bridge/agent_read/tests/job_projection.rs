//! Tests for the `job` resource's allowlist projection (`job.rs`).

use super::super::job::{project_value, AgentJob};
use super::support::{assert_object_keys, full_found_job};

#[test]
fn job_projection_has_exact_keys_and_drops_forbidden_fields() {
    let value = project_value::<_, AgentJob>(&full_found_job()).expect("projects");
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "applied",
            "board",
            "clusterMembers",
            "company",
            "description",
            "foundAt",
            "isAgency",
            "isNew",
            "location",
            "postedAt",
            "salaryCurrency",
            "salaryMax",
            "salaryMin",
            "score",
            "scoreProvisional",
            "scoreSource",
            "title",
            "trust",
            "url",
        ]
    );
    let member = &value["clusterMembers"][0];
    assert!(
        member.get("key").is_none(),
        "cluster member's opaque `key` must not cross the wire"
    );
    // NESTED descent (finding #2, security review) — the top-level key
    // set above proves nothing about `trust`'s OWN keys, since it is a
    // whole nested object.
    assert_object_keys(&value["trust"], "job.trust", &["score", "level", "flags"]);
}

#[test]
fn job_projection_never_carries_forbidden_keys() {
    let value = project_value::<_, AgentJob>(&full_found_job()).expect("projects");
    let text = value.to_string();
    for forbidden in ["assistantNotes", "clusterId", "clusterCanonical"] {
        assert!(!text.contains(forbidden), "leaked {forbidden}");
    }
}
