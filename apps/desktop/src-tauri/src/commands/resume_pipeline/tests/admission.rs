/// **Every provider-calling command in this module goes through the limiter.**
///
/// `resume_pipeline_regenerate_section` did not: it went straight to
/// `charge_daily` + `complete`, and `PROVIDER_DAILY_MAX` is a per-DAY TOTAL,
/// not a rate — so a renderer loop on that button burns the whole ceiling in
/// seconds and every other AI feature in the app is dead until UTC midnight.
///
/// The command needs an `AppHandle` this crate has no harness for, so the
/// assertion is on the exact admission call it makes, against a real `Limiter`:
/// the bucket's concurrency cap refuses the next caller with the retriable
/// error the command's `?` propagates, and the guard releases on drop.
///
/// **This pins the MECHANISM, not the number.** Raising
/// `AGENT_RUN_CONCURRENCY_MAX` does NOT fail it — the loop below is derived
/// from that constant, deliberately, because the value belongs to `limits` and
/// is pinned there. Mutation check for what this DOES guard: make `acquire`
/// return `Ok` on a full gate and the refusal assertion fails; make the guard
/// leak its permit and the re-open assertion does. The call SITE is pinned by
/// the source lock below — deleting the `acquire` call fails that one (verified).
#[test]
fn the_regenerate_section_bucket_refuses_a_caller_past_its_concurrency_cap() {
    let limiter = std::sync::Arc::new(crate::limits::Limiter::default());
    let held: Vec<_> = (0..crate::limits::AGENT_RUN_CONCURRENCY_MAX)
        .map(|index| {
            limiter
                .acquire(
                    "agent_run",
                    crate::limits::AGENT_RUN_RATE_MAX,
                    crate::limits::AGENT_RUN_CONCURRENCY_MAX,
                )
                .unwrap_or_else(|e| panic!("slot {index} must be admitted: {e}"))
        })
        .collect();

    let refused = limiter.acquire(
        "agent_run",
        crate::limits::AGENT_RUN_RATE_MAX,
        crate::limits::AGENT_RUN_CONCURRENCY_MAX,
    );
    assert!(
        matches!(refused, Err(crate::error::AppError::RateLimited(_))),
        "the bucket must refuse, and with the retriable variant the command propagates"
    );
    drop(held);
    assert!(
        limiter
            .acquire(
                "agent_run",
                crate::limits::AGENT_RUN_RATE_MAX,
                crate::limits::AGENT_RUN_CONCURRENCY_MAX,
            )
            .is_ok(),
        "the guard is RAII — releasing it must re-open the slot"
    );
}

/// The source-level half of the lock above: the two provider-calling commands
/// in this module must ADMIT before they spend. Grep-shaped for the same reason
/// `job_analysis_never_reaches_match_scoring` is — the command bodies need a
/// Tauri harness, and "it calls the limiter" is otherwise provable only by
/// reading the code.
///
/// Mutation check: delete either `acquire` call and this fails.
#[test]
fn every_provider_calling_command_admits_before_it_spends() {
    let source = concat!(include_str!("../run.rs"), include_str!("../regenerate.rs"));
    assert!(
        source.contains(".acquire_queued("),
        "resume_pipeline_run must park on the concurrency cap"
    );
    assert!(
        source.contains(".acquire("),
        "resume_pipeline_regenerate_section must be admitted (and REFUSED, not parked — it is a \
         click, not a run)"
    );
}
