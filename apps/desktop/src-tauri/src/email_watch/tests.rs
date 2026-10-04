//! Store tests for [`EmailWatchStore`] — the invariant lock for PR A's
//! foundation (no poller/parser/matcher exists yet; these tests lock the
//! schema/API contract those will build on in PR B).
//!
//! Split by topic: connect/disconnect, defaults, the opt-in toggles and the factory-reset wipe
//! ([`connection`]); the concurrent-clear guard on every trailing write ([`concurrent_clear`]);
//! and the `seen` dedupe plus the UID watermark ([`watermark`]). [`support`] holds the fixtures
//! the sibling modules' tests share.

use tempfile::TempDir;

use super::{EmailWatchAccount, EmailWatchStore, CREDENTIAL_SLOT};

mod concurrent_clear;
mod connection;
pub(in crate::email_watch) mod support;
mod watermark;

fn new_store() -> (TempDir, EmailWatchStore) {
    let dir = TempDir::new().unwrap();
    let store = EmailWatchStore::open(&dir.path().to_path_buf()).expect("open store");
    (dir, store)
}

/// A store with `a@gmail.com` already connected (the common precondition).
fn connected_store() -> (TempDir, EmailWatchStore) {
    let (dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
    (dir, store)
}
