use super::super::prompts::HumanizeTier;
use super::super::stages::{
    apply_patches, flagged_lines, humanize_one, FlaggedLine, Patch, PatchList, HUMAN_VOICE_FLAGS,
};
use super::support::{live_deadline, ok_report, voice_report};
use crate::error::AppError;

const DOC: &str = "SUMMARY\nA robust engineer.\n- Cut p95 latency by 40% with a robust cache\n- Wrote docs\nhttps://example.com/robust-demo\n";

fn flagged() -> Vec<FlaggedLine> {
    flagged_lines(&voice_report(&["robust"]), DOC).lines
}

fn patch(id: usize, replacement: &str) -> Patch {
    Patch {
        id,
        replacement: replacement.to_string(),
    }
}

#[test]
fn flagged_lines_locates_every_line_but_never_a_link_line() {
    // The phrase sits on lines 2 and 3 AND on the URL line (5): the URL line is
    // a link line, and `on_link_line` drops the finding entirely.
    assert!(flagged().is_empty());

    let doc =
        "SUMMARY\nA Robust engineer.\n- Cut p95 latency by 40% with a robust cache\n- Wrote docs\n";
    let flags = flagged_lines(&voice_report(&["robust"]), doc);
    let ids: Vec<usize> = flags.lines.iter().map(|l| l.id).collect();
    assert_eq!(ids, vec![2, 3], "case-insensitive, 1-based, every hit");
    assert!(flags.document_wide.is_empty());
}

#[test]
fn a_flag_with_no_locatable_line_is_document_wide_and_patches_nothing() {
    let flags = flagged_lines(&voice_report(&["stddev=1.2"]), "One line.\nTwo lines.\n");
    assert!(flags.lines.is_empty());
    assert_eq!(flags.document_wide.len(), 1);
}

