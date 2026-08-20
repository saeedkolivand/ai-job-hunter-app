use super::*;

#[test]
fn test_parse_inline_md() {
    let segments = parse_inline_md("Use **React** and **TypeScript** here");
    assert_eq!(segments.len(), 5);
    assert!(!segments[0].bold);
    assert!(segments[1].bold);
    assert_eq!(segments[1].text, "React");
}

#[test]
fn test_company_name_not_section() {
    let line = parse_line("NASA ENGINEER", 1, &["Name", "NASA ENGINEER"]);
    assert!(!matches!(line.kind, LineKind::SectionHeader));
}

#[test]
fn test_numbered_bullet() {
    let line = parse_line("1. First bullet point", 5, &[]);
    assert!(matches!(line.kind, LineKind::Bullet));
}

#[test]
fn test_strip_md() {
    assert_eq!(strip_md("**bold**"), "bold");
    assert_eq!(strip_md("# Heading"), "Heading");
    assert_eq!(strip_md("#Heading#"), "Heading");
    assert_eq!(strip_md("## Section"), "Section");
    assert_eq!(strip_md("normal text"), "normal text");
}

#[test]
fn test_section_header_detection() {
    let line = parse_line("work experience", 5, &[]);
    assert!(matches!(line.kind, LineKind::SectionHeader));
}

#[test]
fn test_all_caps_section() {
    let line = parse_line("EXPERIENCE", 5, &[]);
    assert!(matches!(line.kind, LineKind::SectionHeader));
}

#[test]
fn test_bullet_detection() {
    let line = parse_line("• First point", 5, &[]);
    assert!(matches!(line.kind, LineKind::Bullet));
}

#[test]
fn test_job_entry_detection() {
    let line = parse_line("Software Engineer  Jan 2020 - Present", 5, &[]);
    assert!(matches!(line.kind, LineKind::JobEntry));
}

#[test]
fn test_contact_detection() {
    let line = parse_line("john@example.com", 5, &[]);
    assert!(matches!(line.kind, LineKind::Contact));
}

