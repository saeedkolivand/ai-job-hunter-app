//! Split by topic (R8 relief -- redistributed from the crate-level `test.rs`, this unit's own
//! tests alone exceed the LOC cap): `dispatch_basic` covers `CallerClass::resolve` + the
//! plain pass-through verbs; `agent_query`/`agent_call`/`settings`/`document_export` each cover
//! one gated verb pair's `advance_authenticated` routing.

use super::*;

mod agent_call;
mod agent_query;
mod dispatch_basic;
mod document_export;
mod settings;

pub(super) fn state() -> (tempfile::TempDir, BridgeState) {
    let dir = tempfile::tempdir().unwrap();
    let state = BridgeState::load(dir.path());
    (dir, state)
}
