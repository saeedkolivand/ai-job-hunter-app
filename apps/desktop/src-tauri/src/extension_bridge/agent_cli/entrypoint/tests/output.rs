//! The exit-2 usage-error body's shape.

use super::super::*;

// ── exit-2 shape uniformity (MINOR fix — security review round 2) ──────

#[test]
fn usage_error_value_carries_a_null_resource_key_like_every_other_exit_2_reply() {
    // Before the fix this branch's JSON had no `resource` key at all
    // (`{"detail":…,"error":"usage","ok":false}`) while `emit_cli_error`
    // always emits one — a consumer reading `resource` unconditionally
    // on every exit-2 reply got a missing key specifically here.
    let v = usage_error_value("unknown verb (run `ajh-tauri agent --help`)");
    assert!(
        v.get("resource").is_some(),
        "usage-error JSON must carry a `resource` key (null is fine): {v}"
    );
    assert!(v["resource"].is_null());
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"], ERR_USAGE);
}
