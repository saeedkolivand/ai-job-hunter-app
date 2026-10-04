//! The remaining layout variants (Navy, Sidebar, Monogram) and the
//! cross-cutting ATS-mode / DE-market behavior shared across all five
//! non-Classic layouts.

use super::support::{document_xml, letter_request, REFINED_DE_TEXT, REFINED_US_TEXT};
use crate::export::docx::generate_docx;
use crate::export::types::{GenerationMeta, LetterLayout};

/// Navy's DOCX must match Navy's PDF, not Banded's.
///
/// The renderer branched on a single `is_refined` boolean, so every non-Refined
/// layout got Banded's treatment. Two review rounds were needed to find them
/// all, because "differs from Banded" passes as soon as ONE branch is split —
/// these assert the specific features instead.
#[test]
fn navy_docx_follows_the_navy_design_not_banded() {
    let navy = document_xml(
        &generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Navy)).expect("navy"),
    );
    let banded = document_xml(
        &generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Banded)).expect("banded"),
    );

    // 1. No header band. `letter_navy.typ` has no shaded block; Banded does, and
    //    Navy silently inherited it.
    assert!(
        banded.contains("<w:shd"),
        "precondition: Banded is expected to carry the shaded band"
    );
    assert!(
        !navy.contains("<w:shd"),
        "Navy must not render Banded's shaded header band"
    );

    // 2. Centred letterhead — the NAME AND the contact line, not just one.
    //    `letter_request` supplies `contact: None`, so these exercise the
    //    no-profile FALLBACK contact path, which used to right-align Navy while
    //    the profile-backed path centred it. A single "contains center" check
    //    passed anyway, because the name alone satisfied it.
    let centred = |xml: &str| xml.matches(r#"w:val="center""#).count();
    let right = |xml: &str| xml.matches(r#"w:val="right""#).count();
    assert!(
        centred(&navy) >= 2,
        "Navy must centre the name AND the contact line; found {} centred paragraph(s)",
        centred(&navy)
    );
    assert_eq!(centred(&banded), 0, "precondition: Banded centres nothing");
    assert!(
        right(&navy) < right(&banded),
        "Navy must not right-align the header lines Banded does: navy={} banded={}",
        right(&navy),
        right(&banded)
    );

    // 3. Date and recipient stay REGULAR weight — `letter_navy.typ`'s
    //    emit-date-block / emit-recipient-block carry no `weight: "bold"`,
    //    unlike Banded's. Bold-run count is the observable proxy.
    let bold_runs = |xml: &str| xml.matches("<w:b />").count();
    assert!(
        bold_runs(&navy) < bold_runs(&banded),
        "Navy bolds fewer runs than Banded (it does not bold date/recipient):          navy={} banded={}",
        bold_runs(&navy),
        bold_runs(&banded)
    );
}

/// Navy's role line and subject caption must use NAVY's styling, not Refined's.
///
/// The style struct made each feature's PRESENCE layout-aware but left its
/// STYLING hardcoded to Refined's, so Navy rendered the role line
/// accent-coloured, uppercased and letter-spaced while `letter_navy.typ` puts it
/// in the muted date colour, plain case, untracked — and the subject caption in
/// the accent colour where the `.typ` uses the name colour. Presence and style
/// are separate decisions; asserting only presence missed both.
#[test]
fn navy_docx_styles_the_title_and_caption_like_its_typ() {
    let with_title = |layout: LetterLayout| {
        let mut req = letter_request(REFINED_US_TEXT, layout);
        req.meta = Some(GenerationMeta {
            candidate_name: Some("Jane Smith".to_string()),
            job_title: Some("Platform Engineer".to_string()),
            company_name: None,
            target_language: None,
        });
        document_xml(&generate_docx(&req).expect("docx"))
    };

    let navy = with_title(LetterLayout::Navy);
    let refined = with_title(LetterLayout::Refined);

    // Refined uppercases and tracks its role line; Navy does neither.
    assert!(
        refined.contains("PLATFORM ENGINEER"),
        "precondition: Refined uppercases the role line"
    );
    assert!(
        navy.contains("Platform Engineer"),
        "Navy must keep the role line in its original case"
    );
    assert!(
        !navy.contains("PLATFORM ENGINEER"),
        "Navy must not uppercase the role line — letter_navy.typ renders it as written"
    );

    // Letter-spacing is Refined-only (`character_spacing` ⇒ `<w:spacing w:val=…>`
    // on the run). Navy's role line carries none.
    let spaced_runs = |xml: &str| xml.matches("w:spacing w:val=\"24\"").count();
    assert!(
        spaced_runs(&refined) > spaced_runs(&navy),
        "Refined tracks more runs than Navy: refined={} navy={}",
        spaced_runs(&refined),
        spaced_runs(&navy)
    );
}

/// Sidebar's DOCX approximates the tinted rail as paragraph shading behind the
/// name (there is no margin-anchored frame in DOCX, and a text box or a
/// two-column table would be exactly the multi-column trap the export avoids)
/// and keeps the contact at the LEFT margin, because the rail stacks it under
/// the name rather than pulling it to the right edge.
///
/// The contact alignment is the assertion that matters: it was a function of
/// "centred or right" before this layout existed, so a Sidebar that forgot to
/// state its own alignment would silently right-align — the same
/// inherit-by-omission defect that took four review rounds on Navy.
#[test]
fn sidebar_docx_shades_the_name_and_left_aligns_the_contact() {
    let xml = document_xml(
        &generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Sidebar)).expect("sidebar"),
    );
    // Classic's accent #222222 lightened 85 % toward white → #DEDEDE, the same
    // `band_tint_hex` Banded uses, so PDF and DOCX show one tint.
    assert!(
        xml.contains("w:shd") && xml.contains(r#"w:fill="DEDEDE""#),
        "Sidebar must approximate the rail with the lightened-accent shading: {xml}"
    );
    assert!(
        xml.contains(r#"w:jc w:val="left""#),
        "Sidebar must LEFT-align the contact line — the rail stacks it under the name: {xml}"
    );
    assert!(
        !xml.contains(r#"w:jc w:val="right""#),
        "Sidebar must not inherit Refined/Banded's right-aligned contact: {xml}"
    );
    assert!(
        xml.contains("Jane Smith"),
        "Sidebar must keep the name as written (no uppercasing): {xml}"
    );
}

/// Monogram's DOCX device is a shaded RUN at the head of the name paragraph,
/// carrying the SAME initials the `.typ` gets from `LetterHead.initials` — both
/// call `monogram_initials`, so the two formats cannot disagree about what the
/// device says, and both extract "JS Jane Smith" rather than putting the
/// initials on a line of their own.
#[test]
fn monogram_docx_prefixes_the_name_with_the_shaded_initials() {
    let xml = document_xml(
        &generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Monogram)).expect("monogram"),
    );
    // Run shading, not paragraph shading: the initials sit BESIDE the name in
    // the `.typ`, so they must share its paragraph.
    let name_para = xml
        .split("<w:p>")
        .find(|p| p.contains("Jane Smith"))
        .expect("a paragraph containing the name");
    let runs: Vec<&str> = name_para.split("<w:r>").collect();

    let initials_run = runs
        .iter()
        .find(|r| r.contains(">JS<"))
        .unwrap_or_else(|| panic!("no run carries the initials: {name_para}"));
    assert!(
        initials_run.contains("w:shd") && initials_run.contains(r#"w:fill="DEDEDE""#),
        "the Monogram initials run must carry the accent-tint shading: {initials_run}"
    );

    // The gap between device and name must sit OUTSIDE the tile. docx-rs always
    // writes `xml:space="preserve"`, so spaces bundled into the shaded run get
    // painted and the tile runs on past the initials — which the `.typ` square
    // never does.
    let sep_run = runs
        .iter()
        .find(|r| r.contains(">  <"))
        .unwrap_or_else(|| panic!("no separator run between the device and the name: {name_para}"));
    assert!(
        !sep_run.contains("w:shd"),
        "the separator spaces must NOT be shaded — the tint would extend past the \
         initials: {sep_run}"
    );
    assert!(
        xml.contains(r#"w:jc w:val="left""#),
        "Monogram must left-align the contact line under the lockup: {xml}"
    );
}

/// A letter whose first line is a DATE has no letterhead name, and the device
/// must not invent one from it.
///
/// This renderer's line filter excludes a salutation, a sign-off and a subject
/// — and nothing else — so a date reached the name branch and `12 March 2025`
/// put `12` in the device. Fixed by routing through the SHARED
/// `letterhead_initials` the `.typ` side already used, rather than the
/// unguarded `monogram_initials` this file used to call: one guard, so the two
/// formats cannot disagree about which openings are not names.
#[test]
fn monogram_docx_emits_no_device_for_a_date_opening() {
    const DATE_FIRST: &str =
        "12 March 2025\n\nDear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";

    for candidate_name in [None, Some(String::new())] {
        let mut request = letter_request(DATE_FIRST, LetterLayout::Monogram);
        request.meta = candidate_name.clone().map(|candidate_name| GenerationMeta {
            candidate_name: Some(candidate_name),
            job_title: None,
            company_name: None,
            target_language: None,
        });
        let xml = document_xml(&generate_docx(&request).expect("date-opening monogram docx"));
        assert!(
            !xml.contains(">12<"),
            "candidate_name={candidate_name:?}: the device must not read `12` off the date line: {xml}"
        );
        // …and NO device at all, not merely a different one. Asserting only
        // `!">12<"` passes on the `is_name_token` letters-only rule alone —
        // that rule turns "12 March 2025" into `M`, a device built from the
        // month. It is the date guard, not the token rule, that has to refuse
        // the line outright, and only this assertion can tell them apart.
        assert!(
            !xml.contains("w:shd"),
            "candidate_name={candidate_name:?}: a date opening must produce NO monogram device \
             (found the shaded tile): {xml}"
        );
    }
}

/// The Monogram device is TEXT, so ATS mode must remove the initials
/// themselves — not merely their shading.
#[test]
fn ats_mode_drops_the_monogram_initials_from_docx() {
    let mut request = letter_request(REFINED_US_TEXT, LetterLayout::Monogram);
    request.ats_mode = true;
    let ats = document_xml(&generate_docx(&request).expect("ats monogram docx"));
    assert!(
        !ats.contains(">JS<"),
        "ATS-mode Monogram must not emit the initials run — they extract as noise \
         before the name: {ats}"
    );
    assert!(
        ats.contains("Jane Smith"),
        "ATS-mode Monogram must still render the name: {ats}"
    );
}

/// ATS mode drops the decorative tint in DOCX exactly as it does in the PDF.
/// Without this the two formats disagree: the user turns the toggle on, the PDF
/// loses its band and the Word file keeps it.
#[test]
fn ats_mode_drops_every_letter_docx_tint() {
    for layout in [
        LetterLayout::Banded,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let design = document_xml(
            &generate_docx(&letter_request(REFINED_US_TEXT, layout)).expect("design docx"),
        );
        let mut ats_request = letter_request(REFINED_US_TEXT, layout);
        ats_request.ats_mode = true;
        let ats = document_xml(&generate_docx(&ats_request).expect("ats docx"));

        assert!(
            design.contains(r#"w:fill="DEDEDE""#),
            "precondition: {layout:?} is expected to carry the accent tint in design mode"
        );
        assert!(
            !ats.contains(r#"w:fill="DEDEDE""#),
            "{layout:?} must drop its accent tint under ATS mode: {ats}"
        );
        // Degradation loses decoration, not words.
        for needle in ["Jane Smith", "Dear Hiring Manager", "distributed systems"] {
            assert!(
                ats.contains(needle),
                "ATS-mode {layout:?} DOCX dropped {needle:?}: {ats}"
            );
        }
    }
}

/// The DE market caption, in DOCX, for every caption-bearing layout.
///
/// Every other `letter_request` in this file uses `locale: None`, which resolves
/// to the label-less `intl` market — so nothing here had ever exercised a market
/// that HAS a subject label, and the DOCX suppression rule
/// (`strip_market_label` + `has_own_label`) was running untested. That gap is
/// what let the PDF side ship the duplicate: with no DE coverage on either side,
/// the two formats could disagree silently.
///
/// Asserts the label renders exactly ONCE — the caption is emitted uppercased
/// and the body keeps the market's own casing, so a duplicate shows up as two
/// case-insensitive matches.
#[test]
fn cover_letter_docx_renders_the_de_market_label_exactly_once() {
    for layout in [
        LetterLayout::Refined,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let mut request = letter_request(REFINED_DE_TEXT, layout);
        request.locale = Some("de".to_string());
        let xml = document_xml(&generate_docx(&request).expect("de docx"));

        // Text nodes only — attribute values never carry the label.
        let body: String = xml.to_lowercase();
        let count = body.matches("betreff").count();
        assert_eq!(
            count, 1,
            "{layout:?}: the DE label must appear exactly once in the DOCX, found {count} \
             — the caption is duplicating the label data.subject already carries: {xml}"
        );
        assert!(
            xml.contains("Bewerbung als Software Engineer"),
            "{layout:?}: the DE subject body went missing: {xml}"
        );
        // Same isolating check as the PDF side: the colon only survives on an
        // unstripped body, so this pins WHICH of the two occurrences was kept.
        assert!(
            !body.contains("betreff: bewerbung"),
            "{layout:?}: the label was left on the subject body instead of being stripped \
             into the caption: {xml}"
        );
    }
}
