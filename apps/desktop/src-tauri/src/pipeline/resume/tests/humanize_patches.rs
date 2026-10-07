use super::super::prompts::HumanizeTier;
use super::super::stages::{
    apply_patches, flagged_lines, humanize_mode, humanize_one, FlaggedLine, Mode, Patch, PatchList,
    HUMAN_VOICE_FLAGS,
};
use super::support::{live_deadline, ok_report, voice_report};
use crate::error::AppError;

const DOC: &str = "SUMMARY\nA robust engineer.\n- Cut p95 latency by 40% with a robust cache\n- Wrote docs\nhttps://example.com/robust-demo\n";

fn flagged() -> Vec<FlaggedLine> {
    flagged_lines(&voice_report(&["robust"]), DOC, "en").lines
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
    let flags = flagged_lines(&voice_report(&["robust"]), doc, "en");
    let ids: Vec<usize> = flags.lines.iter().map(|l| l.id).collect();
    assert_eq!(ids, vec![2, 3], "case-insensitive, 1-based, every hit");
    assert!(flags.document_wide.is_empty());
}

#[test]
fn a_flag_with_no_locatable_line_is_document_wide_and_patches_nothing() {
    let flags = flagged_lines(
        &voice_report(&["stddev=1.2"]),
        "One line.\nTwo lines.\n",
        "en",
    );
    assert!(flags.lines.is_empty());
    assert_eq!(flags.document_wide.len(), 1);
}

/// Mutation check: drop the flagged-id filter in `apply_patches` and the
/// context-line / unknown-id assertions fail.
/// M1: the validator's word-boundary rule, not a substring scan.
#[test]
fn a_longer_word_containing_the_phrase_is_not_flagged() {
    let doc = "Revitalized the pipeline\nVitality Health, Backend Engineer\nA vital fix\n";
    let ids: Vec<usize> = flagged_lines(&voice_report(&["vital"]), doc, "en")
        .lines
        .iter()
        .map(|l| l.id)
        .collect();
    assert_eq!(ids, vec![3]);
}

/// M2: the validator's normalisation (case, whitespace, curly apostrophe).
#[test]
fn a_curly_apostrophe_and_double_space_still_locate_the_line() {
    let doc = "Intro\nIt\u{2019}s  worth   noting that it works\n";
    let flags = flagged_lines(&voice_report(&["it's worth noting"]), doc, "en");
    let ids: Vec<usize> = flags.lines.iter().map(|l| l.id).collect();
    assert_eq!(ids, vec![2]);
    assert!(flags.document_wide.is_empty());
}

/// L2: `**bold` is not a bullet; a heading keeps its marker.
/// The never-worse guard still fires on a patched document that scores worse.
#[tokio::test]
async fn a_patch_that_scores_worse_is_reverted() {
    let doc = "SUMMARY\nA robust engineer.\n";
    let report = voice_report(&["robust"]);
    let lines = flagged_lines(&report, doc, "en").lines;
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
        false,
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
    let lines = flagged_lines(&report, doc, "en").lines;
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
        false,
    )
    .await
    .expect("revalidate succeeds");
    assert!(!attempt.reverted && !attempt.failed);
    assert_eq!(attempt.text, "SUMMARY\nA dependable engineer.\n");
}

/// Routing: document-wide flags alone mean a whole-document rewrite, any
/// line-locatable flag means patches, nothing eligible means no call.
///
/// Mutation check: force `humanize_mode` to always return `Mode::Patch` and the
/// document-wide-only and no-flag assertions fail.
#[test]
fn routing_picks_rewrite_for_document_wide_only_and_patch_when_any_line_is_flagged() {
    let doc = "One robust line.
Two lines.
";
    let wide_only = flagged_lines(&voice_report(&["stddev=1.2"]), doc, "en");
    assert!(wide_only.lines.is_empty());
    assert_eq!(humanize_mode(&wide_only), Some(Mode::Rewrite));

    let both = flagged_lines(&voice_report(&["stddev=1.2", "robust"]), doc, "en");
    assert_eq!(both.document_wide.len(), 1);
    assert_eq!(humanize_mode(&both), Some(Mode::Patch));

    let none = flagged_lines(&voice_report(&[]), doc, "en");
    assert_eq!(humanize_mode(&none), None);
}

/// With no eligible flag at all the attempt makes zero provider calls.
///
/// Mutation check: drop the empty-findings early return in `humanize_one`.
#[tokio::test]
async fn a_document_with_no_eligible_flag_makes_no_provider_call() {
    let doc = "One line.
";
    let mut called = false;
    let attempt = humanize_one(
        live_deadline(),
        doc.to_string(),
        ok_report(),
        flagged_lines(&ok_report(), doc, "en").lines,
        |_text, _lines: Vec<FlaggedLine>| {
            called = true;
            async move { Ok("never".to_string()) }
        },
        |_candidate: &str| None,
        |_candidate| async move { Ok(ok_report()) },
        HumanizeTier::Resume,
        false,
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
    let lines = flagged_lines(&report, doc, "en").lines;
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
        false,
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
    let lines = flagged_lines(&report, doc, "en").lines;
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
        false,
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
