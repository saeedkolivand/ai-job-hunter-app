//! The Phase 4 AI-notes step: its gate, the fenced user turn, and the fake-driven loop guarantees.

use parking_lot::Mutex;

use super::super::*;
use crate::autopilot::tests::support::found_job_full;

// ── AI notes (Phase 4) ────────────────────────────────────────────────────

#[test]
fn notes_gate_requires_optin_and_a_resume() {
    // Opt-in OFF → never runs, regardless of résumé.
    assert!(!notes_enabled(false, Some("full résumé text")));
    assert!(!notes_enabled(false, None));
    // Opt-in ON but no usable résumé (absent / empty / whitespace) → skip: the
    // note is grounded in the résumé, so there is nothing to reason about.
    assert!(!notes_enabled(true, None));
    assert!(!notes_enabled(true, Some("")));
    assert!(!notes_enabled(true, Some("   \n\t ")));
    // Opt-in ON with a real résumé → runs.
    assert!(notes_enabled(true, Some("Senior Rust engineer, 8y")));
}

#[test]
fn note_user_msg_fences_resume_and_job_as_data() {
    // SECURITY (OWASP LLM01): the résumé and job posting must ride as fenced
    // DATA in the user turn — the system prompt is the only instruction source.
    // LOW-6: the caller fences the résumé ONCE, before the loop; `note_user_msg`
    // takes it already-fenced.
    let resume_fence = fenced("candidate_resume", "my résumé", RESUME_CAP);
    let msg = note_user_msg(
        &resume_fence,
        "Staff Engineer",
        "Acme",
        Some("Build distributed systems in Rust."),
    );
    assert!(msg.contains("<candidate_resume>\nmy résumé\n</candidate_resume>"));
    assert!(msg.contains("<job_posting>"));
    assert!(msg.contains("Staff Engineer at Acme"));
    assert!(msg.contains("Build distributed systems in Rust."));
    assert!(msg.contains("</job_posting>"));
    // A missing description still produces a valid, fenced job block.
    let no_desc = note_user_msg(&resume_fence, "Dev", "Beta", None);
    assert!(no_desc.contains("<job_posting>"));
    assert!(no_desc.contains("Dev at Beta"));
}

#[test]
fn note_user_msg_never_renders_a_bare_at_separator() {
    // A blank title and/or company must not leave a dangling " at " (or
    // " at Acme" / "Acme at ") in the header — only join the two with " at "
    // when BOTH are present.
    let resume_fence = fenced("candidate_resume", "r", RESUME_CAP);

    let both_blank = note_user_msg(&resume_fence, "", "", Some("desc"));
    assert!(
        !both_blank.contains(" at "),
        "both blank must not render a bare separator; got: {both_blank}"
    );

    let title_only = note_user_msg(&resume_fence, "Dev", "", Some("desc"));
    assert!(title_only.contains("Dev"));
    assert!(
        !title_only.contains(" at "),
        "missing company must not render a trailing separator; got: {title_only}"
    );

    let company_only = note_user_msg(&resume_fence, "", "Acme", Some("desc"));
    assert!(company_only.contains("Acme"));
    assert!(
        !company_only.contains(" at "),
        "missing title must not render a leading separator; got: {company_only}"
    );
}

#[test]
fn note_user_msg_caps_oversized_resume_and_job_as_data() {
    // A pathological résumé/description can't blow the context/cost budget of the
    // note call — each blob is capped at the shared agent-tools char cap. The
    // résumé cap is applied once, by the caller, when it builds the fence.
    let huge = "z".repeat(RESUME_CAP + 5_000);
    let resume_fence = fenced("candidate_resume", &huge, RESUME_CAP);
    let msg = note_user_msg(&resume_fence, "T", "C", Some(&huge));
    // The résumé fence carries at most RESUME_CAP chars of `z`.
    let resume_zs = msg
        .split("<candidate_resume>\n")
        .nth(1)
        .and_then(|s| s.split("\n</candidate_resume>").next())
        .unwrap_or("");
    assert_eq!(resume_zs.chars().filter(|&c| c == 'z').count(), RESUME_CAP);
    assert!(msg.chars().filter(|&c| c == 'z').count() <= RESUME_CAP + JOB_CAP);
}

