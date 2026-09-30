use super::super::stages::{run_draft_attempt, DraftEnv, LanguageRetryOutcome};
use super::support::live_deadline;
use crate::commands::ai_provider::{AiGenerateRequest, AiGenerateRequestMessage};
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use parking_lot::Mutex;

const DRAFT_HOOK_SOURCE: &str = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n";

/// **The draft-stage hook actually normalizes**, restoring an altered link —
/// proof the wiring (seed → normalize → record) runs end to end, not just
/// `projects::normalize_projects` in isolation.
#[test]
fn apply_projects_normalization_restores_an_altered_link_and_records_counts() {
    let draft = "PROJECTS\n\n**Ledger CLI** · https://an-altered-fork.example.com/ledger\n";
    let (text, artifact) =
        super::super::stages::apply_projects_normalization(DRAFT_HOOK_SOURCE, draft.to_string());
    assert!(text.contains("https://github.com/janedoe/ledger"));
    assert!(!text.contains("an-altered-fork"));
    assert_eq!(artifact["projectsMatched"], 1);
    assert_eq!(artifact["projectsDropped"], 0);
    assert_eq!(artifact["linksRestored"], 1);
    assert_eq!(artifact["chars"], text.chars().count());
}

/// **A PLAIN-TEXT source (no bold/bullet Projects entries — the shape every
/// `extraction::*` PDF/DOCX/RTF importer produces) is a no-op.** Without this
/// gate, the source's single-line-per-section grouping fallback collapses a
/// three-project section into one mega-entry whose "stack"/"description" are
/// really the next two projects' own title lines — and normalizing over that
/// would rewrite a CORRECT draft into that garbage, validation-clean.
///
/// Mutation check: call `source::seed_projects` directly instead of
/// `projects::seed_projects_for_normalize` in the hook and this fails — the
/// draft gets rewritten from the collapsed mega-seed instead of left alone.
#[test]
fn apply_projects_normalization_is_a_no_op_over_a_plain_text_source() {
    let plain_source = "PROJECTS\n\n\
        Ledger CLI - https://github.com/janedoe/ledger\n\
        A bookkeeping tool.\n\
        CrossKit - https://github.com/janedoe/crosskit\n\
        A design system.\n\
        Dotfiles - https://github.com/janedoe/dotfiles\n";
    let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\n\
        **CrossKit** · https://github.com/janedoe/crosskit\n\n\
        **Dotfiles** · https://github.com/janedoe/dotfiles\n";
    let (text, artifact) =
        super::super::stages::apply_projects_normalization(plain_source, draft.to_string());
    assert_eq!(
        text, draft,
        "a plain-text source must never rewrite an already-correct draft"
    );
    assert_eq!(artifact["projectsMatched"], 0);
    assert_eq!(artifact["projectsDropped"], 0);
    assert_eq!(artifact["linksRestored"], 0);
}

/// **All-dropped is a no-op, not a heading-only Projects section.** Every
/// draft entry the model wrote is unrelated to the source's one seed, so
/// nothing survives the match — and persisting an emptied section with no
/// undo would be worse than leaving the (wrong, but non-destructive) draft
/// alone.
#[test]
fn apply_projects_normalization_is_a_no_op_when_every_entry_is_invented() {
    let draft = "PROJECTS\n\n**Some Other Thing** · https://example.com/unrelated\n";
    let (text, artifact) =
        super::super::stages::apply_projects_normalization(DRAFT_HOOK_SOURCE, draft.to_string());
    assert_eq!(
        text, draft,
        "nothing kept must never become a heading-only section"
    );
    assert_eq!(artifact["projectsMatched"], 0);
}

// Real, already-calibrated fixtures (shared with `validate::content::test`,
// which pins their detected language) rather than hand-written strings — a
// language-detection guard tested against text no detector was ever shown to
// confidently read would prove nothing.
const RETRY_EN_SOURCE: &str =
    include_str!("../../../validate/content/fixtures/en_source_resume.txt");

