//! `export_reply_unless_revoked` (the `document.export` mid-compile revoke race) —
//! redistributed from the crate-level `test.rs` (R8 relief).

use super::*;

use super::super::test_support::bridge_state;

#[tokio::test]
async fn export_reply_unless_revoked_discards_the_reply_when_the_token_rotates_mid_export() {
    // Models "hold the export, rotate the token": the export future itself rotates `state`
    // (standing in for a concurrent `Settings → Regenerate` mutation) before resolving — this
    // loop's own inline `.await` cannot observe the rotation any other way (see the fn's doc).
    let (_dir, state) = bridge_state();
    state.inc_connected(); // an authenticated connection is the only caller of this arm

    let reply = export_reply_unless_revoked(&state, async {
        state.regenerate_token();
        "document.result payload".to_string()
    })
    .await;

    assert_eq!(
        reply, None,
        "a rotation landing mid-export must discard the reply, never send it"
    );
}

#[tokio::test]
async fn export_reply_unless_revoked_passes_the_reply_through_when_the_epoch_is_unchanged() {
    let (_dir, state) = bridge_state();
    state.inc_connected();

    let reply =
        export_reply_unless_revoked(&state, async { "document.result payload".to_string() }).await;

    assert_eq!(
        reply,
        Some("document.result payload".to_string()),
        "no rotation happened — the real export reply must still be sent"
    );
}
