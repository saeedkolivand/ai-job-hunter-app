//! `mcp` is a MODE, never a `VERB_TABLE` row, so the drift-loop tests in
//! `super::super::verb::tests` cannot see it — these are its own dispatch-order
//! tests.

use super::super::*;
use crate::extension_bridge::agent_cli::tests::support::s;

// ── mcp mode dispatch (owner request — `mcp` is a MODE, never a VERB_TABLE
// row, so the drift-loop tests above cannot see it) ─────────────────────

#[test]
fn help_text_mentions_the_mcp_mode() {
    assert!(help_text().contains("mcp"));
}

#[test]
fn help_text_indents_the_mcp_row_like_every_verb_row() {
    // LOW fix, review round 3 — the `\` line-continuation used to build this row strips ALL
    // leading whitespace off the continued line, so "mcp [...]" rendered flush-left instead of
    // indented two spaces like every VERB_TABLE row.
    let text = help_text();
    let mcp_line = text
        .lines()
        .find(|l| l.trim_start().starts_with("mcp "))
        .expect("must have an mcp row");
    assert!(
        mcp_line.starts_with("  mcp "),
        "the mcp row must be indented like every verb row: {mcp_line:?}"
    );
}

#[test]
fn is_mcp_mode_matches_only_the_exact_first_token() {
    assert!(is_mcp_mode(&s(&["mcp"])));
    assert!(is_mcp_mode(&s(&["mcp", "--allow-irreversible"])));
    assert!(!is_mcp_mode(&s(&["mcpx"])));
    assert!(!is_mcp_mode(&s(&["MCP"])));
    assert!(!is_mcp_mode(&s(&["--help", "mcp"])));
    assert!(!is_mcp_mode(&s(&[])));
}

#[test]
fn help_wins_over_mcp_when_help_is_the_first_token() {
    // `agent --help mcp` must be a help request, never mcp mode — `run()`
    // checks `is_help_request` FIRST, so the first-token-only check on
    // `is_mcp_mode` is what makes that ordering actually matter here.
    assert!(is_help_request(&s(&["--help", "mcp"])));
    assert!(!is_mcp_mode(&s(&["--help", "mcp"])));
}