const RETRY_EN_JOB_AD: &str = include_str!("../../../validate/content/fixtures/en_job_ad.txt");

const RETRY_EN_CLEAN: &str =
    include_str!("../../../validate/content/fixtures/en_generated_clean.txt");

// Actually GERMAN prose (the fixture name is the EN scenario it belongs to,
// not its own language) — the wrong-language first draft this whole family
// exists to catch.
const RETRY_WRONG_LANGUAGE: &str =
    include_str!("../../../validate/content/fixtures/en_generated_wrong_language.txt");

// A DIFFERENT German document, so "the still-wrong retry is discarded" is
// provable by content, not just by two identical strings comparing equal.
const RETRY_DE_CLEAN: &str =
    include_str!("../../../validate/content/fixtures/de_generated_clean.txt");

/// The retry is kept ONLY when it actually fixed the language. A still-wrong
/// retry keeps the FIRST draft — the canonical prompt's own answer — not the
/// candidate, mirroring `repair::round_is_worse`'s never-hand-back-a-worse-
/// document floor.
///
/// Mutation check: drop the second `document_language_mismatch` check in
/// `draft_with_language_retry` (keep the retry unconditionally) — RAN, went
/// red on the second half (the still-German candidate shipped instead of the
/// first draft), reverted.
#[tokio::test]
async fn the_draft_retry_keeps_the_second_attempt_only_when_it_fixed_the_language() {
    // The retry FIXES the language.
    let (kept, _artifact, retry) = super::super::stages::draft_with_language_retry(
        RETRY_EN_SOURCE,
        RETRY_EN_JOB_AD,
        "en",
        live_deadline(),
        |is_retry| {
            let text = if is_retry {
                RETRY_EN_CLEAN
            } else {
                RETRY_WRONG_LANGUAGE
            };
            async move { Ok(text.to_string()) }
        },
    )
    .await
    .expect("neither call errors in this test");
    assert_eq!(
        retry,
        LanguageRetryOutcome::Fixed,
        "a wrong first draft must trigger the retry, and this one fixed it"
    );
    assert!(
        kept.contains("SUMMARY") && !kept.contains("ZUSAMMENFASSUNG"),
        "a retry that fixed the language must ship, not the wrong-language first draft"
    );

    // The retry is STILL wrong (a different German document).
    let (kept, _artifact, retry) = super::super::stages::draft_with_language_retry(
        RETRY_EN_SOURCE,
        RETRY_EN_JOB_AD,
        "en",
        live_deadline(),
        |is_retry| {
            let text = if is_retry {
                RETRY_DE_CLEAN
            } else {
                RETRY_WRONG_LANGUAGE
            };
            async move { Ok(text.to_string()) }
        },
    )
    .await
    .expect("neither call errors in this test");
    assert_eq!(
        retry,
        LanguageRetryOutcome::StillWrong,
        "a wrong first draft must trigger the retry, and this one stayed wrong"
    );
    assert!(
        kept.contains("ZUSAMMENFASSUNG"),
        "a still-wrong retry must not replace the first, canonical-prompt draft"
    );
    assert!(
        !kept.contains("PROFIL"),
        "the still-wrong candidate must never ship"
    );
}

