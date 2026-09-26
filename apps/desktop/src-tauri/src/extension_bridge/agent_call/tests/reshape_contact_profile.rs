//! Tests for the `contact_profile_get`/`_set` projection and restore (`reshape/contact_profile.rs`).

use super::super::reshape::*;
use super::super::*;

/// A reply built from a store holding a `photo` has NO `photo` key after
/// `reshape_reply`, and every allowlisted field (plus an unrelated future key
/// the allowlist has never heard of) is dropped the same way — the guarantee
/// is "nothing but the named set survives", not "photo specifically is
/// blocked". Every field `CONTACT_PROFILE_AGENT_FIELDS` names is present on
/// the input too, so the second assertion proves the projection is not
/// simply emptying the object.
#[test]
fn reshape_reply_projects_contact_profile_get_to_the_photoless_allowlist() {
    use crate::contact_profile::{ContactLink, ContactProfile, LocalizedText};
    use crate::extension_bridge::autofill_profile::CONTACT_PROFILE_AGENT_FIELDS;

    // Built from the REAL struct (round-1 review, P-r1-F5), not a hand-typed
    // `json!` literal — a hand-typed input can't catch a field added to the
    // struct tomorrow, since it would simply never appear in the literal
    // either. `serde_json::to_value` is the same round trip production takes.
    let profile = ContactProfile {
        full_name: Some("Saeed Kolivand".to_string()),
        email: Some("saeed@example.com".to_string()),
        phone: Some("+31 6 12".to_string()),
        location: Some(LocalizedText {
            default: "Amsterdam".to_string(),
            // Non-empty — an empty `by_lang` is dropped entirely by its own
            // `#[serde(skip_serializing_if = "BTreeMap::is_empty")]`, which
            // would make the `byLang` assertion below inert against exactly
            // the skip-serialized shape it claims to cover (round-2 review,
            // P-r2-R2-F3).
            by_lang: [("de".to_string(), "Amsterdam".to_string())].into(),
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
    let mut raw = serde_json::to_value(&profile).expect("ContactProfile serializes");
    raw.as_object_mut().expect("object").insert(
        "someFutureLocalOnlyField".to_string(),
        json!("must not survive either"),
    );

    let out = reshape_reply("contact_profile_get", raw, None);
    let out_map = out.as_object().expect("still an object");

    assert!(
        !out_map.contains_key("photo"),
        "photo must never cross this wire"
    );
    assert!(!out_map.contains_key("someFutureLocalOnlyField"));
    for field in CONTACT_PROFILE_AGENT_FIELDS {
        assert!(
            out_map.contains_key(*field),
            "`{field}` must survive the projection"
        );
    }
    assert_eq!(out_map.len(), CONTACT_PROFILE_AGENT_FIELDS.len());

    // P-r1-F5: the top-level allowlist is not enough — `location` and
    // `extraLinks` are the source struct's own nested types crossing the
    // wire VERBATIM. Assert their key sets too, or a field added to either
    // later passes straight through with nothing here to notice.
    let location = out_map["location"]
        .as_object()
        .expect("location is an object");
    // Exact set, not membership (round-2 review, P-r2-R2-F3): a membership
    // check over whatever keys HAPPEN to be present is inert against a
    // fixture whose `byLang` never serializes at all, which is exactly the
    // shape the fixture above used to have.
    let mut location_keys: Vec<&str> = location.keys().map(String::as_str).collect();
    location_keys.sort_unstable();
    assert_eq!(location_keys, ["byLang", "default"]);
    let extra_links = out_map["extraLinks"]
        .as_array()
        .expect("extraLinks is an array");
    for link in extra_links {
        let link = link.as_object().expect("extraLinks entry is an object");
        for key in link.keys() {
            assert!(
                ["label", "url"].contains(&key.as_str()),
                "unexpected extraLinks entry key `{key}` crossed the wire"
            );
        }
    }
}

// ── contact_profile_set local-only-field restore (round-1 review, issue
// #1180; generalised round-2, P-r2-R2-F2) ──────────────────────────────

/// P-r2-AC-R5-F2 (MEDIUM, round-2 review, issue #1180): `CONTACT_PROFILE_SET_COMMAND`
/// is the one string that decides whether the whole restore above fires at
/// all, and unlike its sibling `CONTACT_PROFILE_GET_COMMAND` (pinned by
/// `commands_marks_the_contact_profile_get_row_with_its_projection_note` in
/// `agent_cli::mcp::tests`) it had no anchor to a real `POLICY` row — a
/// rename of the underlying command would leave this const matching
/// nothing, silently stop the restore, and reopen the CRITICAL with a fully
/// green suite.
#[test]
fn contact_profile_set_command_matches_a_real_policy_row() {
    assert!(
        super::super::super::agent_cli::policy::POLICY
            .iter()
            .any(|e| split_path(e.path) == ("contact_profile", CONTACT_PROFILE_SET_COMMAND)),
        "CONTACT_PROFILE_SET_COMMAND must name a real POLICY row"
    );
}

/// P-r3-AC-R7-F2 (MEDIUM, round-3 review, issue #1180): unlike the command name above,
/// [`restore_local_only_contact_fields`]'s `"profile"` key is resolved against nothing — it
/// mirrors the Tauri parameter name in `contact_profile_set`'s own signature, which
/// (`docs/knowledge/agent-cli.md`) "exists only in the handler signature under `commands/`".
/// A parameter rename there (e.g. to `payload`) would leave this key matching nothing,
/// silently disarm the restore, and reopen the CRITICAL photo-deletion with a green suite —
/// so pin the real signature text here.
#[test]
fn contact_profile_set_payload_key_matches_the_real_handler_signature() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/commands/contact_profile.rs"
    ));
    assert!(
        SOURCE.contains("pub async fn contact_profile_set(app: AppHandle, profile: Value)"),
        "restore_local_only_contact_fields reads input[\"profile\"] — the handler's own \
         parameter must still be named `profile`"
    );
}

