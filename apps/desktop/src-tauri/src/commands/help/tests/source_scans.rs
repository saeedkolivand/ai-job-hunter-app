// ── The production gate for the dense arm ────────────────────────────────────

/// The same deletion-guard shape as the eviction test below, for the same
/// reason: `help_search` needs a running Tauri app, so the ONE `if
/// semantic_on(&app)` that decides whether a question ever reaches a paid
/// embedding provider cannot be exercised from a unit test. What CAN be checked
/// is that the dense arm's only call site still sits inside that gate.
///
/// Without this, un-gating `run_dense` — the whole "semantic OFF makes zero
/// embed calls" property, and the default-install posture behind it — would
/// leave nothing red anywhere: every dense-arm test calls `run_dense_arm`
/// directly, below the gate.
#[test]
fn the_dense_arm_call_still_sits_inside_the_semantic_gate() {
    // `help.rs` holds the gate and `help/dense_arm.rs` the arm behind it; the
    // "only call site" count must cover both files.
    const HELP_SRC: &str = concat!(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands/help.rs")),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/commands/help/dense_arm.rs"
        )),
    );
    let start = HELP_SRC
        .find("let dense = if semantic_on(&app) {")
        .expect("`let dense = if semantic_on(&app) {` still guards the dense arm in help_search");
    let rest = &HELP_SRC[start..];
    let end = rest
        .find("} else {")
        .expect("the gate has an else branch (the `skipped` arm)");
    let gated = &rest[..end];
    assert!(
        gated.contains("run_dense(&app,"),
        "the dense arm must be called INSIDE the semantic_on gate; branch body was:\n{gated}"
    );
    assert_eq!(
        HELP_SRC.matches("run_dense(&app").count(),
        1,
        "and that must remain its ONLY call site — a second, ungated one would spend against \
         the provider with the preference off"
    );
}

// ── The eviction call site ───────────────────────────────────────────────────

/// A deletion guard, not a semantics proof: `ai_set_embedding_config` needs a
/// running Tauri app, so its body cannot be called from a unit test. What CAN
/// be checked is that the line still sits inside the `space_changed` branch
/// next to its two siblings — the same `include_str!` shape
/// `agent_cli::policy`'s exactness test uses for `lib.rs`.
///
/// Without this, dropping `clear_help_vectors()` from that branch would leave
/// every help vector stranded in a space nothing can read, and no test
/// anywhere would go red.
#[test]
fn the_embedding_space_change_branch_still_clears_the_help_vector_cache() {
    const AI_MOD: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/commands/ai/mod.rs"
    ));
    let start = AI_MOD
        .find("if space_changed {")
        .expect("`if space_changed {` still exists in ai_set_embedding_config");
    let rest = &AI_MOD[start..];
    // The branch's body ends at the success payload that follows it.
    let end = rest
        .find("json!(")
        .expect("the branch is followed by its json! reply");
    let branch = &rest[..end];
    for needle in [
        "clear_posting_vectors",
        "clear_match_scores",
        "clear_help_vectors",
    ] {
        assert!(
            branch.contains(needle),
            "`{needle}` must be evicted in the space-changed branch; branch body was:\n{branch}"
        );
    }
}
