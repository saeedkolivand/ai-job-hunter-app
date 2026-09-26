//! Agent-layer payload reshaping — everything the generic tier does to a
//! payload that the renderer's own `commands/**` wire shape must not see, and
//! nothing else. Outbound: [`reshape_reply`] owns the ONE order the fence,
//! page and byte-encode steps run in; inbound:
//! [`unfence_named_fields_recursive`] is that fence's mirror, stripping a
//! wrapper a caller echoed back before any command body sees it. Both are
//! pure, and both are applied at the single `super::dispatch_direct`
//! chokepoint.
//!
//! R8 LOC-cap split (`docs/architecture-rules.md`), the same move `agent_read`
//! made for `found_jobs` and `agent_cli::mcp` for `instructions`: this is the
//! RESHAPING unit, so nothing about policy, refusal vocabulary, dispatch or
//! the frame-size ceiling travelled with it — those stay in `agent_call.rs`,
//! which names in one `use` the three items it calls.
//!
//! The fencing TABLES and the outbound fence walk itself are a safety property of that same
//! chokepoint read from BOTH directions (`super::fence_scraped_fields` on the way out, the
//! mirror here on the way in) — moved to their own `agent_call/fence.rs` under this SAME R8
//! reasoning, a sibling this module still reaches as `super::fence_scraped_fields` unchanged
//! (re-exported at `agent_call.rs`'s top, same as this module's own three items are).
//!
//! Split further under this same cap, by concern: list paging (`paging`), the raw-byte-array
//! base64 re-encode (`base64`), the `contact_profile_get`/`_set` projection (`contact_profile`),
//! dropping a dead field (`drop_fields`), the inbound unfence mirror (`unfence`), and the
//! bare-string reply fence plus its truncation marker (`scalar_fence`). The ordering orchestrators
//! ([`reshape_reply`], [`reshape_pre_fence`], [`fence_reply`]) stay here, at the entry point.

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

/// Step 1 of [`reshape_reply`] ("fence first"), factored out so
/// `proof::extract_from_fenced_response` can fence a confirm-proof
/// read the EXACT SAME way [`super::dispatch_direct`] fences every reply a
/// caller actually reads (MEDIUM fix, review round 6 —
/// `B1-r2-ACLI-R6-4`). Before this fn existed, the proof path called only
/// `fence_scraped_fields` directly, one call short of what a real dispatch
/// does — latent today only because no `ProofSource::read_command` is on
/// [`SCALAR_FENCE_COMMANDS`], but the two lists were never asserted disjoint
/// either, so a future row landing on both would have silently reintroduced
/// the "permanently unsatisfiable confirm" bug security review round 4 fixed
/// once already (a caller reading a fenced string, `--confirm` checked
/// against the raw one).
pub(super) fn fence_reply(command: &str, data: &mut Value) {
    fence_scalar_reply(command, data);
    fence_scraped_fields(data);
}

/// Step 0 of [`reshape_reply`] ("drop dead fields, then reserve the
/// truncation marker"), factored out for the SAME reason [`fence_reply`] was
/// (MEDIUM fix, review round 8 — `B2-r1-ACLI-R8-1`): `proof::extract_from_
/// fenced_response` must run the identical pre-fence transform a real
/// dispatch runs, not a hand-rolled subset that stops at fencing. Before this
/// fn existed, [`reshape_reply`] ran [`drop_dead_fields`]/
/// [`mark_truncated_document_text`] inline and the proof path skipped both —
/// latent only because both `documents_list`-backed `ListMatch` proofs read
/// `name` (untouched by either step) and both `autopilot_get`-backed
/// `Lookup` proofs read `name` (`totalApplied` is the only field
/// `drop_dead_fields` touches on that command) — a future proof row reading
/// `text` or `totalApplied` would make its confirm ceremony permanently
/// unsatisfiable the moment either list grows, exactly like the fencing gap
/// this mirrors.
pub(super) fn reshape_pre_fence(command: &str, data: &mut Value) {
    drop_dead_fields(command, data);
    if command == "documents_list" {
        mark_truncated_document_text(data);
    }
}

/// Every reshape a dispatched reply gets before it goes on the wire, in the
/// ONE order they are allowed to run in. Pure — no `AppHandle`, no I/O — so
/// the ordering itself is testable, which is the reason it is a fn at all
/// (as three statements inline, nothing failed when they were reordered).
///
/// 1. **Project first.** [`project_contact_profile_get`] is the other
///    unconditional-over-the-whole-reply safety property (privacy, not
///    injection) — it only ever touches its own one command, so its position
///    relative to the other steps cannot change any of their output; placed
///    first as the same class of "runs no matter what else happens" step
///    fencing is.
/// 2. **Drop dead fields, then reserve the truncation marker.**
///    [`reshape_pre_fence`] removes a [`DROP_FIELDS`] key before anything else
///    looks at the payload — it carries no scraped text to fence, no byte
///    array to re-encode, and dropping it first means the later steps'
///    byte-budget math (paging) never accounts for a key about to disappear
///    anyway. It then reserves [`TRUNCATION_MARKER`] room on `documents_list`
///    only, and this must run BEFORE fencing — it needs the ORIGINAL text
///    length to decide whether the marker applies, which fencing's own
///    truncation would otherwise have already destroyed. This is the SAME fn
///    [`super::proof::extract_from_fenced_response`] calls, so the
///    confirm-proof path and the read path can never run a different
///    pre-fence transform.
/// 3. **Then fence.** [`fence_reply`] ([`fence_scraped_fields`] plus
///    [`fence_scalar_reply`] for the one bare-string reply it structurally
///    cannot reach) is the security property and is unconditional over the
///    WHOLE reply; narrowing it to "only the rows we are about to return"
///    would make its coverage depend on a paging decision. `fence_reply` is
///    the SAME fn [`super::proof::extract_from_fenced_response`] calls, so
///    the confirm-proof path and the read path can never fence differently.
/// 4. **Then page.** [`paginate_list_reply`]'s byte budget must measure the
///    FENCED bytes that will really ship: fencing rewrites every field it
///    touches (`crate::prompt_fence::JOB_CAP` truncates a long one, the
///    wrapper adds to a short one), so a budget applied first would be
///    measuring a payload that no longer exists by the time it ships. This is
///    the step the test mutation-checks: reorder 3 and 4 and the page's row
///    count changes.
/// 5. **Then base64.** [`base64_byte_fields`] must see the raw `Vec<u8>`
///    array rather than something a later step rewrote, and it writes a
///    TOP-LEVEL key — after paging, "top level" means the paged envelope. No
///    command is in both [`PAGINATED_LIST_COMMANDS`] and
///    [`BASE64_BYTE_FIELDS`] today, so no payload can currently observe
///    4-vs-5 ordering; that disjointness is itself asserted in the tests, so
///    the day it stops holding, the guard fires instead of the ordering
///    silently starting to matter unnoticed.
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