/// The CRITICAL repro (P-r1-F1): an agent read-modify-write that never saw
/// `photo` (because [`project_contact_profile_get`] already stripped it) must
/// not delete it on the whole-row-replace write.
#[test]
fn restore_local_only_contact_fields_reinjects_the_stored_photo_when_the_payload_omits_it() {
    let mut input = json!({ "profile": { "fullName": "Jane Doe" } });
    let stored = json!({ "fullName": "Jane Doe", "photo": "data:image/png;base64,AAAA" });
    restore_local_only_contact_fields("contact_profile_set", &mut input, Some(&stored));
    assert_eq!(input["profile"]["photo"], "data:image/png;base64,AAAA");
}

/// P-r2-AC-R5-F1 (HIGH, round-2 review, issue #1180): a `null` is a shape
/// the published contract (`photo?: string`) does not even permit, and the
/// renderer's own clear gesture OMITS the key rather than sending `null` —
/// so an agent read-modify-write that echoes an explicit `"photo": null`
/// (e.g. because its JSON library round-trips an absent field as `null`)
/// must not be treated as a deliberate delete either; the stored value is
/// restored the same as an outright omission. This is the inversion of the
/// former `restore_local_only_contact_fields_respects_an_explicit_value_including_null`,
/// which encoded the opposite, data-losing rule.
#[test]
fn restore_local_only_contact_fields_treats_an_explicit_null_as_not_supplied_and_restores_the_stored_value(
) {
    let mut input = json!({ "profile": { "photo": null } });
    let stored = json!({ "photo": "stored" });
    restore_local_only_contact_fields("contact_profile_set", &mut input, Some(&stored));
    assert_eq!(input["profile"]["photo"], "stored");
}

/// Same rule, the other shape no UI ever emits: an explicit `""` is treated
/// as "not supplied" too, not as a deliberate delete.
#[test]
fn restore_local_only_contact_fields_treats_an_explicit_empty_string_as_not_supplied_and_restores_the_stored_value(
) {
    let mut input = json!({ "profile": { "photo": "" } });
    let stored = json!({ "photo": "stored" });
    restore_local_only_contact_fields("contact_profile_set", &mut input, Some(&stored));
    assert_eq!(input["profile"]["photo"], "stored");
}