#[test]
fn is_contact_shaped_matches_ts_is_header_contact_line_fixture() {
    // Cross-language parity guard: this exact fixture is also asserted by the TS
    // isHeaderContactLine() / isFirstLineContactShaped() tests in
    // packages/prompts/src/generate/text/header-contact-line.test.ts. Both read
    // the same file, so the two implementations can never silently drift — see
    // docs/knowledge (item H, header-seeding) for why this matters: a divergence
    // here either lets a leaked link survive re-seeding unrecognised, or
    // duplicates the seeded line on regeneration.
    #[derive(serde::Deserialize)]
    struct Case {
        line: String,
        contact: bool,
        #[serde(rename = "firstLine")]
        first_line: bool,
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/prompts/src/fixtures/header-contact-line.json");
    let raw = std::fs::read_to_string(&path).expect(
        "read header-contact-line parity fixture \
         (packages/prompts/src/fixtures/header-contact-line.json)",
    );
    let cases: Vec<Case> =
        serde_json::from_str(&raw).expect("parse header-contact-line parity fixture");

    assert!(
        !cases.is_empty(),
        "header-contact-line parity fixture must not be empty"
    );
    for c in &cases {
        // `parse_line` never calls `is_contact_shaped` on the raw line — only
        // on `clean = strip_md(trimmed)`. Doing the same here is what makes
        // this a real end-to-end parity check against a `**bold**`- or
        // `#`-decorated line, not just an isolated-function coincidence: the
        // TS mirror applies its own equivalent stripping internally now too.
        let clean = strip_md(c.line.trim());
        assert_eq!(
            is_contact_shaped(&clean),
            c.contact,
            "is_contact_shaped drift for {:?} (clean: {:?})",
            c.line,
            clean
        );
        assert_eq!(
            is_first_line_contact_shaped(&clean),
            c.first_line,
            "is_first_line_contact_shaped drift for {:?} (clean: {:?})",
            c.line,
            clean
        );
    }
}

#[test]
fn section_names_exactly_matches_ts_known_section_names_fixture() {
    // Cross-language parity guard, same shape as the contact-line fixture
    // above, but for one of the TWO predicates that gate the renderer's
    // header-seeding scan boundary (`isKnownSectionName` in
    // packages/prompts/src/generate/text/header-contact-line.ts — the other
    // is `isAllCapsSectionHeading`, tested below). Asserted both ways
    // (fixture ⊆ SECTION_NAMES and SECTION_NAMES ⊆ fixture) so extending
    // either list without the other fails immediately. Covers all 7 locales
    // `packages/prompts/src/locale/index.ts`'s `CONVENTIONS` ships résumé
    // headers for (en/de/fr/es/it/nl/pt) — a résumé generated for any of them
    // whose model wrote a Title-Case (not ALL-CAPS) heading still stops the
    // scan here. TS doesn't hold a second copy of this list at all — it
    // imports the fixture directly as its runtime data — so only this
    // direction can ever drift.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/prompts/src/fixtures/section-names.json");
    let raw = std::fs::read_to_string(&path).expect(
        "read section-names parity fixture (packages/prompts/src/fixtures/section-names.json)",
    );
    let fixture: Vec<String> =
        serde_json::from_str(&raw).expect("parse section-names parity fixture");

    assert!(
        !fixture.is_empty(),
        "section-names parity fixture must not be empty"
    );
    let fixture_set: std::collections::BTreeSet<&str> =
        fixture.iter().map(String::as_str).collect();
    let rust_set: std::collections::BTreeSet<&str> = SECTION_NAMES.iter().copied().collect();
    // Set equality alone silently absorbs a duplicate entry on either side
    // (a name authored twice collapses to one element and the comparison
    // below would still pass) — catch that separately so a duplicate is a
    // real, loud failure rather than a no-op.
    assert_eq!(
        fixture.len(),
        fixture_set.len(),
        "section-names fixture must not contain a duplicate entry"
    );
    assert_eq!(
        SECTION_NAMES.len(),
        rust_set.len(),
        "SECTION_NAMES must not contain a duplicate entry"
    );
    assert_eq!(
        fixture_set, rust_set,
        "SECTION_NAMES and the shared fixture must contain exactly the same names"
    );
}

#[test]
fn is_all_caps_section_heading_matches_ts_fixture() {
    // Cross-language parity guard for the shape-based (not list-based)
    // heading predicate — this is what recognizes a locale's own ALL-CAPS
    // heading (the résumé prompt mandates ALL-CAPS section titles) without a
    // per-locale word list, and what an unfixtured/unrecognised locale falls
    // back to when it isn't literally in SECTION_NAMES. A previous version of
    // this predicate was deleted from the TS mirror without a fixture gate,
    // which silently broke header-seeding for es/it/nl/pt résumés (and any
    // en résumé whose first heading — "PROFESSIONAL EXPERIENCE", "KEY
    // ACHIEVEMENTS" — isn't literally in SECTION_NAMES either); restoring it
    // WITHOUT this gate would be the same mistake again.
    #[derive(serde::Deserialize)]
    struct Case {
        line: String,
        heading: bool,
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/prompts/src/fixtures/all-caps-headings.json");
    let raw = std::fs::read_to_string(&path).expect(
        "read all-caps-headings parity fixture \
         (packages/prompts/src/fixtures/all-caps-headings.json)",
    );
    let cases: Vec<Case> =
        serde_json::from_str(&raw).expect("parse all-caps-headings parity fixture");

    assert!(
        !cases.is_empty(),
        "all-caps-headings parity fixture must not be empty"
    );
    for c in &cases {
        // Same reasoning as the contact-line fixture above: `parse_line`
        // always runs this predicate on `strip_md(trimmed)`, never the raw
        // line, so the parity check must too.
        let clean = strip_md(c.line.trim());
        assert_eq!(
            is_all_caps_section_heading(&clean),
            c.heading,
            "is_all_caps_section_heading drift for {:?} (clean: {:?})",
            c.line,
            clean
        );
    }
}

#[test]
fn test_job_title_detection() {
    let lines = vec!["Software Engineer  Jan 2020 - Present", "Senior Developer"];
    let line = parse_line("Senior Developer", 1, &lines);
    assert!(matches!(line.kind, LineKind::JobTitle));
}

#[test]
fn test_multilingual_sections() {
    let line = parse_line("berufserfahrung", 5, &[]);
    assert!(matches!(line.kind, LineKind::SectionHeader));
}

// ── German/Italian heading recogniser gap (Projekte / Progetti) ────────────

/// The exact reported bug: a Title-Case German "Projekte" heading (not
/// ALL-CAPS, so `is_all_caps_section_heading` cannot save it) must render as
/// a SectionHeader, not fall through to body Text.
#[test]
fn german_title_case_projekte_is_section_header() {
    let line = parse_line("Projekte", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "Title-Case 'Projekte' must be a SectionHeader, got {:?}",
        line.kind
    );
}

/// The other three German headings the producer can now emit and the
/// recogniser previously lacked.
#[test]
fn german_title_case_new_headings_are_section_headers() {
    for heading in ["Zertifikate", "Auszeichnungen", "Publikationen"] {
        let line = parse_line(heading, 5, &[]);
        assert!(
            matches!(line.kind, LineKind::SectionHeader),
            "{heading:?} must be a SectionHeader, got {:?}",
            line.kind
        );
    }
}

/// Italian is the other priority locale named in the bug report (a real user
/// works in Italy) — its five new headings must all recognise too.
#[test]
fn italian_title_case_new_headings_are_section_headers() {
    for heading in [
        "Progetti",
        "Certificazioni",
        "Lingue",
        "Riconoscimenti",
        "Pubblicazioni",
    ] {
        let line = parse_line(heading, 5, &[]);
        assert!(
            matches!(line.kind, LineKind::SectionHeader),
            "{heading:?} must be a SectionHeader, got {:?}",
            line.kind
        );
    }
}

/// pt-PT "Prémios" is what the producer actually emits; pt-BR "Prêmios" is
/// accepted too even though the producer never generates it — nothing in the
/// producer's header table discriminates the two spellings.
#[test]
fn portuguese_awards_both_spellings_recognised() {
    for heading in ["Prémios", "Prêmios"] {
        let line = parse_line(heading, 5, &[]);
        assert!(
            matches!(line.kind, LineKind::SectionHeader),
            "{heading:?} must be a SectionHeader, got {:?}",
            line.kind
        );
    }
}

// ── Combined "X & Y" headings (the "Ausbildung & Sprachen" half of the bug) ─

/// The other reported symptom: a merged heading where BOTH halves are
/// individually known must still render as a heading rather than as an
/// unstyled paragraph — the producer forbids emitting this shape going
/// forward, but this covers already-generated documents and any
/// non-compliant generation.
#[test]
fn combined_ampersand_heading_both_known_is_section_header() {
    let line = parse_line("Ausbildung & Sprachen", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader for a combined heading with both known halves, got {:?}",
        line.kind
    );
}

/// English combined heading — the join logic is not German-specific.
#[test]
fn combined_ampersand_heading_english_is_section_header() {
    let line = parse_line("Skills & Certifications", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
}

/// The join must NOT recognise a heading when only one half is known, or
/// neither is — an arbitrary "X & Y" prose line stays Text.
#[test]
fn combined_ampersand_heading_requires_both_halves_known() {
    let one_known = parse_line("Projekte & Craft Beer", 5, &[]);
    assert!(
        !matches!(one_known.kind, LineKind::SectionHeader),
        "one known half must NOT be enough, got {:?}",
        one_known.kind
    );
    let neither_known = parse_line("Beer & Wine", 5, &[]);
    assert!(
        !matches!(neither_known.kind, LineKind::SectionHeader),
        "neither half known must stay non-heading, got {:?}",
        neither_known.kind
    );
}

// ── Blast radius: the new vocabulary must not fire on ordinary content ─────

/// A company literally named after a new section word, in JobEntry shape,
/// must stay a JobEntry — the exact-match test requires the WHOLE line to
/// equal the section word, so trailing text (here the date column) already
/// prevents a false positive; this pins that down for the newly added words.
#[test]
fn company_named_after_new_section_word_stays_job_entry() {
    let line = parse_line("Projekte GmbH  2020 - Present", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry, got {:?}",
        line.kind
    );
}

/// A prose bullet that STARTS with a new section word must stay a Bullet —
/// the whole line, not a prefix, has to match a known name.
#[test]
fn bullet_starting_with_new_section_word_stays_bullet() {
    let line = parse_line(
        "- Projekte für interne Kunden geleitet und Teams koordiniert",
        5,
        &[],
    );
    assert!(
        matches!(line.kind, LineKind::Bullet),
        "expected Bullet, got {:?}",
        line.kind
    );
}

/// A candidate's own prose sentence that merely CONTAINS a new section word
/// must stay Text.
#[test]
fn prose_line_containing_new_section_word_stays_text() {
    let line = parse_line("Mehrere Projekte erfolgreich abgeschlossen.", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Text),
        "expected Text, got {:?}",
        line.kind
    );
}

/// A grouped skills line labelled with a new section word ("Sprachen:") must
/// stay Text, not become a heading — the exact-match test requires the whole
/// line to equal the bare word, and a trailing colon + list is longer than that.
#[test]
fn skills_group_labelled_with_new_section_word_stays_text() {
    let line = parse_line("Sprachen: Deutsch, Englisch, Französisch", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Text),
        "expected Text, got {:?}",
        line.kind
    );
}

/// Parity guard for `is_known_section_name`'s `" & "` arm — the half of that
/// predicate the section-names fixture structurally cannot see. That fixture
/// compares two name LISTS; the join arm is a rule that combines two entries,
/// so Rust could (and did) gain it while the TS mirror
/// (`isKnownSectionName`) kept answering `false` for every merged heading,
/// with every existing test still green.
///
/// The three-part cases pin `split_once`'s cut-at-the-FIRST-separator
/// semantics, which is load bearing because one entry contains a separator of
/// its own ("certifications & training").
#[test]
fn section_name_joins_match_the_ts_predicate_fixture() {
    #[derive(serde::Deserialize)]
    struct Case {
        line: String,
        known: bool,
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/prompts/src/fixtures/section-name-joins.json");
    let raw = std::fs::read_to_string(&path).expect(
        "read section-name-joins parity fixture \
         (packages/prompts/src/fixtures/section-name-joins.json)",
    );
    let cases: Vec<Case> =
        serde_json::from_str(&raw).expect("parse section-name-joins parity fixture");

    assert!(
        !cases.is_empty(),
        "section-name-joins parity fixture must not be empty"
    );
    assert!(
        cases.iter().any(|c| c.known) && cases.iter().any(|c| !c.known),
        "the fixture must carry BOTH accepted and rejected joins — one-sided, \
         it passes for a predicate hardwired to that answer"
    );
    for c in &cases {
        // Same reasoning as the sibling fixtures: `parse_line` only ever runs
        // this predicate on `strip_md(trimmed)`.
        let clean = strip_md(c.line.trim());
        assert_eq!(
            is_known_section_name(&clean),
            c.known,
            "is_known_section_name drift for {:?} (clean: {:?})",
            c.line,
            clean
        );
    }
}

// ── Recurrence guard: producer/recogniser contract, mechanically enforced ──

/// Every heading `pipeline::resume::prompt_blocks::resume_conventions` can
/// emit, for every one of the nine ordered `SectionId`s and all seven curated
/// locales, must be recognised by `export::parser` as a SectionHeader —
/// exactly as the producer emits it (Title-Case, not ALL-CAPS, since
/// ALL-CAPS already has its own shape-based recognition path and Title-Case
/// is the shape that broke in the reported bug). This closes the loop
/// mechanically: a future SectionId or locale added to the producer's total
/// `headers` record without a matching recogniser entry fails HERE, not in a
/// screenshot. Both axes are enumerated from the generated data itself
/// (`RESUME_CONVENTION_LOCALES` and `ResumeConventions::ids`) rather than
/// restated as literal lists — a hardcoded list is exactly how a guard ends up
/// never visiting the new thing it was written to catch.
///
/// Mutation check on the locale axis: added an 8th locale (`sv`) to the TS
/// `CONVENTIONS` with headings absent from `SECTION_NAMES` and re-ran
/// `pnpm gen:prompts` — RAN, went red naming `sv`, reverted. With the old
/// hardcoded `LOCALES` list it would have stayed green.
///
/// Whether this guard would have caught the ORIGINALLY reported bug, if it
/// had existed beforehand: see the report — the honest answer is nuanced,
/// not a flat yes.
#[test]
fn every_producer_heading_is_recognised_by_the_parser() {
    use crate::pipeline::resume::prompt_blocks::{resume_conventions, RESUME_CONVENTION_LOCALES};

    for &lang in RESUME_CONVENTION_LOCALES {
        let conventions = resume_conventions(lang);
        for id in conventions.ids() {
            let heading = conventions.header(id);
            let line = parse_line(heading, 5, &[]);
            assert!(
                matches!(line.kind, LineKind::SectionHeader),
                "locale {lang:?} heading {heading:?} (for SectionId::{id}) was not \
                 recognised as a SectionHeader by export::parser, got {:?}",
                line.kind
            );
        }
    }
}

#[test]
fn test_parse_resume() {
    let text = "John Doe\njohn@example.com\n\nExperience\nSoftware Engineer  Jan 2020 - Present";
    let doc = parse_resume(text);
    assert!(doc.has_name);
    assert!(doc.has_contact);
    assert!(doc.section_count > 0);
}

#[test]
fn thematic_breaks_are_dropped_as_blank() {
    // Each form the model emits as a section separator must be dropped so it
    // never renders as stray "---" text doubling the template's own rule.
    for sep in ["---", "***", "___", "----------", "- - -"] {
        let line = parse_line(sep, 3, &[]);
        assert!(
            matches!(line.kind, LineKind::Blank),
            "expected Blank for separator {sep:?}, got {:?}",
            line.kind
        );
    }
}

#[test]
fn em_dash_and_short_runs_are_not_thematic_breaks() {
    // A real em-dash (single glyph) and a 2-char run are content, not breaks.
    assert!(!matches!(
        parse_line("\u{2014}", 3, &[]).kind,
        LineKind::Blank
    ));
    assert!(!matches!(parse_line("--", 3, &[]).kind, LineKind::Blank));
    // A dashed bullet keeps its text — only pure marker runs are breaks.
    assert!(!matches!(
        parse_line("- real bullet", 3, &[]).kind,
        LineKind::Blank
    ));
}

#[test]
fn sanitize_markdown_strips_stray_emphasis_but_keeps_bold() {
    // The observed symptom: lone asterisks leaked by the model.
    assert_eq!(sanitize_markdown("*React and AWS*"), "React and AWS");
    assert_eq!(sanitize_markdown("AWS*-Services"), "AWS-Services");
    // Valid bold survives (the renderer turns it into real bold).
    assert_eq!(
        sanitize_markdown("Use **React** today"),
        "Use **React** today"
    );
    // Stray backticks go; in-word underscores (snake_case) are left untouched.
    assert_eq!(sanitize_markdown("a `code` span"), "a code span");
    assert_eq!(sanitize_markdown("create_react_app"), "create_react_app");
}

#[test]
fn typography_fixes_sentence_break_dashes_only() {
    // En-dash glued to the previous word (cover-letter symptom) → spaced en-dash.
    assert_eq!(typography("zu Hause\u{2013} die"), "zu Hause \u{2013} die");
    // ASCII hyphen used as a sentence break → spaced en-dash.
    assert_eq!(typography("zu Hause- die"), "zu Hause \u{2013} die");
    // German suspended hyphen is preserved (NOT turned into a dash).
    assert_eq!(typography("Backend- und Frontend"), "Backend- und Frontend");
    // A tight numeric range and a real compound are left alone.
    assert_eq!(typography("2020\u{2013}2023"), "2020\u{2013}2023");
    assert_eq!(typography("state-of-the-art"), "state-of-the-art");
}

// ── New job-entry detection branches ─────────────────────────────────────────

/// Comma + parenthesized date: the AI's documented output format.
/// "Senior Engineer, Acme Corp (January 2021 – March 2023)" → JobEntry
/// with text = the full line (role + company + period all bold).
#[test]
fn job_entry_paren_date_full_line() {
    let line = parse_line(
        "Senior Engineer, Acme Corp (January 2021 \u{2013} March 2023)",
        5,
        &[],
    );
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry, got {:?}",
        line.kind
    );
    assert!(
        line.text
            .contains("Senior Engineer, Acme Corp (January 2021"),
        "text should contain the full header; got: {:?}",
        line.text
    );
    assert!(
        line.right_text.is_none(),
        "right_text must be None for paren-date format; got: {:?}",
        line.right_text
    );
}

/// Pipe-separated with a year-only range.
/// "Senior Platform Engineer | Globex Corp | 2020 – Present" → JobEntry
#[test]
fn job_entry_pipe_date_segment_year_range() {
    let line = parse_line(
        "Senior Platform Engineer | Globex Corp | 2020 \u{2013} Present",
        5,
        &[],
    );
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry, got {:?}",
        line.kind
    );
    assert!(
        line.text.contains("Senior Platform Engineer"),
        "text should contain the full header; got: {:?}",
        line.text
    );
    assert!(
        line.right_text.is_none(),
        "right_text must be None for pipe-date format; got: {:?}",
        line.right_text
    );
}

