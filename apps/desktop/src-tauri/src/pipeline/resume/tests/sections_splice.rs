use super::super::stages::sections;
use super::super::types::SectionKey;
use super::support::{DRAFTED, THREE_ROLE_RESUME};

/// A splice replaces exactly one section and leaves every other byte alone —
/// the property that makes "section-scoped repair" true rather than aspirational.
///
/// Mutation check: splice by re-rendering the parsed document and the
/// untouched-sections assertion fails on whitespace alone.
#[test]
fn splice_replaces_one_section_and_touches_nothing_else() {
    let split = sections::split(DRAFTED);
    let skills = sections::find(&split, SectionKey::Skills).expect("skills section");
    let out = sections::splice(DRAFTED, skills, "SKILLS\nGo, Rust, Kubernetes");

    assert!(out.contains("SKILLS\nGo, Rust, Kubernetes"));
    assert!(out.starts_with("PROFESSIONAL SUMMARY\nA payments engineer."));
    assert!(out.contains("WORK EXPERIENCE\nAcme | Engineer | 2021 - Present"));
    assert!(
        out.ends_with('\n'),
        "the trailing-newline shape must survive"
    );
}

/// **The cross-module assumption the splice is built on, pinned.**
///
/// `sections::split` zips `parse_resume(text).lines` against `text.lines()` BY
/// INDEX, and `splice` then slices `text.lines()` with the ranges that zip
/// produced. The whole thing rests on `export::parser::parse_resume` mapping
/// exactly one `ParsedLine` per `text.lines()` entry — an assumption owned by
/// another module, documented in `sections`' own header, and enforced nowhere.
/// If that parser ever starts merging wrapped lines or dropping blanks, the
/// failure here is not a wrong section: it is an out-of-range slice, i.e. a
/// PANIC inside a background run.
///
/// Mutation check: `.filter(|l| !l.trim().is_empty())` in `parse_resume`'s
/// mapping and every blank-carrying case below fails.
#[test]
fn parse_resume_maps_one_to_one_over_text_lines() {
    for text in [
        DRAFTED,
        THREE_ROLE_RESUME,
        "",
        "\n",
        "\n\n\n",
        "one line, no newline",
        "trailing newline\n",
        "  \n\nblank-heavy\n\n  \n",
        "PROFESSIONAL SUMMARY\r\nCRLF body\r\n",
    ] {
        assert_eq!(
            crate::export::parser::parse_resume(text).lines.len(),
            text.lines().count(),
            "parse_resume must stay 1:1 with text.lines() for {text:?} — \
             sections::split zips them by index and splice slices with the result"
        );
    }
}

/// A truncated replacement is a FAILED attempt. Splicing one in would delete the
/// section's content silently, which is the worst outcome available here.
///
/// Mutation check: make `is_usable_replacement` return `true` and every
/// rejection below fails.
#[test]
fn a_truncated_section_is_rejected_rather_than_spliced() {
    assert!(!sections::is_usable_replacement(""));
    assert!(
        !sections::is_usable_replacement("SKILLS"),
        "heading with no body"
    );
    assert!(
        !sections::is_usable_replacement("Go, Rust, Kubernetes"),
        "body with no heading — splicing this loses the heading"
    );
    assert!(sections::is_usable_replacement(
        "SKILLS\nGo, Rust, Kubernetes"
    ));
}

/// **MEDIUM finding fix.** `ParsedDocument::section_count` counts every
/// detected `SectionHeader` LINE, and `export::parser`'s ALL-CAPS heading
/// heuristic cannot tell a genuine second heading from an ALL-CAPS employer
/// name inside the section being replaced — measured through the real parser
/// at `section_count == 3` for exactly this two-employer reply. Rejecting on
/// that raw count silently no-opped the REGENERATE button on a document with
/// real Criticals still unfixed, because every realistic multi-entry
/// Experience reply carries at least one ALL-CAPS company name.
///
/// Mutation check: revert `real_section_count(&parsed) <= 1` to
/// `parsed.section_count <= 1` in `is_usable_replacement` and this fails.
#[test]
fn a_realistic_multi_entry_experience_reply_with_all_caps_employers_is_usable() {
    let replacement = "EXPERIENCE\n\n\
         ACME PAYMENTS GMBH\n\
         Senior Engineer  2019 - 2021\n\
         - Led the checkout migration to the new ledger service\n\
         - Cut p95 latency by 30% across the payments API\n\n\
         GLOBEX LOGISTICS\n\
         Engineer  2016 - 2019\n\
         - Built the shipment-tracking pipeline";
    // Premise, checked rather than merely claimed in the docstring above: the
    // raw parser really does count more than one `SectionHeader` line for
    // this reply (the two ALL-CAPS employer names) — without this, the test
    // goes vacuously green the day the parser stops promoting ALL-CAPS
    // employer names to headings, because `real_section_count` would then
    // agree with `parsed.section_count` and the distinction this test exists
    // to prove would no longer be exercised.
    assert!(
        crate::export::parser::parse_resume(replacement).section_count > 1,
        "premise: the raw parser must count more than one SectionHeader line \
         for two ALL-CAPS employer names inside one Experience section, or \
         this test is not exercising `real_section_count`'s distinction at all"
    );
    assert!(
        sections::is_usable_replacement(replacement),
        "two ALL-CAPS employer names inside one Experience section must not \
         read as a second real section; got {replacement:?}"
    );
}

