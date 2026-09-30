use super::super::*;
use super::support::seed;

// ── L1/L2/L3: markdown link-span extraction (source.rs) ──────────────

#[test]
fn a_paren_containing_href_falls_back_to_the_bare_form_not_a_malformed_span() {
    let source = "PROJECTS\n\n**Rust wiki** · \
        [Rust](https://en.wikipedia.org/wiki/Rust_(programming_language))\n";
    let seeds = crate::pipeline::resume::source::seed_projects(source);
    assert_eq!(seeds.len(), 1);
    assert_eq!(seeds[0].links.len(), 1);
    let link = &seeds[0].links[0];
    assert!(
        !link.starts_with('['),
        "the round-trip/balance check must fail on a paren-truncated capture and fall \
         back to the bare href rather than writing the unbalanced markdown span \
         verbatim: {link:?}"
    );
}

#[test]
fn a_url_shaped_label_is_not_double_harvested() {
    let source =
        "PROJECTS\n\n**Site** · [https://other.example.com](https://real.example.com/app)\n";
    let seeds = crate::pipeline::resume::source::seed_projects(source);
    assert_eq!(seeds.len(), 1);
    assert_eq!(
        seeds[0].links.len(),
        1,
        "exactly one link, not the label harvested a second time: {:?}",
        seeds[0].links
    );
}

#[test]
fn a_scheme_less_www_label_survives() {
    let source = "PROJECTS\n\n**Site** · [Website](www.example.com/app)\n";
    let seeds = crate::pipeline::resume::source::seed_projects(source);
    assert_eq!(seeds.len(), 1);
    assert_eq!(seeds[0].links, vec!["[Website](www.example.com/app)"]);
}

/// N3: a SKIPPED span (its parens hold no single recognizable URL) must
/// not delete the bare URL that is genuinely present elsewhere on the
/// SAME line — only spans actually captured as links are stripped before
/// the bare-URL pass runs.
#[test]
fn a_skipped_span_does_not_delete_a_sibling_bare_url_on_the_same_line() {
    let source = "PROJECTS\n\n**Site** · [1](not a url at all) · https://github.com/janedoe/site\n";
    let seeds = crate::pipeline::resume::source::seed_projects(source);
    assert_eq!(seeds.len(), 1);
    assert!(
        seeds[0]
            .links
            .iter()
            .any(|l| l.contains("github.com/janedoe/site")),
        "the bare URL after a skipped, non-link bracket must still be harvested: {:?}",
        seeds[0].links
    );
}

// ── C1: the three reproduced corruption shapes are now no-ops ───────

/// **C1-a: plain titles + bulleted achievements.** Bullets satisfy
/// `project_entry_starts`, so a naive "any entry-start line" gate would
/// have passed this — but the swallowed plain titles never become their
/// own seed, so the ACHIEVEMENT BULLETS end up as fully-empty seeds
/// (no link, no stack, no description). The empty-seed whole-bail catches
/// it: no normalization at all, validators still grade the draft as-is.
#[test]
fn c1a_plain_titles_with_bulleted_achievements_disables_normalization() {
    let source = "PROJECTS\n\n\
        Ledger CLI\n\
        - Built a payment reconciliation tool\n\
        - Used Rust and SQLite\n\
        CrossKit\n\
        - An award-winning design system\n";
    let (seeds, reason) = seed_projects_for_normalize(source);
    assert!(seeds.is_empty(), "must disable normalization: {seeds:?}");
    assert_eq!(reason, Some("empty_seed"));

    // And end to end: a correct draft is left byte-for-byte untouched.
    let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\n\
         **CrossKit** · https://github.com/janedoe/crosskit\n";
    assert_eq!(normalize_projects(draft, &seeds), None);
}

/// **C1-b: bold titles + bullet achievements** (the max-depth-style
/// shape). Each bold title opens its OWN entry here (unlike C1-a, it is
/// not swallowed by the next bullet) — but that title-only entry then has
/// no link, no stack and no description of its own (its achievements are
/// bullets, which each open THEIR OWN entry instead of joining the
/// title's), so it is a fully-empty seed too. The SAME empty-seed
/// whole-bail that catches C1-a catches this shape as well.
#[test]
fn c1b_bold_titles_with_bullet_achievements_disables_normalization() {
    let source = "PROJECTS\n\n\
        **Ledger CLI**\n\
        - Built a payment reconciliation tool\n\
        - Used Rust and SQLite\n\
        **CrossKit**\n\
        - An award-winning design system\n";
    let (seeds, reason) = seed_projects_for_normalize(source);
    assert!(seeds.is_empty(), "must disable normalization: {seeds:?}");
    assert_eq!(reason, Some("empty_seed"));

    let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\n\
         **CrossKit** · https://github.com/janedoe/crosskit\n";
    assert_eq!(normalize_projects(draft, &seeds), None);
}

