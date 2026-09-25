use super::*;
// ── source hygiene guard (mutation-visible: adding any of these turns
// this test red immediately) ────────────────────────────────────────────

#[test]
fn mcp_source_never_prints_or_pretty_prints() {
    for banned in ["println!(", "print!(", "to_string_pretty(", "eprintln!("] {
        assert!(
            !SOURCE.contains(banned),
            "mcp.rs must never call {banned} — see emit()'s own doc"
        );
    }
    assert_eq!(
        SOURCE.matches("stdout()").count(),
        1,
        "mcp.rs must call stdout() exactly once — see emit()'s own doc"
    );
}
