//! Coverage test pinning every literal fence tag against `EMITTED_FENCE_TAGS` (`fence.rs`).

use super::super::reshape::EMITTED_FENCE_TAGS;

/// Every literal tag string passed directly as the first argument to `prompt_fence::fenced(` or
/// `prompt_fence::strip_fence_wrapper(` in `src` (multi-line calls included). A variable-typed
/// first argument (e.g. `fence.rs`'s own `tag` local for the title/body block) contributes
/// nothing here — see this test's own doc for why that is still sound today.
fn literal_fence_tags(src: &str) -> Vec<String> {
    const CALLS: [&str; 2] = [
        "crate::prompt_fence::fenced(",
        "crate::prompt_fence::strip_fence_wrapper(",
    ];
    let mut tags = Vec::new();
    for call in CALLS {
        let mut pos = 0usize;
        while let Some(rel) = src[pos..].find(call) {
            let after = pos + rel + call.len();
            let rest = src[after..].trim_start();
            if let Some(stripped) = rest.strip_prefix('"') {
                if let Some(end) = stripped.find('"') {
                    tags.push(stripped[..end].to_string());
                }
            }
            pos = after;
        }
    }
    tags
}

/// `EMITTED_FENCE_TAGS` is a HAND-WRITTEN list whose own doc instructs "update THIS list ... the
/// moment a new `fenced(...)`/`strip_fence_wrapper(...)` literal tag is added anywhere in
/// `fence.rs` or this module" — but nothing enforced that instruction, and the only consumer
/// (`mcp::tests::instructions_documents_every_fence_tag_this_surface_emits`) iterates the SAME
/// list, so a stale entry there could never be caught by it. This scans `fence.rs` + `reshape.rs`
/// for every literal tag passed directly to `fenced(`/`strip_fence_wrapper(` and `assert_eq!`s
/// the derived set against `EMITTED_FENCE_TAGS`, the same discipline
/// `EXPECTED_RESOLVED_WRAPPER_ARGS`/`EXPECTED_UNCATALOGUED` already use elsewhere in this crate
/// (asserted against a DERIVED set, not merely iterated).
///
/// A variable-typed first argument (fence.rs's `tag` local, used for the title/body/text blocks)
/// is invisible to this scan — every value it can hold happens to ALSO appear as a direct
/// literal call elsewhere in these two files today (`"job_posting"`/`"app_notification"` at
/// fence.rs's own by-name loop's default and reshape.rs's several direct calls;
/// `"user_document"` at reshape.rs's own `fence_user_document_bare_text`), so the derived set
/// below is complete for the current source. A fence tag introduced ONLY through a variable, with
/// no direct literal call anywhere in either file, would not be caught by this test — a full
/// data-flow trace is the upgrade path, not attempted here since every real addition to this
/// surface so far has started as a direct literal call.
#[test]
fn fence_rs_and_reshape_rs_literal_tags_match_emitted_fence_tags() {
    // Scans every file `fence.rs`/`reshape.rs` were split into under the R8 LOC cap (issue #1280)
    // — `concat!` of absolute `CARGO_MANIFEST_DIR`-rooted paths, never a relative `include_str!`
    // of just the two entry files, which would silently stop scanning the split-out bodies where
    // the actual `fenced(`/`strip_fence_wrapper(` calls now live.
    const SRC: &str = concat!(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/fence.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/fence/named_fields.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/fence/shape_helpers.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/fence/shape_tables.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/fence/tables.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/reshape.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/reshape/base64.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/reshape/contact_profile.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/reshape/drop_fields.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/reshape/paging.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/reshape/scalar_fence.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/extension_bridge/agent_call/reshape/unfence.rs"
        )),
    );
    let mut found: Vec<String> = literal_fence_tags(SRC);
    found.sort();
    found.dedup();

    let mut expected: Vec<&str> = EMITTED_FENCE_TAGS.to_vec();
    expected.sort_unstable();

    assert_eq!(
        found, expected,
        "fence.rs/reshape.rs's own literal fenced(...)/strip_fence_wrapper(...) tag arguments \
         drifted from EMITTED_FENCE_TAGS — update that const (and mcp::INSTRUCTIONS if the tag \
         is genuinely new)"
    );
}

// ── PR1 — extension read tier: the extension's own `agent.call` gate ──────────────────────────