/// Mutation check: drop the flagged-id filter in `apply_patches` and the
/// context-line / unknown-id assertions fail.
#[test]
fn apply_patches_touches_only_flagged_ids_and_ignores_unknown_ones() {
    let doc =
        "SUMMARY\nA Robust engineer.\n- Cut p95 latency by 40% with a robust cache\n- Wrote docs\n";
    let lines = flagged_lines(&voice_report(&["robust"]), doc).lines;
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
    let lines = flagged_lines(&voice_report(&["robust"]), doc).lines;
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
fn the_first_patch_for_an_id_wins() {
    let doc = "A robust line\n";
    let lines = flagged_lines(&voice_report(&["robust"]), doc).lines;
    let out = apply_patches(
        doc,
        &lines,
        &[patch(1, "A plain line"), patch(1, "Another")],
    );
    assert_eq!(out, "A plain line\n");
}

/// The never-worse guard still fires on a patched document that scores worse.
#[tokio::test]
async fn a_patch_that_scores_worse_is_reverted() {
    let doc = "SUMMARY\nA robust engineer.\n";
    let report = voice_report(&["robust"]);
    let lines = flagged_lines(&report, doc).lines;
    let attempt = humanize_one(
        live_deadline(),
        doc.to_string(),
        report,
        lines.clone(),
        |text, lines| async move {
            Ok(apply_patches(
                &text,
                &lines,
                &[patch(2, "A seamless, robust engineer.")],
            ))
        },
        |_candidate: &str| None,
        |_candidate| async move { Ok(voice_report(&["seamless", "robust"])) },
        HumanizeTier::Resume,
    )
    .await
    .expect("revalidate succeeds");
    assert!(attempt.called && attempt.reverted && !attempt.failed);
    assert_eq!(attempt.text, doc);
}

#[tokio::test]
async fn a_patch_that_scores_better_is_kept() {
    let doc = "SUMMARY\nA robust engineer.\n";
    let report = voice_report(&["robust"]);
    let lines = flagged_lines(&report, doc).lines;
    let attempt = humanize_one(
        live_deadline(),
        doc.to_string(),
        report,
        lines,
        |text, lines| async move {
            Ok(apply_patches(
                &text,
                &lines,
                &[patch(2, "A dependable engineer.")],
            ))
        },
        |_candidate: &str| None,
        |_candidate| async move { Ok(ok_report()) },
        HumanizeTier::Resume,
    )
    .await
    .expect("revalidate succeeds");
    assert!(!attempt.reverted && !attempt.failed);
    assert_eq!(attempt.text, "SUMMARY\nA dependable engineer.\n");
}

/// Skip path: only document-wide flags means nothing to patch and ZERO calls.
///
/// Mutation check: drop the empty-findings early return in `humanize_one` and
/// `called` flips true.
#[tokio::test]
async fn a_document_with_only_document_wide_flags_makes_no_provider_call() {
    let doc = "One line.\nTwo lines.\n";
    let report = voice_report(&["stddev=1.2"]);
    let mut called = false;
    let attempt = humanize_one(
        live_deadline(),
        doc.to_string(),
        report.clone(),
        flagged_lines(&report, doc).lines,
        |_text, _lines: Vec<FlaggedLine>| {
            called = true;
            async move { Ok("never".to_string()) }
        },
        |_candidate: &str| None,
        |_candidate| async move { Ok(ok_report()) },
        HumanizeTier::Resume,
    )
    .await
    .expect("no call, no error path");
    assert!(!called && !attempt.called && !attempt.failed);
    assert_eq!(attempt.text, doc);
}

/// Malformed patch JSON fails soft: the call's error is a `failed` attempt with
/// the original kept, exactly like a provider error — never a failed run.
#[tokio::test]
async fn unreadable_patch_json_fails_soft_to_the_original_document() {
    assert!(crate::pipeline::json::parse::<PatchList>("not json at all").is_err());
    let doc = "SUMMARY\nA robust engineer.\n";
    let report = voice_report(&["robust"]);
    let lines = flagged_lines(&report, doc).lines;
    let attempt = humanize_one(
        live_deadline(),
        doc.to_string(),
        report,
        lines,
        |_text, _lines| async move {
            Err(AppError::Message(
                "The AI response could not be read as JSON".into(),
            ))
        },
        |_candidate: &str| None,
        |_candidate| async move { Ok(ok_report()) },
        HumanizeTier::Resume,
    )
    .await
    .expect("a patch-answer error is caught inside humanize_one");
    assert!(attempt.called && attempt.failed && !attempt.reverted);
    assert_eq!(attempt.text, doc);
}

/// An answer with no usable patch leaves the document byte-identical and is
/// not re-graded.
#[tokio::test]
async fn an_answer_with_no_surviving_patch_keeps_the_original_without_revalidating() {
    let doc = "SUMMARY\nA robust engineer.\n";
    let report = voice_report(&["robust"]);
    let lines = flagged_lines(&report, doc).lines;
    let mut revalidated = false;
    let attempt = humanize_one(
        live_deadline(),
        doc.to_string(),
        report,
        lines,
        |text, lines| async move { Ok(apply_patches(&text, &lines, &[patch(99, "ghost")])) },
        |_candidate: &str| None,
        |_candidate| {
            revalidated = true;
            async move { Ok(ok_report()) }
        },
        HumanizeTier::Resume,
    )
    .await
    .expect("no error path");
    assert!(attempt.called && !attempt.failed && !attempt.reverted);
    assert!(!revalidated);
    assert_eq!(attempt.text, doc);
}

/// The stage's "already reads human" gate, pinned at the one place it is made
/// (driving `Humanize::run` needs a `Completer`, which needs an `AppHandle`).
///
/// Mutation check: delete the `voice_before == HUMAN_VOICE_FLAGS` gate and this
/// fails.
#[test]
fn the_stage_skips_before_any_call_when_nothing_is_flagged() {
    const SRC: &str = include_str!("../stages/humanize.rs");
    let gate = SRC
        .find("voice_before == HUMAN_VOICE_FLAGS")
        .expect("the skip gate must exist");
    let first_call = gate
        + SRC[gate..]
            .find("humanize_doc(")
            .expect("the call site must exist");
    assert!(
        gate < first_call,
        "the skip gate must precede every provider call"
    );
    assert!(SRC[gate..first_call].contains("Artifact::skip()"));
    assert_eq!(HUMAN_VOICE_FLAGS, 0);
}