/// The other half of the branch: a genuine, non-empty explicit value for a
/// non-allowlisted field IS a real, visible choice (the caller must have
/// computed or been given it some other way) and must not be clobbered by
/// the stored one.
#[test]
fn restore_local_only_contact_fields_respects_a_genuine_non_empty_explicit_value() {
    let mut input = json!({ "profile": { "photo": "data:image/png;base64,NEW" } });
    let stored = json!({ "photo": "data:image/png;base64,OLD" });
    restore_local_only_contact_fields("contact_profile_set", &mut input, Some(&stored));
    assert_eq!(input["profile"]["photo"], "data:image/png;base64,NEW");
}

#[test]
fn restore_local_only_contact_fields_is_a_no_op_for_any_other_command() {
    let mut input = json!({ "profile": { "fullName": "Jane Doe" } });
    let stored = json!({ "photo": "stored" });
    restore_local_only_contact_fields("jobs_list", &mut input, Some(&stored));
    assert!(input["profile"].get("photo").is_none());
}

#[test]
fn restore_local_only_contact_fields_is_a_no_op_when_nothing_is_stored() {
    let mut input = json!({ "profile": { "fullName": "Jane Doe" } });
    restore_local_only_contact_fields("contact_profile_set", &mut input, None);
    assert!(input["profile"].get("photo").is_none());
}

/// P-r2-R2-F2: the restore is not photo-specific. ANY key the stored
/// profile carries that `CONTACT_PROFILE_AGENT_FIELDS` does not name is
/// restored the same way, so the next local-only field added to
/// `ContactProfile` gets this fix for free instead of reproducing the
/// CRITICAL the day someone forgets this fn also names `photo` specifically.
#[test]
fn restore_local_only_contact_fields_restores_any_field_the_allowlist_does_not_name() {
    let mut input = json!({ "profile": { "fullName": "Jane Doe" } });
    let stored = json!({ "fullName": "Jane Doe", "someFutureLocalOnlyField": "keep-me" });
    restore_local_only_contact_fields("contact_profile_set", &mut input, Some(&stored));
    assert_eq!(input["profile"]["someFutureLocalOnlyField"], "keep-me");
}

/// The allowlist skip is the OTHER half of the loop body, untouched by any
/// test above (every prior fixture's allowlisted key was already present in
/// the payload, so `profile.contains_key(key)` alone would have skipped it
/// too). An agent CAN see `email` ([`CONTACT_PROFILE_AGENT_FIELDS`] names
/// it), so omitting it from the whole-row-replace payload is a real,
/// visible deletion — unlike `photo`, it must NOT be reinjected. Deleting
/// the allowlist `continue` (leaving only the `contains_key` check) makes
/// this fail while every other `restore_local_only_contact_fields` test
/// above stays green.
#[test]
fn restore_local_only_contact_fields_does_not_reinject_an_omitted_allowlisted_field() {
    let mut input = json!({ "profile": { "fullName": "Jane Doe" } });
    let stored = json!({
        "fullName": "Jane Doe",
        "email": "old@example.com",
        "photo": "data:image/png;base64,AAAA",
    });
    restore_local_only_contact_fields("contact_profile_set", &mut input, Some(&stored));
    assert!(
        input["profile"].get("email").is_none(),
        "an allowlisted field the caller can see and chose to omit is a real deletion"
    );
    assert_eq!(
        input["profile"]["photo"], "data:image/png;base64,AAAA",
        "the non-allowlisted field must still be restored in the same call"
    );
}

/// The gate is by command name, not by shape: another command whose reply
/// happens to carry a `photo`-named key is left untouched.
#[test]
fn reshape_reply_leaves_a_photo_key_alone_on_any_other_command() {
    let out = reshape_reply(
        "jobs_list",
        json!({ "id": "j-1", "photo": "keep-me" }),
        None,
    );
    assert_eq!(out, json!({ "id": "j-1", "photo": "keep-me" }));
}

/// A non-object `contact_profile_get` reply (never real in production, but
/// the projection must degrade rather than panic) is returned verbatim.
#[test]
fn reshape_reply_projection_is_a_noop_on_a_non_object_reply() {
    let out = reshape_reply("contact_profile_get", json!("not an object"), None);
    assert_eq!(out, json!("not an object"));
}
