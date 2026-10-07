//! Source guard (#1352): the crate has no mock `AppHandle`, so no behavioural
//! test can run the real `analyze_job` -> `cover_letter` hand-off. This scans
//! the two stages for the two ends of it instead.
//!
//! `analyze_job` must publish the role (that is what starts the early research
//! lookup) and `cover_letter` must read the early brief before falling back to
//! inline research.
//!
//! Mutation check (executed): delete `ctx.publish_role();` from
//! `stages/analyze.rs` and the first case fails.

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
fn analyze_publishes_the_role_that_starts_the_early_research() {
    assert!(
        code_only("analyze.rs").contains("ctx.publish_role();"),
        "analyze.rs must call ctx.publish_role() after setting the analysis"
    );
}

#[test]
fn the_letter_stage_reads_the_early_brief_before_researching_inline() {
    let src = code_only("cover_letter.rs");
    let early = src
        .find("ctx.early_research")
        .expect("reads early_research");
    let inline = src
        .find("research_company_brief(completer,ctx).await")
        .expect("keeps the inline fallback");
    assert!(early < inline, "early brief must be tried first");
}
