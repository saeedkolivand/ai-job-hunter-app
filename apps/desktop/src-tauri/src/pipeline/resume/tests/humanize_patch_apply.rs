use super::super::stages::{apply_patches, excerpt_within, flagged_lines, Patch};
use super::support::voice_report;

fn patch(id: usize, replacement: &str) -> Patch {
    Patch {
        id,
        replacement: replacement.to_string(),
    }
}

#[test]
fn apply_patches_touches_only_flagged_ids_and_ignores_unknown_ones() {
    let doc =
        "SUMMARY\nA Robust engineer.\n- Cut p95 latency by 40% with a robust cache\n- Wrote docs\n";
    let lines = flagged_lines(&voice_report(&["robust"]), doc, "en").lines;
    let out = apply_patches(
        doc,
        &lines,
        &[
            patch(2, "A dependable engineer."),
            patch(4, "- Rewrote the docs"), // context line, not flagged
            patch(99, "ghost"),             // unknown id
            patch(0, "ghost"),
            patch(3, "Cut p95 latency by 40% with a plain cache"),
        ],
    );
    assert_eq!(
        out,
        "SUMMARY\nA dependable engineer.\n- Cut p95 latency by 40% with a plain cache\n- Wrote docs\n",
        "bullet kept, context line and unknown ids untouched, trailing newline kept"
    );
}

#[test]
fn apply_patches_rejects_unsafe_replacements_and_keeps_the_original_line() {
    let doc = "SUMMARY\nShipped 3 robust services\n";
    let lines = flagged_lines(&voice_report(&["robust"]), doc, "en").lines;
    for bad in [
        "",
        "   ",
        "Shipped 3 services\nand more", // multi-line
        "Shipped 4 plain services",     // number changed
        "Shipped plain services",       // number dropped
        "<humanize_document>Shipped 3 plain services",
    ] {
        assert_eq!(
            apply_patches(doc, &lines, &[patch(2, bad)]),
            doc,
            "{bad:?} must be rejected"
        );
    }
    assert_eq!(
        apply_patches(doc, &lines, &[patch(2, "Shipped 3 plain services")]),
        "SUMMARY\nShipped 3 plain services\n"
    );
}

#[test]
fn the_first_patch_for_an_id_wins_even_when_it_is_rejected() {
    let doc = "A robust line\nShipped 3 robust things\n";
    let lines = flagged_lines(&voice_report(&["robust"]), doc, "en").lines;
    let out = apply_patches(
        doc,
        &lines,
        &[patch(1, "A plain line"), patch(1, "Another")],
    );
    assert_eq!(out, "A plain line\nShipped 3 robust things\n");
    // Rejected first (number changed), valid second: the original stays.
    let out = apply_patches(
        doc,
        &lines,
        &[
            patch(2, "Shipped 4 plain things"),
            patch(2, "Shipped 3 plain things"),
        ],
    );
    assert_eq!(out, doc);
}

#[test]
fn bold_is_not_a_bullet_and_a_heading_keeps_its_marker() {
    let doc = "**Robust** platform\n## Robust summary\n";
    let lines = flagged_lines(&voice_report(&["robust"]), doc, "en").lines;
    let out = apply_patches(
        doc,
        &lines,
        &[
            patch(1, "**Dependable** platform"),
            patch(2, "Dependable summary"),
        ],
    );
    assert_eq!(out, "**Dependable** platform\n## Dependable summary\n");
    let echoed = apply_patches(doc, &lines, &[patch(2, "## Dependable summary")]);
    assert_eq!(echoed, "**Robust** platform\n## Dependable summary\n");
}

/// Mutation check: remove the budget test in `excerpt_within` and the
/// dropped-id assertions fail.
#[test]
fn an_over_budget_flagged_line_is_not_offered_and_never_truncated() {
    let long = format!("{} robust tail", "word ".repeat(40));
    let doc = format!(
        "A robust start
ctx a
ctx b
ctx c
{long}
ctx d
Short robust end
"
    );
    let lines = flagged_lines(&voice_report(&["robust"]), &doc, "en").lines;
    assert_eq!(
        lines.iter().map(|l| l.id).collect::<Vec<_>>(),
        vec![1, 5, 7]
    );
    // The budget fits the short lines (with context) but not the long one.
    let (text, shown) = excerpt_within(&doc, &lines, 90);
    let ids: Vec<usize> = shown.iter().map(|l| l.id).collect();
    assert_eq!(ids, vec![1, 7], "the over-budget line is not offered");
    assert!(text.chars().count() <= 90);
    assert!(
        !text.contains("tail") && !text.contains("word"),
        "never shown truncated"
    );
    // A patch for the excluded id is ignored by apply_patches.
    let out = apply_patches(&doc, &shown, &[patch(5, "cut")]);
    assert_eq!(out, doc);
}

#[test]
fn only_one_echoed_bullet_is_stripped_not_a_bold_opener() {
    let doc = "* Robust platform\n";
    let lines = flagged_lines(&voice_report(&["robust"]), doc, "en").lines;
    assert_eq!(
        apply_patches(doc, &lines, &[patch(1, "**Dependable** platform")]),
        "* **Dependable** platform\n"
    );
    assert_eq!(
        apply_patches(doc, &lines, &[patch(1, "* Dependable platform")]),
        "* Dependable platform\n"
    );
}
