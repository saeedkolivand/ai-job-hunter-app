//! `profile.get` consent gate (`resolve_profile`) + the wire-key pinning that keeps
//! [`CONTACT_PROFILE_AGENT_FIELDS`] honest -- redistributed from the crate-level `test.rs` (R8
//! relief).

use super::super::*;

#[test]
fn resolve_profile_refuses_when_opt_in_off() {
    use crate::contact_profile::ContactProfile;
    let profile = ContactProfile {
        email: Some("a@b.com".to_string()),
        ..Default::default()
    };
    // Even with a profile present, opt-in OFF returns a clear refusal, never data.
    let err = resolve_profile(false, Some(&profile)).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Autofill is off"), "refusal message: {msg}");
}

#[test]
fn resolve_profile_projects_when_opt_in_on() {
    use crate::contact_profile::{ContactProfile, LocalizedText};
    let profile = ContactProfile {
        full_name: Some("Saeed Kolivand".to_string()),
        email: Some("saeed@example.com".to_string()),
        phone: Some("  +31 6 12  ".to_string()), // trimmed on projection
        location: Some(LocalizedText {
            default: "Amsterdam, Netherlands".to_string(),
            ..Default::default()
        }),
        linkedin: Some("https://linkedin.com/in/saeed".to_string()),
        website: Some("   ".to_string()), // whitespace-only -> dropped
        ..Default::default()
    };
    let out = resolve_profile(true, Some(&profile)).expect("opt-in on returns the profile");
    assert_eq!(out.full_name.as_deref(), Some("Saeed Kolivand"));
    assert_eq!(out.email.as_deref(), Some("saeed@example.com"));
    assert_eq!(out.phone.as_deref(), Some("+31 6 12"));
    assert_eq!(out.location.as_deref(), Some("Amsterdam, Netherlands"));
    assert_eq!(
        out.linkedin.as_deref(),
        Some("https://linkedin.com/in/saeed")
    );
    assert_eq!(out.website, None, "whitespace-only fields are dropped");
    assert_eq!(out.github, None);
}

/// The `profile` resource ([`super::super::agent_read`]) reuses this exact `resolve_profile`
/// outcome verbatim -- so this pins BOTH `profile.get`'s and the agent `profile` resource's wire
/// key set in one place. Hand-written, not derived from `AutofillProfile`'s own field list (a
/// self-referential check proves nothing -- see the repo's standing lesson on exactly this).
/// `ContactProfile.photo` is populated here too, to prove it never crosses:
/// `AutofillProfile::from_contact` has no field to receive it.
#[test]
fn resolve_profile_projection_has_exact_keys_and_no_forbidden_fields() {
    use crate::contact_profile::{ContactLink, ContactProfile, LocalizedText};
    let profile = ContactProfile {
        full_name: Some("Saeed Kolivand".to_string()),
        email: Some("saeed@example.com".to_string()),
        phone: Some("+31 6 12".to_string()),
        location: Some(LocalizedText {
            default: "Amsterdam".to_string(),
            ..Default::default()
        }),
        linkedin: Some("https://linkedin.com/in/saeed".to_string()),
        github: Some("https://github.com/saeed".to_string()),
        website: Some("https://saeed.dev".to_string()),
        extra_links: vec![ContactLink {
            label: "Portfolio".to_string(),
            url: "https://saeed.dev/p".to_string(),
        }],
        photo: Some("data:image/png;base64,AAAA".to_string()),
    };
    let out = resolve_profile(true, Some(&profile)).expect("projects");
    let value = serde_json::to_value(&out).unwrap();
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "email",
            "extraLinks",
            "fullName",
            "github",
            "linkedin",
            "location",
            "phone",
            "website",
        ]
    );
    assert!(
        !value.to_string().contains("data:image"),
        "the candidate photo must never cross this wire"
    );
}

