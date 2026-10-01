//! Guardrail: body-only letter completion regression (shipped-fix pin).

use super::fixtures::{letter_lower, signature_block};
use super::letter_fixtures::{LETTER_FIXTURE_BODY_ONLY_DE, LETTER_FIXTURE_BODY_ONLY_US};
use crate::export::types::LetterLayout;

//
// Background: `pipeline::resume::prompts::letter_system` tells the model NOT
// to write a salutation/sign-off/signature — "the application adds them at
// export time". For months the application did not, and nobody noticed
// because every letter test above only ever fed a COMPLETE fixture (the
// shape the OLD prompt produced). The user-visible result: the opening
// paragraph rendered as the letterhead name AND under the sign-off, a body
// paragraph with a percentage became the date, the rest flattened into the
// plain-text recipient block, and there was no salutation and no sign-off.
// The fix is `export::letter_shape::complete_letter_text`, called from
// `export::commands::validate_and_normalize`.
//
// These tests assert on the extracted PDF TEXT LAYER, not the `LetterModel`
// struct — the incident's own lesson is that a document can render
// successfully and still be wrong.

/// Guardrail for the shipped fix, US market.
///
/// Seam: `letter_lower` (above) renders via `render_letter_pdf` directly,
/// which — unlike the real export command — bypasses
/// `export::commands::validate_and_normalize`, the ONLY place that calls
/// `complete_letter_text`. So this test completes the fixture HERE first,
/// the same call `typst_engine/letter.rs`'s own end-to-end tests make (see
/// its `completed_body_only_letter_keeps_bold_as_a_rich_run_in_body` and
/// neighbours), before handing the now-complete text to `letter_lower`.
/// Without that call this test would render the raw body-only fixture as-is
/// and assert nothing meaningful about the fix — `complete_letter_text` is
/// exactly the step under test.
#[test]
fn body_only_us_letter_gets_completed_furniture_in_the_pdf_text_layer() {
    let completed = crate::export::letter_shape::complete_letter_text(
        LETTER_FIXTURE_BODY_ONLY_US,
        "us",
        "Jane Smith",
    );
    let lower = letter_lower(LetterLayout::Classic, &completed, "us", false);

    let pos_sal = lower
        .find("dear hiring manager")
        .expect("US: salutation must be present — complete_letter_text must have run");
    let pos_p1 = lower
        .find("distributed systems")
        .expect("US: body paragraph 1 missing");
    let pos_p2 = lower
        .find("microservices architecture")
        .expect("US: body paragraph 2 (bold keyword) missing");
    let pos_p3 = lower
        .find("contribute from day one")
        .expect("US: body paragraph 3 missing");
    let pos_signoff = lower
        .find("sincerely")
        .expect("US: sign-off must be present — complete_letter_text must have run");

    // Every body paragraph reads AFTER the salutation, and the sign-off reads
    // after the last of them — the reading-order pattern every other letter
    // test in this file already uses.
    for (label, pos) in [
        ("paragraph 1", pos_p1),
        ("paragraph 2 (bold keyword)", pos_p2),
        ("paragraph 3", pos_p3),
    ] {
        assert!(
            pos > pos_sal,
            "US: body {label} reads at/before the salutation — misrouted into the \
             pre-salutation recipient block instead of the body\npos={pos} sal={pos_sal}\n{lower}"
        );
    }
    let last_body = pos_p1.max(pos_p2).max(pos_p3);
    assert!(
        pos_signoff > last_body,
        "US: sign-off reads before the last body paragraph — signoff={pos_signoff} \
         last_body={last_body}\n{lower}"
    );

    // Candidate name in the letterhead AND under the sign-off.
    assert!(
        lower[..pos_sal].contains("jane smith"),
        "US: candidate name missing from the letterhead\n{lower}"
    );
    assert!(
        signature_block(&lower).contains("jane smith"),
        "US: candidate name missing from the signature block (after the sign-off)\n{lower}"
    );

    // The original bug's exact symptom: the long opening paragraph rendering
    // as the letterhead name instead of "Jane Smith". `pos_p1 > pos_sal`
    // above already proves the FIRST occurrence of "distributed systems"
    // reads after the salutation, which already rules this out, but this
    // assertion names the failure mode directly rather than leaving it
    // implicit in an ordering check.
    assert!(
        !lower[..pos_sal].contains("distributed systems"),
        "US: body paragraph 1 leaked into the letterhead — the letterhead name slot rendered \
         body prose instead of the candidate's name\n{lower}"
    );
}