/// Pipe-separated with month-year range.
/// "Software Engineer | Beta Inc | Jan 2021 – Mar 2023" → JobEntry
#[test]
fn job_entry_pipe_date_segment_month_year_range() {
    let line = parse_line(
        "Software Engineer | Beta Inc | Jan 2021 \u{2013} Mar 2023",
        5,
        &[],
    );
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry, got {:?}",
        line.kind
    );
}

/// "Distributed Rate Limiter | Open Source | 2021" → JobEntry.
/// Projects (and single-year education) use a bare year, not a range — they must
/// still render as bold entries like Experience, not plain paragraphs.
#[test]
fn job_entry_pipe_single_year() {
    let line = parse_line("Distributed Rate Limiter | Open Source | 2021", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry for a single-year project header, got {:?}",
        line.kind
    );
}

/// A contact line carrying a phone + a bare year but NO email must stay Contact —
/// the `@`-only guard was insufficient (real contacts have a phone, not an email).
#[test]
fn contact_line_phone_and_year_stays_contact() {
    let line = parse_line("Berlin, Germany | +49 30 1234567 | 2021", 5, &[]);
    assert!(
        !matches!(line.kind, LineKind::JobEntry),
        "phone+year contact must NOT be JobEntry, got {:?}",
        line.kind
    );
}

