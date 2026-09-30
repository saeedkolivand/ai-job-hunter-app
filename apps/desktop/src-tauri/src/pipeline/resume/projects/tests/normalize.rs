use super::super::*;
use super::support::seed;

// ── link_href ────────────────────────────────────────────────────────

#[test]
fn link_href_unwraps_a_markdown_span_and_passes_a_bare_url_through() {
    assert_eq!(
        link_href("[Website](https://example.com/app)"),
        "https://example.com/app"
    );
    assert_eq!(
        link_href("https://example.com/app"),
        "https://example.com/app"
    );
    // Malformed input (no closing paren) is returned trimmed, not panicked.
    assert_eq!(
        link_href("[Website](https://example.com"),
        "[Website](https://example.com"
    );
}

// ── normalize_projects: no-ops ──────────────────────────────────────

#[test]
fn empty_seeds_is_a_no_op() {
    let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n";
    assert_eq!(normalize_projects(draft, &[]), None);
}

#[test]
fn a_draft_with_no_projects_section_is_a_no_op() {
    let draft = "PROFESSIONAL SUMMARY\nA payments engineer.\n";
    let seeds = vec![seed(
        "Ledger CLI",
        &["https://github.com/janedoe/ledger"],
        &[],
        "",
    )];
    assert_eq!(normalize_projects(draft, &seeds), None);
}

// ── ordering + the never-delete guarantee ───────────────────────────

/// Draft order is preserved, and an entry with no matching seed survives
/// VERBATIM — never deleted, never invented over.
///
/// Mutation check: iterate `seeds` instead of the parsed draft entries in
/// `build` and this fails — the seed order ("Alpha" before "Beta" in
/// `seeds`) would come out ahead of the draft's own "Beta" before
/// "Alpha".
#[test]
fn draft_order_is_preserved_and_an_unmatched_entry_survives_verbatim() {
    let seeds = vec![
        seed("Alpha", &["https://github.com/janedoe/alpha"], &[], ""),
        seed("Beta", &["https://github.com/janedoe/beta"], &[], ""),
    ];
    let draft = "PROJECTS\n\n\
         **Beta** · https://github.com/janedoe/beta\n\n\
         **Ghost Project** · A project with no source link at all\n\n\
         **Alpha** · https://github.com/janedoe/alpha\n";
    let normalized = normalize_projects(draft, &seeds).expect("has a projects section");
    let beta_at = normalized.find("Beta").expect("beta kept");
    let ghost_at = normalized.find("Ghost").expect("ghost kept VERBATIM");
    let alpha_at = normalized.find("Alpha").expect("alpha kept");
    assert!(
        beta_at < ghost_at && ghost_at < alpha_at,
        "draft order (Beta, Ghost, Alpha) survives: {normalized}"
    );
    assert!(
        normalized.contains("A project with no source link at all"),
        "an entry with no matching seed is preserved VERBATIM, never deleted: {normalized}"
    );
}

/// An altered link is restored VERBATIM from the seed, not kept as the
/// model rewrote it.
#[test]
fn an_altered_link_is_restored_from_the_seed() {
    let seeds = vec![seed(
        "Ledger CLI",
        &["https://github.com/janedoe/ledger"],
        &[],
        "",
    )];
    let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/some-other-fork/ledger\n";
    let normalized = normalize_projects(draft, &seeds).unwrap();
    assert!(normalized.contains("https://github.com/janedoe/ledger"));
    assert!(!normalized.contains("some-other-fork"));
}

/// A link the model DROPPED is restored, because the identity fields come
/// back from the seed unconditionally.
#[test]
fn a_dropped_link_is_restored_from_the_seed() {
    let seeds = vec![seed(
        "Ledger CLI",
        &["https://github.com/janedoe/ledger"],
        &[],
        "",
    )];
    let draft = "PROJECTS\n\n**Ledger CLI**\n";
    let normalized = normalize_projects(draft, &seeds).unwrap();
    assert!(normalized.contains("https://github.com/janedoe/ledger"));
}

