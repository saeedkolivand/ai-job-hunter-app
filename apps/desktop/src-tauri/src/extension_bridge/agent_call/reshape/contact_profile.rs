//! The `contact_profile_get`/`contact_profile_set` projection: strip `photo` from an outgoing
//! read, and restore every local-only field an agent never saw on the matching write so a
//! read-modify-write round trip cannot silently delete it (issue #1180).

use serde_json::Value;

/// The one command whose raw reply is projected to a photo-less allowlist before an agent ever
/// sees it (issue #1180). `contact_profile_get`'s real body returns the whole `ContactProfile`,
/// including `photo` (a local-only `data:image/…;base64,…` URI, never sent over the network); the
/// dedicated `profile` resource already resolves through that photo-less shape, so this makes the
/// generic tier's `call-read contact_profile_get` match it instead of leaking `photo`.
// `pub(in crate::extension_bridge)`, not `pub(super)`: `agent_cli::mcp`'s `commands` tool (a
// COUSIN, not a descendant) names this row too, same reasoning as `PAGINATED_LIST_COMMANDS`.
pub(in crate::extension_bridge) const CONTACT_PROFILE_GET_COMMAND: &str = "contact_profile_get";

/// What the `commands` tool prints on the [`CONTACT_PROFILE_GET_COMMAND`] row — plain `call-read`
/// never sees an MCP tool description, so `commands` and the `call` verb's own `--help` text are
/// the two places such a caller can learn the reply is projected (round-1 review, issue #1180).
pub(in crate::extension_bridge) const CONTACT_PROFILE_GET_PROJECTION_NOTE: &str =
    "returns only {fullName,email,phone,location,linkedin,github,website,extraLinks} — `photo` \
     is stripped before an agent ever sees it. The surviving fields are the RAW stored shapes \
     (`location` is {default,byLang}, `extraLinks` is unfiltered and uncapped), not the cleaned, \
     opt-in-gated strings the `profile` MCP resource projects — the two share field names minus \
     `photo`, nothing else.";

/// Drop every top-level key of [`CONTACT_PROFILE_GET_COMMAND`]'s reply that is
/// not in [`super::super::super::autofill_profile::CONTACT_PROFILE_AGENT_FIELDS`] — a
/// no-op for every other command, and for a non-object reply.
pub(super) fn project_contact_profile_get(command: &str, data: &mut Value) {
    if command != CONTACT_PROFILE_GET_COMMAND {
        return;
    }
    let Some(map) = data.as_object_mut() else {
        return;
    };
    map.retain(|k, _| {
        super::super::super::autofill_profile::CONTACT_PROFILE_AGENT_FIELDS.contains(&k.as_str())
    });
}

/// The write leg of [`CONTACT_PROFILE_GET_COMMAND`]'s projection (round-1 review, issue #1180,
/// CRITICAL). An agent's only edit path — read, change one field, write the whole object back —
/// can only ever send a `contact_profile_set` payload with a local-only field (today, `photo`)
/// entirely ABSENT, never an explicit value (it cannot type back what it was never shown).
/// `contact_profile_set` is a whole-row REPLACE, so an absent key silently and permanently deletes
/// the stored value on this `Reversible` row. [`restore_local_only_contact_fields`] closes that at
/// the one dispatch chokepoint, for EVERY such field (round-2 review, P-r2-R2-F2), not a single
/// hardcoded name — the next local-only field added without a matching
/// `CONTACT_PROFILE_AGENT_FIELDS` entry would otherwise reproduce the identical bug.
pub(in crate::extension_bridge::agent_call) const CONTACT_PROFILE_SET_COMMAND: &str =
    "contact_profile_set";

/// Re-inject every key of `stored_profile` that `CONTACT_PROFILE_AGENT_FIELDS` does not name into
/// an outgoing [`CONTACT_PROFILE_SET_COMMAND`] call's `profile` object, unless the caller's
/// payload supplies a present, non-null, non-empty-string value. Round-2 review (issue #1180,
/// P-r2-AC-R5-F1) found the opposite rule backwards: an explicit `null`/`""` is not a deliberate
/// choice an agent could make, since `project_contact_profile_get` never showed it a value to null
/// out, and no UI path emits either shape either — so both are treated as "not supplied" and
/// restored, closing the same data-loss hole as an outright omission. Pure — the impure half
/// (reading the current stored profile) is `dispatch_direct`'s job. A no-op for every other
/// command, or a non-object `input`/`input.profile`/`stored_profile`.
pub(in crate::extension_bridge::agent_call) fn restore_local_only_contact_fields(
    command: &str,
    input: &mut Value,
    stored_profile: Option<&Value>,
) {
    if command != CONTACT_PROFILE_SET_COMMAND {
        return;
    }
    let Some(profile) = input.get_mut("profile").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(stored) = stored_profile.and_then(Value::as_object) else {
        return;
    };
    for (key, value) in stored {
        if super::super::super::autofill_profile::CONTACT_PROFILE_AGENT_FIELDS
            .contains(&key.as_str())
        {
            continue;
        }
        let supplied = match profile.get(key) {
            None | Some(Value::Null) => false,
            Some(Value::String(s)) => !s.is_empty(),
            Some(_) => true,
        };
        if supplied {
            continue;
        }
        profile.insert(key.clone(), value.clone());
    }
}