/// `CONTACT_PROFILE_AGENT_FIELDS` (issue #1180) drives the generic tier's
/// `contact_profile_get` allowlist (`agent_call::reshape::project_contact_profile_get`) -- pinned
/// here against a HAND-WRITTEN literal, not derived from `AutofillProfile`'s own serialization,
/// for the same reason the exact-keys test above is hand-written: a check derived from the very
/// struct it is meant to catch drifting proves nothing.
#[test]
fn contact_profile_agent_fields_matches_the_autofill_profile_wire_shape() {
    let mut fields: Vec<&str> = CONTACT_PROFILE_AGENT_FIELDS.to_vec();
    fields.sort_unstable();
    assert_eq!(
        fields,
        vec![
            "email",
            "extraLinks",
            "fullName",
            "github",
            "linkedin",
            "location",
            "phone",
            "website",
        ]
    );
}

/// Fields present on `AutofillProfile`'s wire shape but deliberately kept OFF
/// `CONTACT_PROFILE_AGENT_FIELDS` -- hand-written, not derived (round-3 review, issue #1180,
/// P-r3-AC-R7-F3). Today's answer is "none", but naming the list separately means a future
/// `AutofillProfile` field fails [`contact_profile_agent_fields_matches_a_fully_populated_autofill_profile_wire_shape`]
/// until someone deliberately files it under ONE of the two lists -- rather than the mechanical
/// "add it to `CONTACT_PROFILE_AGENT_FIELDS`, the test is green" fix, which IS the widening
/// decision (un-gating that field on the ungated generic `contact_profile_get` row and dropping
/// it from `restore_local_only_contact_fields`'s protection) made by default, not on purpose.
const AGENT_EXCLUDED_FIELDS: &[&str] = &[];

/// The derived HALF of the guard above (round-2 review, P-r2-R2-F5): the hand-written literal
/// there only catches `CONTACT_PROFILE_AGENT_FIELDS` drifting from ITSELF; it never notices a
/// field added to `AutofillProfile` and forgotten here, because both guards compare literal to
/// literal. This one serializes a FULLY populated `AutofillProfile` (every
/// `skip_serializing_if` field set, so nothing is silently omitted) and compares its real wire
/// key set to `CONTACT_PROFILE_AGENT_FIELDS` UNION [`AGENT_EXCLUDED_FIELDS`] -- the literal above
/// stays as the deletion guard, this is what fails when a field is added.
#[test]
fn contact_profile_agent_fields_matches_a_fully_populated_autofill_profile_wire_shape() {
    use crate::contact_profile::ContactLink;
    let profile = AutofillProfile {
        full_name: Some("Saeed Kolivand".to_string()),
        email: Some("saeed@example.com".to_string()),
        phone: Some("+31 6 12".to_string()),
        location: Some("Amsterdam".to_string()),
        linkedin: Some("https://linkedin.com/in/saeed".to_string()),
        github: Some("https://github.com/saeed".to_string()),
        website: Some("https://saeed.dev".to_string()),
        extra_links: vec![ContactLink {
            label: "Portfolio".to_string(),
            url: "https://saeed.dev/p".to_string(),
        }],
    };
    let value = serde_json::to_value(&profile).unwrap();
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    let mut expected: Vec<&str> = CONTACT_PROFILE_AGENT_FIELDS
        .iter()
        .chain(AGENT_EXCLUDED_FIELDS)
        .copied()
        .collect();
    expected.sort_unstable();
    assert_eq!(keys, expected);
}

#[test]
fn resolve_profile_errors_when_store_missing() {
    // opt-in on but no profile available (store not managed) -> a Config error, not a panic.
    assert!(resolve_profile(true, None).is_err());
}

#[test]
fn profile_result_reply_carries_type_and_req_id() {
    use crate::contact_profile::ContactProfile;
    let out = resolve_profile(
        true,
        Some(&ContactProfile {
            email: Some("x@y.z".to_string()),
            ..Default::default()
        }),
    );
    let reply = profile_result_reply("req-42", out);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::PROFILE_RESULT);
    assert_eq!(v["reqId"], "req-42");
    assert_eq!(v["payload"]["email"], "x@y.z");
    assert!(v["payload"].get("error").is_none());
}

#[test]
fn profile_result_reply_carries_refusal_error() {
    let reply = profile_result_reply("req-7", resolve_profile(false, None));
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::PROFILE_RESULT);
    assert!(v["payload"]["error"]
        .as_str()
        .unwrap()
        .contains("Autofill is off"));
}