/// The retry fires AT MOST ONCE: one extra provider call when the first draft
/// is wrong, none when it is already right. This pins the MECHANICAL bound (a
/// call counter); the STRUCTURAL argument for why it cannot fire twice is in
/// `draft.rs`'s own module doc.
///
/// Mutation check: replace the single retry `match` in
/// `draft_with_language_retry` with a bounded `for _ in 0..3 { .. }` loop that
/// keeps retrying while the candidate is still wrong — RAN, went red (4 calls
/// instead of 2 on the wrong-first-draft case), reverted. (An UNBOUNDED
/// `while` was not run by hand: the injected closure below always returns the
/// same wrong-language text, so an unbounded loop would spin for the whole
/// `live_deadline()` wall-clock allowance instead of failing fast — the
/// bounded loop is the same category of mutation without the hang.)
#[tokio::test]
async fn the_draft_retries_at_most_once() {
    let mut calls = 0u32;
    let (_kept, _artifact, retry) = super::super::stages::draft_with_language_retry(
        RETRY_EN_SOURCE,
        RETRY_EN_JOB_AD,
        "en",
        live_deadline(),
        |_is_retry| {
            calls += 1;
            async move { Ok(RETRY_WRONG_LANGUAGE.to_string()) }
        },
    )
    .await
    .expect("neither call errors in this test");
    assert_eq!(retry, LanguageRetryOutcome::StillWrong);
    assert_eq!(calls, 2, "a wrong first draft must retry exactly once");

    let mut calls = 0u32;
    let (_kept, _artifact, retry) = super::super::stages::draft_with_language_retry(
        RETRY_EN_SOURCE,
        RETRY_EN_JOB_AD,
        "en",
        live_deadline(),
        |_is_retry| {
            calls += 1;
            async move { Ok(RETRY_EN_CLEAN.to_string()) }
        },
    )
    .await
    .expect("neither call errors in this test");
    assert_eq!(retry, LanguageRetryOutcome::NotNeeded);
    assert_eq!(calls, 1, "a correct first draft must never retry");
}

/// A retry refused before any round trip — the shape a `charge_daily`
/// ceiling refusal takes in [`run_draft_attempt`] (it errors BEFORE
/// `DraftEnv::complete` is ever called) — must produce `Errored`, not one of
/// the two outcomes that made a round trip. `Draft::run`'s ledger fix
/// (`ctx.ledger.count_call` gated on `retry.called()`) depends on exactly
/// this: `Errored` must never be mistaken for a billed call. Proven at this
/// seam because `Draft::run` itself needs a live `Completer`/`AppHandle`
/// this crate has no test harness for.
///
/// Mutation check: change `LanguageRetryOutcome::called` back to
/// `!matches!(self, Self::NotNeeded)` (the old `retried` bool's behavior,
/// counting every attempt regardless of whether a round trip happened) —
/// RAN, went red (`retry.called()` read `true` for this `Errored` case),
/// reverted.
#[tokio::test]
async fn a_retry_refused_before_any_round_trip_is_not_counted_as_a_call() {
    let (kept, _artifact, retry) = super::super::stages::draft_with_language_retry(
        RETRY_EN_SOURCE,
        RETRY_EN_JOB_AD,
        "en",
        live_deadline(),
        |is_retry| async move {
            if is_retry {
                // No round trip is ever sent for this branch — the exact
                // shape `charge_daily`'s `?` short-circuit produces.
                Err(AppError::RateLimited("daily budget exhausted".to_string()))
            } else {
                Ok(RETRY_WRONG_LANGUAGE.to_string())
            }
        },
    )
    .await
    .expect("the first call never errors in this test");
    assert_eq!(retry, LanguageRetryOutcome::Errored);
    assert!(
        !retry.called(),
        "a refused retry made no round trip and must not be billed"
    );
    // The retry's failure is not this stage's failure — the first,
    // wrong-language draft is still what ships.
    assert!(kept.contains("ZUSAMMENFASSUNG"));
}

/// [`DraftEnv`] fake — records which channel [`run_draft_attempt`] called,
/// and how many times, without a live `Completer`/`AppHandle`.
#[derive(Default)]
struct FakeDraftEnv {
    streamed_calls: Mutex<u32>,
    completed_calls: Mutex<u32>,
    charge_calls: Mutex<u32>,
}

fn draft_attempt_request() -> AiGenerateRequest {
    AiGenerateRequest {
        model: String::new(),
        messages: vec![
            AiGenerateRequestMessage {
                role: "system".to_string(),
                content: "sys".to_string(),
            },
            AiGenerateRequestMessage {
                role: "user".to_string(),
                content: "usr".to_string(),
            },
        ],
        locale: "en".to_string(),
        temperature: None,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        repeat_penalty: None,
        max_tokens: None,
        context_window: None,
        effort: None,
        intent: None,
    }
}