/// **C1-c: the DRAFT side has no entry boundary.** A plain-text draft
/// merges two projects into one draft "entry" — the second project's
/// link is present in the draft's Projects text but never attached to
/// its own recognized entry. The draft-side parse-disagreement bail
/// catches it: the unmatched seed's link is found unattached in the
/// draft text, so the WHOLE pass is skipped — the draft is returned
/// untouched, nothing merged or deleted.
#[test]
fn c1c_a_plain_text_draft_merging_two_projects_disables_normalization() {
    let seeds = vec![
        seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &[],
            "",
        ),
        seed(
            "CrossKit",
            &["https://github.com/janedoe/crosskit"],
            &[],
            "",
        ),
    ];
    // Plain text: no bold, no bullet — both projects glue into ONE
    // `entries()` group, so only "Ledger CLI" (the entry's own title)
    // can ever resolve; CrossKit's link is present but unattached.
    let draft = "PROJECTS\n\n\
        Ledger CLI\n\
        https://github.com/janedoe/ledger\n\
        CrossKit\n\
        https://github.com/janedoe/crosskit\n";
    let outcome = normalize_projects_outcome(draft, &seeds);
    assert!(
        matches!(
            outcome,
            ProjectsNormalizeOutcome::Skipped("draft_parse_disagreement")
        ),
        "an unattached-but-present seed link must disable the whole pass: {outcome:?}"
    );
    assert_eq!(normalize_projects(draft, &seeds), None);
}

// ── the mega-seed guard: a link surviving in description/stack ──────

/// **The residual mega-seed shape.** A 2-project plain-text source with
/// no bold/bullet anywhere collapses into ONE seed (no sibling to
/// compare against, so `seeds_are_plausible` is vacuous; the seed has a
/// name, a link AND a description, so the empty-seed bail does not fire
/// either) — but that single seed's swallowed SECOND title line
/// ("Beta Sync · <url>") leaks its own URL into the merged
/// `description`. A URL can never legitimately live in `description` or
/// `stack` (the locked signature puts links on the title line only, and
/// `seed_one_project` already strips a stack line's own URLs before they
/// ever reach `stack`), so its presence there is unambiguous evidence the
/// entry boundary swallowed a following project.
#[test]
fn a_link_surviving_in_the_description_disables_normalization() {
    let source = "PROJECTS\n\n\
        Ledger CLI\n\
        https://github.com/janedoe/ledger\n\
        Beta Sync · https://github.com/janedoe/beta\n\
        Go · gRPC\n";
    let (seeds, reason) = seed_projects_for_normalize(source);
    assert!(seeds.is_empty(), "must disable normalization: {seeds:?}");
    assert_eq!(reason, Some("link_in_description"));

    // End to end: a correct one-project draft is left byte-for-byte
    // untouched — no writing beta's URL onto the Ledger CLI entry.
    let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n";
    assert_eq!(normalize_projects(draft, &seeds), None);
}

/// **The negative case.** A single, honestly-formatted project (link ON
/// the title line — the locked signature — so it never leaks into the
/// description) still normalizes fully: the `link_in_description` guard
/// must not disable the feature for the ordinary, correct shape.
#[test]
fn an_honest_single_project_source_still_normalizes() {
    let source = "PROJECTS\n\n\
        Ledger CLI · https://github.com/janedoe/ledger\n\
        A bookkeeping tool for freelancers.\n";
    let (seeds, reason) = seed_projects_for_normalize(source);
    assert_eq!(
        reason, None,
        "an honest source must not be disabled: {seeds:?}"
    );
    assert!(!seeds.is_empty());

    let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\
        A bookkeeping tool for freelancers.\n";
    let outcome = normalize_projects_outcome(draft, &seeds);
    match outcome {
        ProjectsNormalizeOutcome::Applied(_, stats) => {
            assert_eq!(
                stats.matched, 1,
                "the feature still fires on an honest source"
            );
        }
        other => panic!("expected the feature to fire: {other:?}"),
    }
}

// ── N1: seeds_are_plausible ignores a truthful cross-reference ──────

#[test]
fn seeds_are_plausible_ignores_a_truthful_description_naming_a_sibling() {
    let plausible = vec![
        seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &[],
            "A bookkeeping tool — see also my CrossKit project for the design system side.",
        ),
        seed(
            "CrossKit",
            &["https://github.com/janedoe/crosskit"],
            &[],
            "",
        ),
    ];
    assert!(
        seeds_are_plausible(&plausible),
        "a truthful description naming a sibling by NAME must not switch normalization off"
    );
}

#[test]
fn seeds_are_plausible_still_catches_a_link_leaking_into_a_stack_line() {
    let contaminated = vec![
        seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &["Rust", "https://github.com/janedoe/crosskit"],
            "",
        ),
        seed(
            "CrossKit",
            &["https://github.com/janedoe/crosskit"],
            &[],
            "",
        ),
    ];
    assert!(!seeds_are_plausible(&contaminated));
}

// ── stats ────────────────────────────────────────────────────────────

#[test]
fn stats_report_matched_dropped_and_links_restored() {
    let seeds = vec![
        seed("Alpha", &["https://github.com/janedoe/alpha"], &[], ""),
        seed("Beta", &["https://github.com/janedoe/beta"], &[], ""),
    ];
    let draft = "PROJECTS\n\n\
         **Alpha** · https://altered.example.com/alpha\n\n\
         **Ghost** · https://example.com/ghost-with-no-seed\n\n\
         **Beta** · https://github.com/janedoe/beta\n";
    let (_, stats) = normalize_projects_with_stats(draft, &seeds).unwrap();
    assert_eq!(stats.matched, 2, "Alpha and Beta are kept");
    assert_eq!(stats.links_restored, 1, "only Alpha's link was altered");
}