/// Every `snake_case` token in a prompt — copy of the (now fully deleted,
/// PR-5 step 2) `agent::flows::tests::tool_like_tokens` helper, moved here
/// alongside `AUTOPILOT_NOTE_SYSTEM` (PR-5 step 1).
fn tool_like_tokens(prompt: &str) -> std::collections::BTreeSet<String> {
    prompt
        .split(|c: char| !(c.is_ascii_lowercase() || c == '_'))
        .filter(|token| token.contains('_'))
        .map(str::to_string)
        .collect()
}

/// The headless Autopilot note prompt is single-shot and tool-free — it
/// must never grow a tool instruction (there is no loop, no whitelist and
/// no confirm gate on a schedule to honor one).
#[test]
fn autopilot_note_system_names_no_tools() {
    assert!(tool_like_tokens(AUTOPILOT_NOTE_SYSTEM).is_empty());
}

// ── run_notes_loop (HIGH-2: the async loop's guarantees, fake-driven) ──────

/// A scripted [`NoteEnv`] fake: records every `complete()` call, returns a
/// canned response (or errors), and can fail `charge_daily` from a chosen call
/// onward. Mirrored the now-deleted `agent::controller::tests::FakeEnv`'s
/// shape — no `AppHandle` or live provider, which is the whole point of the
/// seam.
struct FakeNoteEnv {
    calls: Mutex<usize>,
    response: AppResult<String>,
    /// `charge_daily` fails starting from this 1-based call number (`None` =
    /// never fails).
    charge_fails_from: Option<usize>,
}

impl FakeNoteEnv {
    fn ok(response: &str) -> Self {
        Self {
            calls: Mutex::new(0),
            response: Ok(response.to_string()),
            charge_fails_from: None,
        }
    }
    fn charge_fails_from(call: usize) -> Self {
        Self {
            calls: Mutex::new(0),
            response: Ok("a note".to_string()),
            charge_fails_from: Some(call),
        }
    }
}

#[async_trait]
impl NoteEnv for FakeNoteEnv {
    async fn complete(&self, _system: &str, _user: &str, _temperature: f64) -> AppResult<String> {
        *self.calls.lock() += 1;
        match &self.response {
            Ok(s) => Ok(s.clone()),
            Err(e) => Err(AppError::Provider(e.to_string())),
        }
    }
    fn charge_daily(&self) -> AppResult<()> {
        let attempted = *self.calls.lock() + 1; // the call this charge is guarding
        match self.charge_fails_from {
            Some(from) if attempted >= from => {
                Err(AppError::RateLimited("daily cap reached".into()))
            }
            _ => Ok(()),
        }
    }
}

fn stub_job(url: &str) -> FoundJob {
    FoundJob {
        is_new: true,
        ..found_job_full(url, "Engineer", "Acme", 0)
    }
}

/// Run the notes loop over `jobs` with a trivially fenced résumé and a token nobody cancels.
async fn run_loop(env: &dyn NoteEnv, jobs: &mut [FoundJob], prior: &HashSet<String>) -> usize {
    run_notes_loop(
        env,
        "<candidate_resume>\nr\n</candidate_resume>",
        jobs,
        prior,
        &CancellationToken::new(),
    )
    .await
}

#[tokio::test]
async fn more_than_max_new_jobs_makes_exactly_max_calls() {
    // >3 genuinely-new matches must stop at exactly ASSISTANT_NOTES_MAX calls,
    // not process every job — the hard cost bound.
    let env = FakeNoteEnv::ok("Strong fit; tailor the systems-design bullet.");
    let mut jobs: Vec<FoundJob> = (0..5)
        .map(|i| stub_job(&format!("https://acme.example/{i}")))
        .collect();
    let prior: HashSet<String> = HashSet::new();
    let generated = run_loop(&env, &mut jobs, &prior).await;
    assert_eq!(*env.calls.lock(), ASSISTANT_NOTES_MAX);
    assert_eq!(generated, ASSISTANT_NOTES_MAX);
    assert_eq!(
        jobs.iter().filter(|j| j.assistant_notes.is_some()).count(),
        ASSISTANT_NOTES_MAX
    );
    // Only the FIRST ASSISTANT_NOTES_MAX jobs (in order) were annotated.
    assert!(jobs[ASSISTANT_NOTES_MAX].assistant_notes.is_none());
}