/// A description invented for a project the source says nothing else
/// about is dropped, not carried through.
#[test]
fn an_invented_description_on_a_dataless_seed_is_dropped() {
    let seeds = vec![seed(
        "Ledger CLI",
        &["https://github.com/janedoe/ledger"],
        &[],
        "", // no source description
    )];
    let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\
         A completely invented blurb about this project.\n";
    let normalized = normalize_projects(draft, &seeds).unwrap();
    assert!(
        !normalized.contains("invented blurb"),
        "a data-less seed must never gain a generated description"
    );
}

/// A renamed-but-same-link entry still matches its seed through the
/// canonical-link fallback, so the seed's identity (its ORIGINAL name)
/// survives rather than the entry being treated as unmatched.
#[test]
fn a_renamed_entry_still_matches_its_seed_by_link() {
    let seeds = vec![seed(
        "Ledger CLI",
        &["https://github.com/janedoe/ledger"],
        &[],
        "",
    )];
    let draft = "PROJECTS\n\n**Ledger Command Line Tool** · https://github.com/janedoe/ledger\n";
    let normalized = normalize_projects(draft, &seeds).unwrap();
    assert!(
        normalized.contains("Ledger CLI"),
        "matched by link, so the seed's own name is what renders: {normalized}"
    );
}

/// The seed's own markdown-labeled span round-trips through seeding and
/// rendering, and its href agrees with the bare form under
/// `canonical_link(link_href(..))` — the property that keeps a labeled
/// and a bare copy of the same link from ever being treated as two.
#[test]
fn a_labeled_seed_link_round_trips_and_its_href_is_link_href_stable() {
    let seeds = vec![seed(
        "Ledger CLI",
        &["[Website](https://example.com/ledger)"],
        &[],
        "",
    )];
    let draft = "PROJECTS\n\n**Ledger CLI**\n";
    let normalized = normalize_projects(draft, &seeds).unwrap();
    assert!(
        normalized.contains("[Website](https://example.com/ledger)"),
        "the rendered line must carry the label span verbatim: {normalized}"
    );
    assert_eq!(
        canonical_link(link_href("[Website](https://example.com/ledger)")),
        canonical_link("https://example.com/ledger"),
        "labeled and bare forms of the same URL must compare equal"
    );
}

/// A well-formed source normalizes fully: link restored, label preserved,
/// draft order kept, and a legitimate description survives untouched —
/// the positive case proving the feature still fires, not just the
/// negative/no-op guards around it.
#[test]
fn a_well_formed_source_normalizes_fully() {
    let seeds = vec![
        seed(
            "Ledger CLI",
            &["[Website](https://ledger.example.dev)"],
            &["Rust", "SQLite"],
            "A double-entry bookkeeping tool for small businesses.",
        ),
        seed(
            "CrossKit",
            &["https://github.com/janedoe/crosskit"],
            &[],
            "",
        ),
    ];
    let draft = "PROJECTS\n\n\
         **CrossKit** · https://an-altered-fork.example.com/crosskit\n\n\
         **Ledger CLI** · https://ledger.example.dev\n\
         A double-entry bookkeeping tool for small businesses.\n";
    let (normalized, stats) = normalize_projects_with_stats(draft, &seeds).unwrap();
    let crosskit_at = normalized.find("CrossKit").unwrap();
    let ledger_at = normalized.find("Ledger CLI").unwrap();
    assert!(
        crosskit_at < ledger_at,
        "draft order survives: {normalized}"
    );
    assert!(normalized.contains("https://github.com/janedoe/crosskit"));
    assert!(!normalized.contains("an-altered-fork"));
    assert!(normalized.contains("[Website](https://ledger.example.dev)"));
    assert!(normalized.contains("A double-entry bookkeeping tool for small businesses."));
    assert_eq!(stats.matched, 2);
    assert_eq!(stats.links_restored, 1, "only CrossKit's link was altered");
    // `render_project`'s separator glyph (`project_render::PROJECT_SEPARATOR`)
    // has no other pin now that the old `max_test.rs` is gone — the whole
    // suite passes even if this character changes underneath it. Pinned
    // here on both a name-to-link join (CrossKit, tier 3, bullet form)
    // and a multi-item stack join (Ledger CLI's `Rust`/`SQLite`).
    assert!(
        normalized.contains("• CrossKit · https://github.com/janedoe/crosskit"),
        "the project separator glyph is pinned: {normalized}"
    );
    assert!(
        normalized.contains("Rust · SQLite"),
        "the stack-join separator glyph is pinned: {normalized}"
    );
}

