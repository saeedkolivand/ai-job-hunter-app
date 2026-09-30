use crate::ipc_contracts::resume_pipeline::ResumePipelineRunRequest;
use serde_json::json;

/// **The wire schema's caps are Zod, and Zod does not run on this transport.**
/// A direct IPC caller can send an unbounded `targetLanguage`, a 10 000-entry
/// `topRequirements`, or a 5 MB cover letter, all of which reach a prompt or a
/// stored row. The command mirrors the caps server-side, HOISTED from
/// `commands::resume` rather than re-declared.
///
/// Mutation check: drop any one clamp and its assertion fails. The multi-byte
/// straddle is deliberate — a naive byte truncate splits it and produces
/// invalid UTF-8.
#[test]
fn oversized_run_request_free_text_is_clamped_server_side() {
    use crate::commands::resume::{
        TARGET_LANGUAGE_CAP, TOP_REQUIREMENTS_CAP, TOP_REQUIREMENT_BYTES_CAP,
    };

    let huge = "a".repeat(TARGET_LANGUAGE_CAP - 1) + "\u{1F600}" + &"b".repeat(5_000);
    let req: ResumePipelineRunRequest = serde_json::from_value(json!({
        // Oversized on purpose (LOW 1): `resumeId`/`jobId` are the two other
        // renderer strings on this command, echoed into a validation-error
        // message and into `metrics_json`.
        "resumeId": "r".repeat(5_000),
        "jobId": "j".repeat(5_000),
        "jobUrl": format!("https://boards.example/{}", "u".repeat(9_000)),
        "targetLanguage": huge,
        "topRequirements": (0..500).map(|i| format!("{i} {}", "r".repeat(2_000)))
            .collect::<Vec<_>>(),
        "coverLetterText": "c".repeat(1_000_000),
    }))
    .expect("the hostile shape still deserializes — nothing rejects it on the wire");

    let clamped = super::super::clamp_request(&req);
    assert!(clamped.target_language.len() <= TARGET_LANGUAGE_CAP);
    assert!(
        !clamped.target_language.contains('\u{1F600}'),
        "must cut before the multi-byte char, not through it"
    );
    assert_eq!(clamped.top_requirements.len(), TOP_REQUIREMENTS_CAP);
    assert!(clamped
        .top_requirements
        .iter()
        .all(|r| r.len() <= TOP_REQUIREMENT_BYTES_CAP));
    assert!(clamped.job_url.len() <= 2_048);
    assert!(
        clamped.cover_letter.len() <= crate::applications::MAX_JOB_DESCRIPTION_BYTES,
        "the letter is validated AND stored — an unbounded one reaches both"
    );
    assert!(clamped.resume_id.len() <= super::super::resolve::JOB_IDENTITY_CAP);
    assert!(clamped.job_id.len() <= super::super::resolve::JOB_IDENTITY_CAP);
}

/// PR-3: the two id-less text fields, plus the text-path posting identity,
/// get the SAME clamp treatment as every other free-text field on this
/// command. Same mutation check as `oversized_run_request_free_text_is_clamped_server_side`
/// — drop any one clamp and its assertion fails.
#[test]
fn oversized_text_path_fields_are_clamped_server_side() {
    let req: ResumePipelineRunRequest = serde_json::from_value(json!({
        "resumeText": "r".repeat(1_000_000),
        "jobAdText": "j".repeat(1_000_000),
        "jobTitle": "t".repeat(5_000),
        "companyName": "c".repeat(5_000),
        "board": "b".repeat(5_000),
    }))
    .expect("the hostile shape still deserializes — nothing rejects it on the wire");

    let clamped = super::super::clamp_request(&req);
    assert!(clamped.resume_text.len() <= crate::applications::MAX_JOB_DESCRIPTION_BYTES);
    assert!(clamped.job_ad_text.len() <= crate::applications::MAX_JOB_DESCRIPTION_BYTES);
    assert!(clamped.job_title.len() <= super::super::resolve::JOB_IDENTITY_CAP);
    assert!(clamped.company_name.len() <= super::super::resolve::JOB_IDENTITY_CAP);
    assert!(clamped.board.len() <= super::super::resolve::BOARD_CAP);
}