/// Guardrail for the shipped fix, German market — same shape as the US test
/// above (see its doc comment for the seam explanation), plus the
/// DE-specific digits-and-period assertion: [`LETTER_FIXTURE_BODY_ONLY_DE`]'s
/// second paragraph reads "von 0 % auf 90 %." mid-sentence, the exact shape
/// that used to get misrouted before the salutation existed to flip
/// `parse_cover_letter`'s `body_started` flag.
#[test]
fn body_only_de_letter_gets_completed_furniture_in_the_pdf_text_layer() {
    let completed = crate::export::letter_shape::complete_letter_text(
        LETTER_FIXTURE_BODY_ONLY_DE,
        "de",
        "Max Müller",
    );
    let lower = letter_lower(LetterLayout::Classic, &completed, "de", false);

    let pos_sal = lower
        .find("sehr geehrte damen und herren")
        .expect("DE: salutation must be present — complete_letter_text must have run");
    let pos_p1 = lower
        .find("verteilter systeme")
        .expect("DE: body paragraph 1 (long opening) missing");
    let pos_digits = lower
        .find("0 % auf 90 %")
        .expect("DE: body paragraph 2's digits-and-period sentence missing");
    let pos_jest = lower
        .find("einführung von jest")
        .expect("DE: body paragraph 2's bold-keyword sentence missing");
    let pos_p3 = lower
        .find("vorstellungsgespräch")
        .expect("DE: body paragraph 3 missing");
    let pos_signoff = lower
        .find("freundlichen")
        .expect("DE: sign-off must be present — complete_letter_text must have run");

    for (label, pos) in [
        ("paragraph 1", pos_p1),
        ("paragraph 2 (digits + period)", pos_digits),
        ("paragraph 2 (bold keyword)", pos_jest),
        ("paragraph 3", pos_p3),
    ] {
        assert!(
            pos > pos_sal,
            "DE: body {label} reads at/before the salutation — misrouted into the \
             pre-salutation recipient/date block instead of the body\npos={pos} sal={pos_sal}\n{lower}"
        );
    }
    let last_body = pos_p1.max(pos_digits).max(pos_jest).max(pos_p3);
    assert!(
        pos_signoff > last_body,
        "DE: sign-off reads before the last body paragraph — signoff={pos_signoff} \
         last_body={last_body}\n{lower}"
    );

    // Candidate name in the letterhead AND under the sign-off.
    assert!(
        lower[..pos_sal].contains("max") && lower[..pos_sal].contains("müller"),
        "DE: candidate name missing from the letterhead\n{lower}"
    );
    let signature_tail = lower
        .split_once("freundlichen")
        .map(|(_, tail)| tail)
        .unwrap_or("");
    assert!(
        signature_tail.contains("max") && signature_tail.contains("müller"),
        "DE: candidate name missing from the signature block (after the sign-off)\n{lower}"
    );

    // The original bug's exact symptom: the long opening paragraph rendering
    // as the letterhead name instead of "Max Müller".
    assert!(
        !lower[..pos_sal].contains("verteilter systeme"),
        "DE: body paragraph 1 leaked into the letterhead — the letterhead name slot rendered \
         body prose instead of the candidate's name\n{lower}"
    );

    // DE-specific: the digits-and-period paragraph is BODY prose, not the
    // date. `pos_digits > pos_sal` above already proves it did not land in
    // the pre-salutation date/recipient slot; `pos_jest > pos_digits` proves
    // the SAME paragraph's second sentence (after the mid-sentence period)
    // reads immediately after the first, i.e. the whole paragraph rendered
    // together as one piece of body prose rather than being split apart by a
    // date misclassification.
    assert!(
        pos_jest > pos_digits,
        "DE: the digits-and-period paragraph's two sentences reversed reading order — it did \
         not render as one intact body paragraph\ndigits={pos_digits} jest={pos_jest}\n{lower}"
    );
}

/// Prompt↔export lock. `pipeline::resume::prompts::letter_system` instructs
/// the model NOT to write a salutation/sign-off/signature block because "the
/// application adds them at export time" — that promise IS
/// `export::letter_shape::complete_letter_text` (called from
/// `export::commands::validate_and_normalize`). The two are one contract: if
/// this instruction is ever removed or reworded so the model is expected to
/// write its own salutation again, `complete_letter_text` must be retired in
/// the SAME change, or every letter will double up the furniture the model
/// now writes itself — the exact defect class this whole guardrail file
/// section exists to prevent, just inverted.
///
/// Lives here rather than beside `letter_system` in
/// `pipeline/resume/prompts.rs` because this task's scope is test-code-only
/// for the typst_engine/docx test modules.
#[test]
fn letter_system_prompt_still_promises_the_export_adds_the_salutation() {
    let prompt = crate::pipeline::resume::prompts::letter_system("en", "us", false, false);
    assert!(
        prompt.contains("Do NOT write a contact header, a salutation line, or a signature block"),
        "`pipeline::resume::prompts::letter_system` no longer promises the salutation/sign-off/\
         signature are added at export time. If that instruction was intentionally removed, \
         `export::letter_shape::complete_letter_text` (called from \
         `export::commands::validate_and_normalize`) must be retired in the SAME change — \
         otherwise the model will write its own furniture AND the export will add another copy. \
         Got:\n{prompt}"
    );
}
