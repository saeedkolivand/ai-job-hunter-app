//! The `reqId` size bound (`MAX_REQ_ID_BYTES`) + its refusal reply. Split out
//! of `mod.rs` (R8 relief — same reasoning as `caller_gate`/`autotrack`/
//! `settings`) — cohesive enough (one constant + the one reply builder that
//! uses it) to own its own module without changing any behavior.
//!
//! Bounded SEPARATELY from [`super::MAX_FRAME_BYTES`], which only caps the
//! whole incoming frame. `reqId` is copied out of every frame and echoed
//! back verbatim in every reply, and the per-connection reply channel
//! (`out_tx` in `handle_connection`) is unbounded — so without this cap, a
//! client could pad `reqId` itself to make an unthrottled read verb
//! (`settings.get`, `profile.get`, …) enqueue multi-megabyte replies faster
//! than the writer task can drain them, without ever tripping
//! `MAX_FRAME_BYTES` on the (tiny) request. Real clients send short ids: the
//! extension's own incrementing counter (`newReqId` in
//! `apps/extension/src/lib/bridge.ts`) and the CLI's own request ids are both
//! a handful of bytes. Enforced once, in `advance_frame_from`, before any
//! type dispatch (see that call site).

use serde_json::json;

use super::msg;

/// Hard cap on the `reqId` string a frame carries — see this module's doc.
pub(super) const MAX_REQ_ID_BYTES: usize = 256;

/// Fixed placeholder `reqId` for [`oversized_req_id_reply`] — the oversized
/// value is exactly what this refusal exists to never put back on the wire.
const OVERSIZED_REQ_ID_PLACEHOLDER: &str = "invalid";

/// Fixed error sentinel for an oversized `reqId` — never derived from the
/// input, so the reply's own size never depends on what triggered it.
const ERR_INVALID_REQ_ID: &str = "invalid_req_id";

/// Build the bounded refusal for a frame whose `reqId` exceeds
/// [`MAX_REQ_ID_BYTES`]. Reuses `advance_authenticated`'s own "unknown
/// message type" fallback shape — an `import.result` error envelope, the
/// same one `import_flow::result_reply` builds — but with a FIXED
/// placeholder `reqId` and a FIXED error sentinel instead of echoing the
/// caller's own (oversized) value back, since echoing it is exactly the
/// amplification this refusal closes.
pub(super) fn oversized_req_id_reply() -> String {
    json!({
        "type": msg::IMPORT_RESULT,
        "reqId": OVERSIZED_REQ_ID_PLACEHOLDER,
        "payload": { "error": ERR_INVALID_REQ_ID },
    })
    .to_string()
}