/// Single-separator skill / certification lines with a bare year are ambiguous and
/// must NOT be promoted to entries (a single year only counts with ≥2 separators).
#[test]
fn single_separator_year_is_not_job_entry() {
    for s in ["React • 2021", "AWS Certified • 2023"] {
        let line = parse_line(s, 5, &[]);
        assert!(
            !matches!(line.kind, LineKind::JobEntry),
            "{s:?} must NOT be JobEntry, got {:?}",
            line.kind
        );
    }
}

/// Contact line with email MUST still be Contact even if it has pipes.
/// "Haarlem, NL | jane@example.com | +31 6 1234 5678 | LinkedIn" → Contact
#[test]
fn contact_line_with_email_stays_contact() {
    let line = parse_line(
        "Haarlem, NL | jane@example.com | +31 6 1234 5678 | LinkedIn",
        5,
        &[],
    );
    assert!(
        matches!(line.kind, LineKind::Contact),
        "expected Contact (has '@'), got {:?}",
        line.kind
    );
}

/// Contact line with only pipes and no date MUST still be Contact.
/// "New York | LinkedIn | github.com/jane" → Contact (URL_RE matches)
#[test]
fn contact_line_pipes_no_date_stays_contact() {
    let line = parse_line("New York | linkedin.com/in/jane | github.com/jane", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Contact),
        "expected Contact (URL match), got {:?}",
        line.kind
    );
}

