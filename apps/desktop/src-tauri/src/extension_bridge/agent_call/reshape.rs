//! Agent-layer payload reshaping — everything the generic tier does to a payload that the
//! renderer's own `commands/**` wire shape must not see, and nothing else. Outbound:
//! [`reshape_reply`] owns the ONE order the fence, page and byte-encode steps run in; inbound:
//! [`unfence_named_fields_recursive`] is that fence's mirror, stripping a wrapper a caller echoed
//! back before any command body sees it. Both pure, both applied at the single
//! `super::dispatch_direct` chokepoint.
//!
//! R8 LOC-cap split: nothing about policy, refusal vocabulary, dispatch or the frame-size ceiling
//! travelled with it — those stay in `agent_call.rs`. The fencing tables/walk are the same
//! chokepoint's safety property read from BOTH directions and live in the sibling
//! `agent_call/fence.rs`, reached here as `super::fence_scraped_fields` unchanged.
//!
//! Split further, by concern: list paging (`paging`), base64 re-encode (`base64`), the
//! `contact_profile_get`/`_set` projection (`contact_profile`), dropping a dead field
//! (`drop_fields`), the inbound unfence mirror (`unfence`), and the bare-string reply fence plus
//! its truncation marker (`scalar_fence`). The ordering orchestrators stay here.

use serde_json::Value;

use super::fence::*;

mod base64;
mod contact_profile;
mod drop_fields;
mod paging;
mod scalar_fence;
mod unfence;

pub(in crate::extension_bridge) use base64::{base64_byte_fields, encode_base64};
#[cfg(test)]
pub(super) use base64::{BASE64_BYTE_FIELDS, BASE64_ENCODING, ENCODING_KEY_SUFFIX};
use contact_profile::project_contact_profile_get;
pub(super) use contact_profile::{restore_local_only_contact_fields, CONTACT_PROFILE_SET_COMMAND};
pub(in crate::extension_bridge) use contact_profile::{
    CONTACT_PROFILE_GET_COMMAND, CONTACT_PROFILE_GET_PROJECTION_NOTE,
};
pub(super) use drop_fields::drop_dead_fields;
#[cfg(test)]
pub(super) use drop_fields::DROP_FIELDS;
pub(super) use paging::{paginate_list_reply, take_list_page_args};
#[cfg(test)]
pub(super) use paging::{DEFAULT_LIST_PAGE_LIMIT, LIST_PAGE_BYTE_BUDGET, MAX_LIST_PAGE_LIMIT};
pub(in crate::extension_bridge) use paging::{PAGINATED_LIST_COMMANDS, PAGINATED_LIST_NOTE};
use scalar_fence::fence_scalar_reply;
pub(super) use scalar_fence::mark_truncated_document_text;
#[cfg(test)]
pub(in crate::extension_bridge) use scalar_fence::EMITTED_FENCE_TAGS;
#[cfg(test)]
pub(super) use scalar_fence::{reserve_truncation_marker, TRUNCATION_MARKER};
pub(super) use unfence::unfence_named_fields_recursive;

/// Step 3 of [`reshape_reply`], factored out so `proof::extract_from_fenced_response` can fence a
/// confirm-proof read the EXACT SAME way `dispatch_direct` fences every reply a caller actually
/// reads (MEDIUM fix, review round 6): the two must never diverge, or a future command on both
/// [`SCALAR_FENCE_COMMANDS`] and a `ProofSource::read_command` would reintroduce the "permanently
/// unsatisfiable confirm" bug round 4 already fixed once (a caller reading a fenced string,
/// `--confirm` checked against the raw one).
pub(super) fn fence_reply(command: &str, data: &mut Value) {
    fence_scalar_reply(command, data);
    fence_scraped_fields(data);
}

/// Step 2 of [`reshape_reply`], factored out for the SAME reason [`fence_reply`] was (MEDIUM fix,
/// review round 8): `proof::extract_from_fenced_response` must run the identical pre-fence
/// transform a real dispatch runs — a future proof row reading `text` or `totalApplied` would
/// otherwise make its confirm ceremony permanently unsatisfiable the moment either list grows,
/// exactly like the fencing gap this mirrors.
pub(super) fn reshape_pre_fence(command: &str, data: &mut Value) {
    drop_dead_fields(command, data);
    if command == "documents_list" {
        mark_truncated_document_text(data);
    }
}

/// Every reshape a dispatched reply gets before it goes on the wire, in the ONE order they are
/// allowed to run in. Pure — no `AppHandle`, no I/O — so the ordering itself is testable (as three
/// statements inline, nothing failed when they were reordered; this fn is why it now does).
///
/// 1. **Project first.** [`project_contact_profile_get`] only ever touches its own one command, so
///    its position relative to the other steps cannot change their output; placed first as the
///    same "runs no matter what else happens" class of step fencing is.
/// 2. **Drop dead fields, then reserve the truncation marker.** [`reshape_pre_fence`] removes a
///    [`DROP_FIELDS`] key before anything else looks at the payload, so later byte-budget math
///    never accounts for a key about to disappear. It then reserves [`TRUNCATION_MARKER`] room on
///    `documents_list` only, BEFORE fencing — it needs the ORIGINAL text length, which fencing's
///    own truncation would otherwise have destroyed. The SAME fn [`super::proof::
///    extract_from_fenced_response`] calls, so the proof and read paths can never diverge here.
/// 3. **Then fence.** [`fence_reply`] is the security property and unconditional over the WHOLE
///    reply — narrowing it to "only the rows about to return" would make coverage depend on a
///    paging decision. Same fn the proof path calls, for the same reason as step 2.
/// 4. **Then page.** [`paginate_list_reply`]'s byte budget must measure the FENCED bytes that will
///    really ship — fencing rewrites every field it touches, so a budget applied first would
///    measure a payload that no longer exists by the time it ships (mutation-checked: reorder 3
///    and 4 and the page's row count changes).
/// 5. **Then base64.** [`base64_byte_fields`] must see the raw `Vec<u8>` array rather than
///    something a later step rewrote, and writes a TOP-LEVEL key (post-paging, that means the
///    paged envelope). No command is in both [`PAGINATED_LIST_COMMANDS`] and [`BASE64_BYTE_FIELDS`]
///    today (asserted in the tests), so no payload can currently observe 4-vs-5 ordering.
pub(super) fn reshape_reply(
    command: &str,
    mut data: Value,
    page_args: Option<(usize, usize)>,
) -> Value {
    project_contact_profile_get(command, &mut data);
    reshape_pre_fence(command, &mut data);
    fence_reply(command, &mut data);
    if let Some((offset, limit)) = page_args {
        data = paginate_list_reply(data, offset, limit);
    }
    base64_byte_fields(command, &mut data);
    data
}