/// **Confirmation-review finding 2 (HIGH).** `real_section_count` used to
/// count a heading as real ONLY when `classify_section` recognised it —
/// but `classify_section`'s `SectionKind` has no arm for Certifications,
/// Licenses, Languages-spoken, Awards, Publications or Volunteer, so every
/// one of those headings classified `Other` and silently stopped counting,
/// even though the parser's own `SECTION_NAMES` list (and therefore the raw
/// parser itself) promotes every one of them to a real `SectionHeader` line.
/// A reply naming three such headings read as ONE section and would have
/// been spliced in whole, doubling the document's own Certifications and
/// Awards sections underneath it.
///
/// Mutation check: drop the `is_known_section_name` half of
/// `real_section_count`'s filter (back to `classify_section(&line.text) !=
/// SectionKind::Other` alone) and this goes red — `real_section_count`
/// reads 1 (SUMMARY only) and the reply is accepted.
#[test]
fn a_reply_naming_certifications_and_awards_is_rejected_as_more_than_one_section() {
    let reply = "SUMMARY\n\n\
        Backend engineer with eight years on payment and container platforms.\n\n\
        CERTIFICATIONS\n\n\
        AWS Certified Solutions Architect - Professional (2022)\n\n\
        AWARDS\n\n\
        Employee of the Year, 2021";
    assert!(
        !sections::is_usable_replacement(reply),
        "CERTIFICATIONS and AWARDS are both real headings `classify_section` \
         has no bucket for — `real_section_count` must still count them \
         through `is_known_section_name`, or this three-section reply reads \
         as one and gets spliced whole into the Summary range while the \
         document's own Certifications/Awards sections stay duplicated \
         below; got {reply:?}"
    );
}

/// **PR #1003 finding 2 (MAJOR).** A user-authored Markdown ATX heading
/// (`## Leadership`) is a THIRD real-but-unrecognised heading shape,
/// independent of the other two `real_section_count` already covers
/// (ALL-CAPS company names, `is_known_section_name`'s list): `parse_line`
/// promotes an ATX heading to `LineKind::SectionHeader` regardless of whether
/// its NAME matches `classify_section` or `SECTION_NAMES` — that is the whole
/// point of the syntax. Before this fix, neither test recognised
/// "Leadership"/"Speaking Engagements", so `real_section_count` read 1
/// (SUMMARY only) and a three-section ATX reply passed the gate.
///
/// Mutation check: drop the `strip_atx_heading(&line.raw).is_some()` arm from
/// `real_section_count`'s filter and this goes red.
#[test]
fn a_reply_naming_custom_atx_headings_is_rejected_as_more_than_one_section() {
    let reply = "SUMMARY\n\n\
        Backend engineer with eight years on payment and container platforms.\n\n\
        ## Leadership\n\n\
        Mentored four engineers through their first on-call rotation.\n\n\
        ## Speaking Engagements\n\n\
        Spoke at two regional backend meetups about payment reliability.";
    assert!(
        !sections::is_usable_replacement(reply),
        "\"## Leadership\" and \"## Speaking Engagements\" are both real \
         headings neither `classify_section` nor `is_known_section_name` \
         recognises — `real_section_count` must still count them through the \
         ATX marker itself, or this three-section reply reads as one and gets \
         spliced whole into the Summary range; got {reply:?}"
    );
}

/// **BUG-A's shape gap, `repair`'s own copy.** The repair prompt wraps the
/// section it hands the model as `<resume_section>…</resume_section>` and
/// asks for "the replacement section" back — a model that echoes the wrapper
/// instead of just the content would splice the literal tag into the
/// document, and a heading/body-line count alone would not catch it (the
/// wrapper only adds a line). Checked for EVERY registered tag, driven from
/// `prompt_fence::known_fence_tags()` rather than one hardcoded name.
///
/// Mutation check: drop the `contains_fence_tag` gate from
/// `is_usable_replacement` and every assertion below flips true — verified,
/// then reverted.
#[test]
fn is_usable_replacement_rejects_a_replacement_wrapped_in_any_registered_fence_tag() {
    for tag in crate::prompt_fence::known_fence_tags() {
        let wrapped = format!("<{tag}>\nSKILLS\nGo, Rust, Kubernetes\n</{tag}>");
        assert!(
            !sections::is_usable_replacement(&wrapped),
            "a replacement wrapped in <{tag}> must be rejected, not spliced"
        );
    }
}

/// A validator's section LABEL maps back through the same classifier the split
/// used, so a German heading finds its section. Mutation check: compare the
/// label to the English section names as strings and the German case fails.
#[test]
fn a_section_label_maps_back_through_the_shared_classifier() {
    assert_eq!(
        sections::key_for_label(Some("BERUFSERFAHRUNG")),
        Some(SectionKey::Experience(0))
    );
    assert_eq!(
        sections::key_for_label(Some("Kenntnisse")),
        Some(SectionKey::Skills)
    );
    assert_eq!(sections::key_for_label(None), None);
    assert_eq!(sections::key_for_label(Some("Hobbies")), None);
}