// ── C2 / all-verbatim: never a heading-only section ─────────────────

/// Every draft entry is unrelated to the one seed the source has — none
/// of them matches, so all stay verbatim and NOTHING was actually
/// restored: a genuine no-op, never a section spliced down to just its
/// heading.
#[test]
fn nothing_matched_is_a_no_op_not_a_heading_only_section() {
    let seeds = vec![seed(
        "Ledger CLI",
        &["https://github.com/janedoe/ledger"],
        &[],
        "",
    )];
    let draft = "PROJECTS\n\n**Totally Unrelated** · https://example.com/x\n";
    assert_eq!(normalize_projects(draft, &seeds), None);
}

// ── M1: the blank line to the NEXT section survives ──────────────────

#[test]
fn the_blank_line_before_the_next_section_survives_normalization() {
    let seeds = vec![seed(
        "Ledger CLI",
        &["https://github.com/janedoe/ledger"],
        &[],
        "",
    )];
    let draft =
        "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\nEDUCATION\n\nMSc.\n";
    let normalized = normalize_projects(draft, &seeds).unwrap();
    assert!(
        normalized.contains("ledger\n\nEDUCATION"),
        "a blank line must separate the normalized section from the next heading: {normalized:?}"
    );
    assert!(!normalized.contains("ledger\nEDUCATION"));
}

#[test]
fn no_trailing_blank_is_added_when_projects_is_the_last_section() {
    let seeds = vec![seed(
        "Ledger CLI",
        &["https://github.com/janedoe/ledger"],
        &[],
        "",
    )];
    let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n";
    let normalized = normalize_projects(draft, &seeds).unwrap();
    assert!(!normalized.ends_with("\n\n\n"));
}

// ── M2: dedup is counted, and an ambiguous link-rename is refused ────

#[test]
fn a_dedup_collision_is_counted_as_dropped() {
    let seeds = vec![seed(
        "Ledger CLI",
        &["https://github.com/janedoe/ledger"],
        &[],
        "",
    )];
    // Two entries answering to the SAME seed name.
    let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\n\
         **Ledger CLI** · https://github.com/janedoe/ledger\n";
    let (_, stats) = normalize_projects_with_stats(draft, &seeds).unwrap();
    assert_eq!(stats.matched, 1, "only the first survives");
    assert_eq!(stats.dropped, 1, "the second is a counted dedup drop");
}

/// Two SEEDS legitimately share one link (a monorepo's app and its docs
/// site). A draft entry that cannot resolve to either by name must not
/// guess — refused as ambiguous, so it stays VERBATIM. There is only one
/// entry in this draft and it is not matched, so `matched == 0`, and the
/// draft-side disagreement check also fires (the shared link IS present,
/// unattached, in the draft text) — the whole pass is a no-op either way.
#[test]
fn an_ambiguous_link_rename_is_refused_and_the_entry_stays_verbatim() {
    let shared = "https://github.com/janedoe/monorepo";
    let seeds = vec![
        seed("App", &[shared], &[], ""),
        seed("Docs", &[shared], &[], ""),
    ];
    let draft = "PROJECTS\n\n**Renamed Thing** · https://github.com/janedoe/monorepo\n";
    assert_eq!(normalize_projects(draft, &seeds), None);
}
