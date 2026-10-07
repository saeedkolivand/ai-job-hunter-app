//! Source guard (#1382): `QualityCtx` cannot be built without a live
//! `Completer`, so no behavioural test can run `analyze_job`/`strategy`. This
//! scans their source for the three pieces of the quality floor instead
//! (`floor::with_floor`'s own behaviour is unit-tested in `floor/tests.rs`):
//!
//! * the provider call goes through `with_floor` (the retry);
//! * the cache write goes through `store_sound` with `from_cache || degraded`
//!   as the skip flag (a floor miss is never cached);
//! * the cache READ filters on `below_floor` (a row poisoned before the floor
//!   existed is a miss);
//! * and no bare `cache::put(` bypasses `store_sound`.
//!
//! Mutation check (executed): delete each of the three in turn from either
//! stage (use plain `complete_json`; use `cache::put`; drop the `.filter`) and
//! the matching assertion fails.

use std::path::Path;

/// Drop `//` comments, then ALL whitespace, so rustfmt's line breaks cannot
/// hide a call and prose can never satisfy (or trip) the scan.
fn code_only(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/pipeline/resume/stages")
        .join(rel);
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| l.find("//").map_or(l, |i| &l[..i]).to_string())
        .flat_map(|l| l.split_whitespace().map(str::to_string).collect::<Vec<_>>())
        .collect()
}

#[test]
fn analyze_and_strategy_wire_the_quality_floor() {
    // (stage, read-filter, predicate + richness passed to with_floor, how
    // `degraded` is bound).
    for (stage, filter, args, binding) in [
        (
            "analyze.rs",
            ".filter(|a|!a.below_floor(",
            ["|a|a.below_floor(", "JobAnalysis::richness"],
            "(degraded,retried)=(floored.degraded,floored.retried);",
        ),
        (
            "strategy.rs",
            ".filter(|s|!s.below_floor(",
            ["ResumeStrategy::below_floor,", "ResumeStrategy::richness"],
            "letdegraded=strategy.below_floor();",
        ),
    ] {
        let code = code_only(stage);
        assert!(
            code.contains("with_floor("),
            "{stage}: the provider call must go through with_floor"
        );
        // The predicate and richness must be the with_floor call's own
        // arguments, not merely present somewhere in the file.
        let call = &code[code.find("=with_floor(").expect("with_floor call")..];
        let call = &call[..call.find(".await?;").expect("end of call")];
        for arg in args {
            assert!(call.contains(arg), "{stage}: with_floor is missing `{arg}`");
        }
        assert!(
            code.contains(binding),
            "{stage}: `degraded` must be `{binding}`"
        );
        assert!(
            code.contains("store_sound(ctx.cache,NAME,&key,&json,from_cache||degraded)"),
            "{stage}: the cache write must skip on `from_cache || degraded`"
        );
        let read = &code[code.find("cache::get::<").expect("cache read")..];
        let read = &read[..read.find("letfrom_cache=").expect("end of read")];
        assert!(
            read.contains(filter),
            "{stage}: the cache read must keep only rows that clear the floor (`{filter}`)"
        );
        assert!(
            !code.contains("cache::put("),
            "{stage}: a bare cache::put bypasses the floor"
        );
    }
}