// ── Next-line-date job entry (owner-reported: "Title · Company" \n date) ────

/// The reported shape: title/company on their own line with NO date, the date
/// range (+ trailing location) on the line right after. None of the same-line
/// patterns above recognize this — it used to fall through to a plain,
/// non-bold `Text` paragraph, silently losing the entry structure entirely.
#[test]
fn job_entry_title_then_bare_date_line() {
    // idx 0 is padded with a heading so the title line isn't literally the
    // document's first line (idx==0 has its own Name/Contact special case).
    let lines = [
        "EXPERIENCE",
        "Senior Frontend Developer \u{b7} ACTINEO GmbH",
        "December 2022 \u{2013} November 2025, K\u{f6}ln, Deutschland",
    ];
    let line = parse_line(lines[1], 1, &lines);
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Senior Frontend Developer \u{b7} ACTINEO GmbH");
    assert_eq!(
        line.right_text.as_deref(),
        Some("December 2022 \u{2013} November 2025")
    );
}

/// The paired half: the date line itself strips the matched date and keeps
/// the trailing location as a JobTitle (subtitle).
#[test]
fn job_entry_date_line_remainder_becomes_job_title() {
    let lines = [
        "EXPERIENCE",
        "Senior Frontend Developer \u{b7} ACTINEO GmbH",
        "December 2022 \u{2013} November 2025, K\u{f6}ln, Deutschland",
    ];
    let line = parse_line(lines[2], 2, &lines);
    assert!(
        matches!(line.kind, LineKind::JobTitle),
        "expected JobTitle for the date-line remainder, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "K\u{f6}ln, Deutschland");
}

/// A pure date line with nothing after it (no location/description) is
/// dropped as Blank — the date was already attached to the entry above, so
/// it must not ALSO render as a stray, duplicate paragraph.
#[test]
fn job_entry_date_line_with_no_remainder_is_blank() {
    let lines = ["Independent / Open-Source R&D", "Dec 2025 \u{2013} Present"];
    let line = parse_line(lines[1], 1, &lines);
    assert!(
        matches!(line.kind, LineKind::Blank),
        "expected Blank, got {:?}",
        line.kind
    );
}

/// Regression guard: a REAL section heading directly followed by a
/// leading DATE-RANGE line (the exact shape the new backward branch matches
/// on: "Certifications" \n "2020 – Present, AWS Certified Solutions
/// Architect") must NOT be swallowed as a consumed job-entry date line — the
/// heading never opened an entry, so treating this as "consumed" would
/// silently drop the "2020 – Present" range instead of rendering it.
#[test]
fn heading_then_leading_date_range_line_does_not_misfire() {
    let lines = [
        "Certifications",
        "2020 \u{2013} Present, AWS Certified Solutions Architect",
    ];
    let heading = parse_line(lines[0], 0, &lines);
    assert!(
        matches!(heading.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        heading.kind
    );
    let next = parse_line(lines[1], 1, &lines);
    assert!(
        !matches!(next.kind, LineKind::Blank),
        "must not drop the date-range-bearing line as Blank, got {:?}",
        next.kind
    );
}

/// Legacy 2-space format still works.
/// "Acme Corp  2020 - Present" → JobEntry (existing behavior preserved)
#[test]
fn job_entry_legacy_two_space_format_preserved() {
    let line = parse_line("Acme Corp  2020 - Present", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry (legacy 2-space), got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Acme Corp");
    assert_eq!(line.right_text.as_deref(), Some("2020 - Present"));
}

/// A normal skills line is not a job entry.
/// "Rust, TypeScript, React, AWS, Docker" → Text
#[test]
fn skills_line_stays_text() {
    let line = parse_line("Rust, TypeScript, React, AWS, Docker", 5, &[]);
    assert!(
        !matches!(line.kind, LineKind::JobEntry),
        "skills line must not be JobEntry, got {:?}",
        line.kind
    );
}

/// A known section header is still detected as SectionHeader, not JobEntry.
#[test]
fn section_header_not_job_entry() {
    let line = parse_line("EXPERIENCE", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
}

// ── Markdown ATX heading promotion (§F) ──────────────────────────────────────

/// A user-authored custom heading (`## Side Projects`) is promoted to a section
/// heading even though it is neither a known section name nor ALL-CAPS.
#[test]
fn atx_heading_h2_custom_is_section() {
    let line = parse_line("## Side Projects", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Side Projects");
}

/// `### ` (H3) custom heading is likewise promoted, markers stripped.
#[test]
fn atx_heading_h3_custom_is_section() {
    let line = parse_line("### Notable Work", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Notable Work");
}

/// `# Summary` at idx 0 classifies as a heading with the `#` stripped — the ATX
/// rule runs before the idx==0 name/contact block.
#[test]
fn atx_heading_h1_strips_marker_even_at_idx_zero() {
    let line = parse_line("# Summary", 0, &["# Summary"]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Summary");
}

/// Existing known-section behavior is unchanged (no `#` prefix needed).
#[test]
fn known_section_without_hash_still_section() {
    let line = parse_line("Experience", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
}

/// Existing ALL-CAPS behavior is unchanged (no `#` prefix needed).
#[test]
fn all_caps_without_hash_still_section() {
    let line = parse_line("PROFESSIONAL HIGHLIGHTS", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
}

/// A normal prose line (no `#`) is unaffected — still Text.
#[test]
fn prose_line_without_hash_still_text() {
    let line = parse_line("Built and shipped a payments service.", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Text),
        "expected Text, got {:?}",
        line.kind
    );
}

/// A `#hashtag` with no trailing space is NOT an ATX heading — it must fall
/// through to its normal (non-heading) classification.
#[test]
fn hash_without_space_is_not_heading() {
    let line = parse_line("#nospace", 5, &[]);
    assert!(
        !matches!(line.kind, LineKind::SectionHeader),
        "#nospace must NOT be a SectionHeader, got {:?}",
        line.kind
    );
}

/// Bold inside a heading: markers stripped from `text`, but `segments` still
/// tokenize the bold run (`## **Bold Heading**`).
#[test]
fn atx_heading_with_bold_tokenizes_segments() {
    let line = parse_line("## **Bold Heading**", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Bold Heading");
    assert!(
        line.segments
            .iter()
            .any(|s| s.bold && s.text == "Bold Heading"),
        "segments should reflect the bold run; got {:?}",
        line.segments
    );
}

/// Regression: a `---` thematic break is still dropped as Blank — the ATX rule
/// runs AFTER the thematic-break check and never reclassifies it as a heading.
#[test]
fn thematic_break_still_blank_not_heading() {
    let line = parse_line("---", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Blank),
        "expected Blank for thematic break, got {:?}",
        line.kind
    );
}

/// A bare `# ` with empty heading text degrades to Blank (clean is empty) rather
/// than emitting an empty heading.
#[test]
fn empty_atx_heading_falls_through_to_blank() {
    let line = parse_line("# ", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Blank),
        "expected Blank for empty heading, got {:?}",
        line.kind
    );
}

// ── §F bounds: H1–H6 inclusive upper-bound, H7 fall-through, idx-0 Name ────

/// `###### Six` (six hashes) is the inclusive upper bound — must be SectionHeader.
#[test]
fn atx_heading_h6_inclusive_upper_bound() {
    let line = parse_line("###### Six", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "H6 must be SectionHeader (inclusive upper bound), got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Six");
}

/// `####### Seven` (seven hashes) exceeds the ATX limit — must NOT be a heading.
/// It falls through to whatever the normal classification produces (Text / Contact
/// / etc.), but the key requirement is that it is NOT a SectionHeader.
#[test]
fn atx_heading_h7_exceeds_limit_not_heading() {
    let line = parse_line("####### Seven", 5, &[]);
    assert!(
        !matches!(line.kind, LineKind::SectionHeader),
        "7-hash line must NOT be SectionHeader (ATX limit is 6), got {:?}",
        line.kind
    );
}

/// A plain name line at idx 0 with no `#` prefix classifies as `LineKind::Name`,
/// not as a SectionHeader — the ATX rule must not misfire on ordinary prose at
/// the top of the document.
#[test]
fn name_line_at_idx_zero_without_hash_is_name() {
    let line = parse_line("John Doe", 0, &["John Doe"]);
    assert!(
        matches!(line.kind, LineKind::Name),
        "plain line at idx 0 must be Name (no # prefix), got {:?}",
        line.kind
    );
}

// ── Leading-blank-line regression (PDF extraction emits a leading blank) ────

/// Root-cause regression: PDF-extracted résumé text routinely starts with a
/// blank line before the real header. The name rule must key on the first
/// line WITH CONTENT (idx 1 here), not raw idx 0 — otherwise the name falls
/// through to `is_all_caps_section_heading`, `model_from_resume_text` never
/// sets `seen_section` correctly, and the header renders twice (once from
/// the empty header + once as a bogus body section titled with the name).
/// Asserts BOTH symptoms: the header name is populated correctly AND no
/// section in the model is (incorrectly) headed with the candidate's name.
#[test]
fn leading_blank_line_before_name_still_yields_header_not_section() {
    let model = crate::model::adapter::model_from_resume_text(
        "\nSAEED KOLIVAND\nAI & Full-Stack Engineer\nKöln, Germany | a@b.com | +49 179 1402319\n\nPROFESSIONAL SUMMARY\nSome summary.",
    );

    assert_eq!(
        model.header.name, "SAEED KOLIVAND",
        "leading blank line must not prevent the name from populating the header"
    );
    assert!(
        !model
            .sections
            .iter()
            .any(|s| s.heading == model.header.name),
        "the name must never become a section heading; sections: {:?}",
        model
            .sections
            .iter()
            .map(|s| &s.heading)
            .collect::<Vec<_>>()
    );
}

/// Control case: a genuine section heading as the first line WITH CONTENT
/// (still preceded by a leading blank) must still classify as a heading —
/// proving the fix didn't turn real headings into names.
#[test]
fn leading_blank_line_before_section_heading_still_classifies_as_heading() {
    let lines = ["", "EXPERIENCE", "Some body text"];
    let line = parse_line("EXPERIENCE", 1, &lines);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader for the first content line, got {:?}",
        line.kind
    );
}

/// A contact line must never be claimed as a job-entry title.
///
/// `is_entry_title_shaped` is checked BEFORE the `is_contact_shaped` branch, so
/// without its own guard a header contact line that happens to be followed by a
/// leading-date line is swallowed into a fabricated entry — the contact details
/// vanish from the header and reappear as a job title.
#[test]
fn a_contact_line_is_never_claimed_as_an_entry_title() {
    let resume = "\
Max Mustermann
Köln, Deutschland · max@example.de · 0179 1402319
Jan 2021 – Heute, Berlin
- Ein Bulletpoint
";
    let parsed = parse_resume(resume);
    let contact = parsed
        .lines
        .iter()
        .find(|l| l.text.contains("max@example.de"))
        .expect("the contact line must still be present");
    assert_ne!(
        contact.kind,
        LineKind::JobEntry,
        "a contact line was turned into a job entry: {contact:?}"
    );
}
