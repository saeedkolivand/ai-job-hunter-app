//! Source guard (#1351): the crate has no mock `AppHandle`, so no behavioural
//! test can observe which `Completer` method `repair` and `humanize` call.
//! This scans their source instead.
//!
//! Both are mechanical stages: they must send `QualityCtx::stage_effort` (the
//! user's effort, else the model's cheapest tier) with every provider call:
//! `repair` through `Completer::complete_with_effort`, `humanize` through its
//! one structured `complete_json` call (line patches). A plain `.complete(`
//! silently drops the effort again, which is the defect #1351 fixed.
//!
//! Mutation check: pass `None` instead of `env.effort` to `complete_json` in
//! `stages/humanize/doc.rs` and the humanize assertions fail.

use std::path::Path;

/// Drop `//` comments, then ALL whitespace, so rustfmt's line breaks cannot
/// hide a call and prose can never satisfy (or trip) the scan. Several files
/// are concatenated for a stage whose provider call lives in a sub-module.
fn code_only(rels: &[&str]) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/pipeline/resume/stages");
    rels.iter()
        .flat_map(|rel| {
            std::fs::read_to_string(path.join(rel))
                .unwrap()
                .lines()
                .map(|l| l.find("//").map_or(l, |i| &l[..i]).to_string())
                .collect::<Vec<_>>()
        })
        .flat_map(|l| l.split_whitespace().map(str::to_string).collect::<Vec<_>>())
        .collect()
}

#[test]
fn repair_and_humanize_send_the_stage_effort_and_never_a_plain_complete() {
    // `repair` makes one plain-text call through `complete_with_effort`.
    let repair = code_only(&["repair.rs"]);
    assert_eq!(repair.matches(".complete_with_effort(").count(), 1);
    assert!(!repair.contains(".complete("));
    assert!(repair.contains("stage_effort("));

    // `humanize` has TWO provider call sites in `humanize/doc.rs`, shared by
    // the résumé and the letter: the structured line-patch call
    // (`complete_json`) and the whole-document rewrite (`complete_with_effort`,
    // for documents flagged only document-wide). Each must carry the stage
    // effort as its last argument.
    let humanize = code_only(&["humanize.rs", "humanize/doc.rs"]);
    assert_eq!(
        humanize.matches(".complete_json(").count(),
        1,
        "humanize: exactly one patch call site"
    );
    assert_eq!(
        humanize.matches(".complete_with_effort(").count(),
        1,
        "humanize: exactly one whole-document call site"
    );
    assert!(
        humanize.contains("Some(&humanize_patch_schema()),env.effort,)"),
        "humanize: the patch call must carry the stage effort"
    );
    assert!(
        humanize.contains("None,env.effort,)"),
        "humanize: the whole-document call must carry the stage effort"
    );
    // `complete_with_effort` does not charge the per-provider daily ceiling
    // (`complete_json` does), so every such call must be preceded by the charge.
    assert_eq!(
        humanize
            .matches("charge_daily()?;env.completer.complete_with_effort(")
            .count(),
        humanize.matches(".complete_with_effort(").count(),
        "humanize: every complete_with_effort must follow a charge_daily()"
    );
    assert!(
        !humanize.contains(".complete("),
        "humanize: a plain `.complete(` drops the effort"
    );
    assert!(humanize.contains("effort=ctx.stage_effort("));
    assert_eq!(humanize.matches("humanize_doc(&env,").count(), 2);
}