#[tokio::test]
async fn jobs_already_seen_make_zero_calls_even_under_new_tracking_params() {
    // Every match already surfaced in a prior run → the merge preserves its
    // earlier note for free; re-generating would just burn a call for nothing.
    let env = FakeNoteEnv::ok("note");
    // Re-surfaced under new tracking params: same job to the merge.
    let mut jobs = vec![
        stub_job("https://acme.example/1?utm_source=indeed"),
        stub_job("https://acme.example/2#apply"),
    ];
    let prior: HashSet<String> = ["https://acme.example/1", "https://acme.example/2"]
        .into_iter()
        .map(|u| crate::scraping::boards::common::canonical_job_key(u, "Engineer", "Acme"))
        .collect();
    let generated = run_loop(&env, &mut jobs, &prior).await;
    assert_eq!(generated, 0);
    assert_eq!(
        *env.calls.lock(),
        0,
        "no provider call for a re-surfaced job"
    );
}

#[tokio::test]
async fn duplicate_url_variants_within_one_run_pay_for_only_one_note() {
    // The same NEW job surfaced twice in ONE run under different URL variants
    // (same canonical key). It used to buy a note for EACH, then
    // `merge_found_jobs` collapsed them and discarded one. Only the first
    // variant must pay; the second is skipped.
    let env = FakeNoteEnv::ok("note");
    let mut jobs = vec![
        stub_job("https://acme.example/1?utm_source=indeed"),
        stub_job("https://acme.example/1#apply"),
    ];
    let prior: HashSet<String> = HashSet::new();
    let generated = run_loop(&env, &mut jobs, &prior).await;
    assert_eq!(
        generated, 1,
        "only one note for the two variants of one job"
    );
    assert_eq!(
        *env.calls.lock(),
        1,
        "the duplicate variant makes no provider call"
    );
    assert!(jobs[0].assistant_notes.is_some());
    assert!(
        jobs[1].assistant_notes.is_none(),
        "the second variant is skipped, not annotated"
    );
}

#[tokio::test]
async fn daily_ceiling_error_stops_the_loop_early() {
    // MEDIUM-5: shared per-provider daily ceiling. Once `charge_daily` refuses
    // (2nd call onward here), the loop stops WITHOUT calling `complete()` for
    // that job or any after it — the run still completes, just with fewer notes.
    let env = FakeNoteEnv::charge_fails_from(2);
    let mut jobs = vec![
        stub_job("https://acme.example/1"),
        stub_job("https://acme.example/2"),
        stub_job("https://acme.example/3"),
    ];
    let prior: HashSet<String> = HashSet::new();
    let generated = run_loop(&env, &mut jobs, &prior).await;
    assert_eq!(
        generated, 1,
        "only the job admitted before the ceiling got a note"
    );
    assert_eq!(
        *env.calls.lock(),
        1,
        "the loop must stop before calling complete() again"
    );
    assert!(jobs[0].assistant_notes.is_some());
    assert!(jobs[1].assistant_notes.is_none());
    assert!(jobs[2].assistant_notes.is_none());
}

/// HIGH-1: cancellation must interrupt an IN-FLIGHT completion, not just fire
/// between iterations. `complete()` here never resolves on its own — the only
/// way `run_notes_loop` can return is via the `tokio::select!` race against
/// `cancel.cancelled()`. Deterministic under the current-thread test runtime;
/// mirrored the shape of the now-deleted
/// `agent::controller::tests::cancellation_during_an_inflight_turn_stops_immediately`.
#[tokio::test]
async fn cancellation_during_an_inflight_call_stops_immediately() {
    struct HangingNoteEnv;
    #[async_trait]
    impl NoteEnv for HangingNoteEnv {
        async fn complete(
            &self,
            _system: &str,
            _user: &str,
            _temperature: f64,
        ) -> AppResult<String> {
            std::future::pending::<AppResult<String>>().await
        }
        fn charge_daily(&self) -> AppResult<()> {
            Ok(())
        }
    }

    let mut jobs = vec![stub_job("https://acme.example/1")];
    let prior: HashSet<String> = HashSet::new();
    let cancel = CancellationToken::new();
    let cancel_task = cancel.clone();
    tokio::spawn(async move {
        cancel_task.cancel();
    });

    let generated = run_notes_loop(
        &HangingNoteEnv,
        "<candidate_resume>\nr\n</candidate_resume>",
        &mut jobs,
        &prior,
        &cancel,
    )
    .await;
    assert_eq!(generated, 0);
    assert!(jobs[0].assistant_notes.is_none());
}
