use serde_json::{json, Value};

use super::UpdaterState;
use crate::platform::PackageFlavour;

/// The `{ available: false, managedBy }` reply for a build a store/sandbox
/// owns — used directly by `updater_check` (once it already knows it is in
/// the packaged branch) and via `Option::map` by [`status_reply`] (which
/// still needs the "or check normally" `None` case).
///
/// `flavour` comes from [`crate::platform::packaged_flavour`] (MSIX or
/// Snap); it is a parameter rather than a call so the decision
/// is testable off-Windows and without a live `AppHandle` (this crate has
/// no `tauri::test` mock-app harness — same reason
/// [`super::download_in_progress_or_done`] is split out).
///
/// Shape: the existing `{ available: false }` reply plus `managedBy` — see
/// `UpdateCheckResult` in `packages/shared/src/ipc/contracts/updater.ts`.
pub(crate) fn store_managed(flavour: PackageFlavour) -> Value {
    json!({ "available": false, "managedBy": flavour.as_wire_str() })
}

/// The pushed counterpart of [`store_managed`] — what the renderer's status
/// stream carries for a packaged build (`use-updater.ts`'s `managed`
/// variant). Takes the flavour directly (not `Option`): every call site
/// already knows it is inside the packaged branch.
pub(super) fn managed_status(flavour: PackageFlavour) -> Value {
    json!({ "state": "managed", "by": flavour.as_wire_str() })
}

/// Refusal returned by `updater_download`/`updater_install` on a packaged
/// build. Defence in depth: the renderer never offers those actions once it
/// has seen the `managed` status, but an IPC caller could still invoke them,
/// and running the NSIS installer over a packaged install is exactly what
/// [`crate::platform`]'s packaged-build detection exists to prevent.
pub(super) fn store_managed_refusal(flavour: PackageFlavour) -> Value {
    let source = match flavour {
        PackageFlavour::MsStore => "the Microsoft Store",
        PackageFlavour::Snap => "the Snap Store",
    };
    json!({ "error": format!("This build is installed from {source} — updates are delivered by {source}.") })
}

/// The [`super::updater_status`] reply for a given [`UpdaterState`] — split out so
/// it is testable without a live `AppHandle` (this crate has no
/// `tauri::test` mock-app harness, same reason
/// [`super::download_in_progress_or_done`] and [`store_managed`] are split out).
///
/// A packaged (Store/Snap) build is reported via [`store_managed`]
/// BEFORE `pending_version` is even consulted — that state field never gets
/// set on such a build (`setup_auto_check` returns before the first
/// `silent_check` runs), so without this branch a packaged build reported
/// the same bare `{"available": false}` as "genuinely current"
/// (`B1-r2-ACLI-R6-2`). Otherwise, `state.checked` distinguishes "checked,
/// none available" from "never checked" / "last check failed" — both of the
/// latter used to be the identical, unfalsifiable `{"available": false}`.
// `pub(crate)` (T5 hardening) — `agent_call::proof`'s
// `extract_scalar_reads_updater_installs_real_pending_version_off_status_reply`
// feeds a real reply through this to cross-check `updater_install`'s POLICY
// proof source, so the two can never drift apart silently.
pub(crate) fn status_reply(state: &UpdaterState, flavour: Option<PackageFlavour>) -> Value {
    if let Some(managed) = flavour.map(store_managed) {
        return managed;
    }
    match &state.pending_version {
        Some(version) => json!({ "available": true, "version": version }),
        None => json!({ "available": false, "checked": state.checked }),
    }
}

#[cfg(test)]
mod tests;