#[async_trait]
impl DraftEnv for FakeDraftEnv {
    async fn stream_captured(&self, _job_id: &str, _req: AiGenerateRequest) -> AppResult<String> {
        *self.streamed_calls.lock() += 1;
        Ok("streamed draft".to_string())
    }
    async fn complete(&self, _system: &str, _user: &str) -> AppResult<String> {
        *self.completed_calls.lock() += 1;
        Ok("captured draft".to_string())
    }
    fn charge_daily(&self) -> AppResult<()> {
        *self.charge_calls.lock() += 1;
        Ok(())
    }
}

/// **The retry never streams** — the direct guard for the double-render
/// defect fixed on this branch (`draft.rs`'s module doc): before
/// `run_draft_attempt` existed, [`Draft::run`](super::super::stages::Draft)'s
/// closure called the streamed channel on BOTH attempts, so a wrong-language
/// retry rendered a second document appended to the first in the live pane.
/// This asserts the ABSOLUTE observable — which channel fired, and how many
/// times — never a comparison between two derived values.
///
/// Mutation check: swap the two arms of `run_draft_attempt`'s `if is_retry`
/// (route the retry through `stream_captured` and the first attempt through
/// `charge_daily` + `complete`) — RAN, went red (`streamed_calls` reads 0
/// after the first attempt and 1 only after the retry, the reverse of what
/// this test requires), reverted.
#[tokio::test]
async fn the_draft_retry_never_streams() {
    let env = FakeDraftEnv::default();

    let first = run_draft_attempt(&env, "job-1", false, draft_attempt_request())
        .await
        .expect("first attempt succeeds");
    assert_eq!(first, "streamed draft");
    assert_eq!(
        *env.streamed_calls.lock(),
        1,
        "the first attempt must stream"
    );
    assert_eq!(
        *env.completed_calls.lock(),
        0,
        "the first attempt must never call the non-streaming channel"
    );

    let retry = run_draft_attempt(&env, "job-1", true, draft_attempt_request())
        .await
        .expect("retry succeeds");
    assert_eq!(retry, "captured draft");
    assert_eq!(
        *env.streamed_calls.lock(),
        1,
        "the retry must NOT add a second stream over the same job_id — that is exactly the \
         double-render defect (see draft.rs's module doc)"
    );
    assert_eq!(
        *env.completed_calls.lock(),
        1,
        "the retry must call the non-streaming channel exactly once"
    );
    assert_eq!(
        *env.charge_calls.lock(),
        1,
        "the retry must charge the daily ceiling itself — stream_captured does that internally \
         for the first attempt, but the fake never routes through it"
    );
}

/// **The `draft` stage refuses BEFORE it spends anything.**
///
/// A gate that sits after the completer resolution, the prompt build, or the
/// daily-ceiling charge is not a skip — it is a discarded result, and a
/// cover-letter-only run would still be paying for the résumé it asked not to
/// have. Position, not presence, is the property: this asserts the gate comes
/// first, exactly where `CoverLetter::run` puts its own.
///
/// Both offsets are absolute (byte positions inside the stage's `run`), so a
/// build with no gate at all fails on the `expect` rather than passing.
///
/// Mutation check: move the `if !ctx.input.include_resume` block below
/// `let completer = ctx.completer_for(NAME);` and this fails.
#[test]
fn the_draft_stage_refuses_before_it_spends() {
    const SRC: &str = include_str!("../stages/draft.rs");
    let run = SRC
        .find("async fn run(&self, ctx: &mut QualityCtx<'a>)")
        .expect("Draft::run must still exist");
    let body = &SRC[run..];

    let gate = body
        .find("if !ctx.input.include_resume {")
        .expect("Draft::run must gate on include_resume");
    let spend = body
        .find("ctx.completer_for(NAME)")
        .expect("Draft::run must still resolve a completer for the run it DOES do");

    assert!(
        gate < spend,
        "the gate must precede every cost — completer resolution, prompt build, \
         the stream, and the daily-ceiling charge — the same placement \
         CoverLetter::run uses for its own"
    );
}
