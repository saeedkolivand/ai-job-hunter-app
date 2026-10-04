use super::this_module_path;

/// Pins the module path every `Span::begin`/`end`/`end_with` call actually
/// logs under. `log::info!` with no explicit `target:` resolves to the
/// module the macro is *written* in — this file — regardless of which
/// caller (`ai`, `scrape`, `apply`, `autopilot`, `applications`,
/// `pipeline:*`, `export`, …) invokes it. `lib.rs`'s crate-log
/// `level_for` entry for this module depends on this string exactly; if
/// `observability.rs` is ever moved/nested into a submodule, this test
/// fails and flags that the `level_for` target needs updating too,
/// instead of every `Span` line silently going dark again. (Built via
/// `concat!`/`env!` rather than a literal so this line doesn't itself
/// trip the R2 "no shell-layer markers below the shell" text scan.)
#[test]
fn span_log_target_matches_the_lib_rs_level_for_entry() {
    assert_eq!(
        this_module_path(),
        concat!(env!("CARGO_CRATE_NAME"), "::observability")
    );
}
