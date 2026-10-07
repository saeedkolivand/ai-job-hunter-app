//! Source guard (#1351): the crate has no mock `AppHandle`, so no behavioural
//! test can observe which `Completer` method `repair` and `humanize` call.
//! This scans their source instead.
//!
//! Both are mechanical stages: they must send `QualityCtx::stage_effort` (the
//! user's effort, else the model's cheapest tier) through
//! `Completer::complete_with_effort`. A plain `.complete(` silently drops the
//! effort again, which is the defect #1351 fixed.
//!
//! Mutation check (executed): change one `complete_with_effort(` in
//! `stages/humanize.rs` back to `complete(` and the matching case fails.

use std::path::Path;

/// Drop `//` comments, then ALL whitespace, so rustfmt's line breaks cannot
/// hide a call and prose can never satisfy (or trip) the scan.
fn code_only(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/pipeline/resume/stages");
    std::fs::read_to_string(path.join(rel))
        .unwrap()
        .lines()
        .map(|l| l.find("//").map_or(l, |i| &l[..i]))
        .flat_map(str::split_whitespace)
        .collect()
}

#[test]
fn repair_and_humanize_send_the_stage_effort_and_never_a_plain_complete() {
    // (file, how many provider calls the stage makes)
    for (file, calls) in [("repair.rs", 1), ("humanize.rs", 2)] {
        let src = code_only(file);
        assert_eq!(
            src.matches(".complete_with_effort(").count(),
            calls,
            "{file}: every provider call must go through complete_with_effort"
        );
        assert!(
            !src.contains(".complete("),
            "{file}: a plain `.complete(` drops the effort"
        );
        assert!(
            src.contains("stage_effort("),
            "{file}: the effort must come from QualityCtx::stage_effort"
        );
    }
}
