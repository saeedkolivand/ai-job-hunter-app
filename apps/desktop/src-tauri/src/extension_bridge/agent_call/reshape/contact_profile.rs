//! The `contact_profile_get`/`contact_profile_set` projection: strip `photo` from an outgoing
//! read, and restore every local-only field an agent never saw on the matching write so a
//! read-modify-write round trip cannot silently delete it (issue #1180).

use serde_json::Value;

/// The one command whose raw reply is projected to a photo-less allowlist
/// before an agent ever sees it (issue #1180). `contact_profile_get`'s real
/// body (`commands::contact_profile::contact_profile_get`) returns the whole
/// `ContactProfile`, including `photo` — a `data:image/…;base64,…` URI
/// `contact_profile::mod.rs` documents as local-only and never sent over the
/// network. The dedicated `profile` resource (`autofill_profile::AutofillProfile`)
/// already resolves through that exact photo-less shape; this makes the
/// generic tier's `call-read contact_profile_get` match it instead of being
/// the one path that still hands a local-only field to whatever reads an
/// agent reply.
// `pub(in crate::extension_bridge)`, not `pub(super)`: `agent_cli::mcp`'s
// `commands` tool (a COUSIN, not a descendant of this module) names this row
// too, on the same "no second hand-typed command-name string" reasoning
// `PAGINATED_LIST_COMMANDS` already documents above.
pub(in crate::extension_bridge) const CONTACT_PROFILE_GET_COMMAND: &str = "contact_profile_get";

/// What the `commands` tool prints on the [`CONTACT_PROFILE_GET_COMMAND`] row
/// — the SAME discovery precedent [`PAGINATED_LIST_NOTE`] sets, for a caller
/// this projection actually affects and who cannot read this source: plain
/// `call-read` (and any `ajh-tauri agent call` invocation) never sees an MCP
/// tool description at all, so `commands` and the `call` verb's own
/// `--help` text (`agent_cli::VERB_TABLE`'s `call` row, round-2 review) are
/// the two places such a caller can learn the reply is projected (round-1
/// review, issue #1180).
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

/// The write leg of [`CONTACT_PROFILE_GET_COMMAND`]'s projection (round-1
/// review, issue #1180, CRITICAL). The generic tier's `contact_profile_get`
/// reply never carries a field outside
/// [`super::super::super::autofill_profile::CONTACT_PROFILE_AGENT_FIELDS`] — today
/// that is only `photo`, but the allowlist is what decides that, not this
/// fn — so an agent doing the only edit path this tier has — read, change
/// one field, write the whole object back — can only ever send a
/// `contact_profile_set` payload with any such key entirely ABSENT, never an
/// explicit value (it cannot type back what it was never shown).
/// `contact_profile_set` is a whole-row REPLACE
/// (`contact_profile::ContactProfileStore::set`), so an absent key silently
/// and permanently deletes the stored value — on a row policy declares
/// [`super::super::agent_cli::policy::Effect::Reversible`].
/// [`restore_local_only_contact_fields`] closes that at the one dispatch
/// chokepoint, before the write ever reaches the command body, for EVERY
/// such field, not a single hardcoded name (round-2 review, P-r2-R2-F2):
/// the next local-only field added to `ContactProfile` without a matching
/// entry in [`super::super::super::autofill_profile::CONTACT_PROFILE_AGENT_FIELDS`]
/// reproduces the identical
/// delete-on-read-modify-write, with nothing here to notice, if the fix
/// only ever names `photo`.
pub(in crate::extension_bridge::agent_call) const CONTACT_PROFILE_SET_COMMAND: &str =
    "contact_profile_set";

/// Re-inject every key of `stored_profile` that
/// [`super::super::super::autofill_profile::CONTACT_PROFILE_AGENT_FIELDS`] does not
/// name into an outgoing [`CONTACT_PROFILE_SET_COMMAND`] call's
/// `profile` object, unless the caller's payload supplies a real value for
/// that key — a present, non-null, non-empty-string value. Round-2 review
/// (issue #1180, P-r2-AC-R5-F1) found the opposite rule here: treating an
/// explicit `null`/`""` as "the caller's own deliberate choice" and skipping
/// the restore. That was backwards — the published contract for this field
/// (`photo?: string`, `packages/shared/src/ipc/contracts/contactProfile.ts`)
/// never permits an explicit `null`, and the renderer's OWN clear gesture
/// (`ContactProfileForm`'s `persistPhoto`) OMITS the key, it never sends
/// `null` or `""`, on its own write path outside this dispatcher. So `null`
/// and `""` are shapes no UI emits either — an agent sending one is not
/// expressing a choice it could have made deliberately, since
/// `project_contact_profile_get` never showed it a value to null out in the
/// first place. Treating them as "not supplied" and restoring the stored
/// value closes the same data-loss hole as an outright omission. Pure — the
/// impure half (reading the CURRENT stored profile before this write lands,
/// as `serde_json::to_value`) is `dispatch_direct`'s job, the same "read app
/// state, pass the value in" split this module already uses. A no-op for
/// every other command, a non-object `input`, an `input.profile` that is not
/// an object, or a `stored_profile` that is not an object.
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
