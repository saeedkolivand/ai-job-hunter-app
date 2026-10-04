use super::*;

/// A cancel that landed before the arm started must cost ZERO provider calls
/// — not "one, then stop".
///
/// Mutation-visible: replace the query's `embed_or_cancel` with a bare
/// `embedder.embed_one(query).await` and the count goes to 1 while the STATUS
/// stays `Unavailable` (the loop's own `is_cancelled()` break still fires),
/// which is why this asserts the CALL COUNT and not the status alone. There
/// is deliberately no separate `is_cancelled()` guard above the query embed
/// to delete — the `biased` race IS that guard (see `run_dense_arm`).
#[test]
fn a_pre_cancelled_token_embeds_nothing_and_reports_unavailable() {
    let (_dir, store, active) = fixture();
    let embedder = ScriptedEmbedder::new(&active, vec![vec![1.0, 0.0]]);
    let token = CancellationToken::new();
    token.cancel();

    let (ranks, status) = arm_with(&store, &active, &embedder, "q", &corpus(), &token);

    assert_eq!(status, ArmStatus::Unavailable);
    assert!(ranks.is_empty());
    assert_eq!(
        embedder.calls(),
        0,
        "a cancel that arrived before the arm started must not reach the provider at all"
    );
}

/// A cancel arriving MID-arm stops the entry loop instead of running it out.
/// The fake cancels its own token on its 2nd call (the query embed is the
/// 1st), so the loop must stop after the entry that was already in flight —
/// never the full `1 + entries.len()`.
///
/// Mutation-visible: drop `|| token.is_cancelled()` from the loop guard and
/// the count runs to the end of the corpus.
#[test]
fn a_cancel_after_the_second_embed_stops_the_entry_loop() {
    let (_dir, store, active) = fixture();
    let token = CancellationToken::new();
    let entries = corpus();
    let embedder = CancellingEmbedder {
        calls: AtomicUsize::new(0),
        cfg: active.clone(),
        token: token.clone(),
        cancel_after: 2,
    };

    let (ranks, status) = arm_with(&store, &active, &embedder, "q", &entries, &token);

    assert_eq!(
        status,
        ArmStatus::Unavailable,
        "a cancelled arm is Unavailable — the caller still gets the keyword results"
    );
    assert!(ranks.is_empty(), "all-or-nothing: {ranks:?}");
    assert!(
        embedder.calls.load(Ordering::SeqCst) <= 3,
        "the loop must stop at the cancel, not run the corpus out: {} calls for {} entries",
        embedder.calls.load(Ordering::SeqCst),
        entries.len()
    );
    // …and the premise: without the cancel this corpus takes 1 + len calls.
    // A FRESH store — the run above already cached the entry it embedded, and
    // reusing that store would make the premise cheaper than it really is.
    let (_dir2, cold_store) = self::store();
    let uncancelled = ScriptedEmbedder::new(&active, vec![vec![1.0, 0.0]]);
    let (_, ok_status) = arm(&cold_store, &active, &uncancelled, "q", &entries);
    assert_eq!(
        ok_status,
        ArmStatus::Ran,
        "premise: the same corpus ranks fine when nothing cancels it"
    );
    assert_eq!(uncancelled.calls(), 1 + entries.len());
}

/// The case the between-entries token check exists for, and the ONE case the
/// per-embed race cannot cover: a cache HIT needs no embed, so it never
/// touches the token. Every entry but the first is pre-cached here and the
/// cancel lands during that one cold embed — which SUCCEEDS (the race polls
/// the token first, and it was still live at that moment). Without the loop's
/// `token.is_cancelled()` break the remaining hits pair from cache, the arm
/// ranks all three and reports `Ran`, putting `mode: "hybrid"` on the wire
/// for a search the user cancelled.
///
/// Mutation-visible: drop `|| token.is_cancelled()` from the loop guard and
/// this flips to `Ran` with three ranks.
#[test]
fn a_cancel_mid_arm_is_unavailable_even_when_every_remaining_entry_is_cached() {
    let (_dir, store, active) = fixture();
    let entries = corpus();
    seed(&store, &active, &entries[1..], &[1.0, 0.0]);
    let token = CancellationToken::new();
    // Call 1 is the query, call 2 is the only COLD entry — cancel there.
    let embedder = CancellingEmbedder {
        calls: AtomicUsize::new(0),
        cfg: active.clone(),
        token: token.clone(),
        cancel_after: 2,
    };

    let (ranks, status) = arm_with(&store, &active, &embedder, "q", &entries, &token);

    assert_eq!(
        embedder.calls.load(Ordering::SeqCst),
        2,
        "premise: only the query and the single cold entry are embedded — the rest are hits"
    );
    assert_eq!(
        status,
        ArmStatus::Unavailable,
        "a cancelled arm must never report Ran, even when the cache could complete it"
    );
    assert!(ranks.is_empty(), "all-or-nothing: {ranks:?}");
}

/// The half no other test here reaches: a cancel that arrives while an embed
/// is ALREADY in flight. Every other cancellation test lands the cancel
/// between calls, where a plain `.await` would look identical.
///
/// The fake never returns from `embed_one` (`std::future::pending` — a
/// oneshot nobody sends, without the channel), so with a bare
/// `embedder.embed_one(text).await` in `embed_or_cancel` this arm can only
/// end by running out the wall clock: `DENSE_ARM_TIMEOUT` is checked BETWEEN
/// entries and never interrupts a call, so nothing would ever cancel the
/// query embed at all. That is exactly the "sits waiting out the provider's
/// per-attempt timeout" behaviour the race exists to prevent, and the reason
/// the assertion is wrapped in `tokio::time::timeout`: the failure mode
/// under mutation is a HANG, and a hanging test is worse than no test.
///
/// The budget is 2 s against a cancel fired at 50 ms — two orders of
/// magnitude, so this measures the mechanism, not the scheduler.
///
/// Mutation-visible: replace the query's `embed_or_cancel` with
/// `embedder.embed_one(query).await` and this fails on the `expect` below
/// (the elapsed budget), rather than hanging the suite.
#[test]
fn a_cancel_of_an_in_flight_embed_returns_without_waiting_the_provider_out() {
    struct HangingEmbedder {
        calls: AtomicUsize,
    }
    #[async_trait]
    impl Embedder for HangingEmbedder {
        async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            // Never resolves — a provider call that has begun and will not
            // come back inside this test's lifetime.
            std::future::pending::<()>().await;
            None
        }
    }

    let (_dir, store, active) = fixture();
    let embedder = HangingEmbedder {
        calls: AtomicUsize::new(0),
    };

    let outcome = block_on(async {
        let token = CancellationToken::new();
        let canceller = token.clone();
        // Fired from a separate task so the cancel lands while the arm is
        // parked inside the embed, not before it starts (which is what
        // `a_pre_cancelled_token_embeds_nothing_and_reports_unavailable`
        // already covers).
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            canceller.cancel();
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            run_dense_arm(
                &store,
                &active,
                &embedder,
                "q",
                &corpus(),
                crate::commands::ai_provider::timeouts::DENSE_ARM_TIMEOUT,
                &token,
            ),
        )
        .await
    });

    let (ranks, status) = outcome.expect(
        "a cancel must abandon an IN-FLIGHT embed, not wait it out: the arm was still \
         running 2s after a cancel fired at 50ms",
    );
    assert_eq!(
        status,
        ArmStatus::Unavailable,
        "a cancelled query embed degrades the arm, it does not fail the search"
    );
    assert!(ranks.is_empty(), "all-or-nothing: {ranks:?}");
    assert_eq!(
        embedder.calls.load(Ordering::SeqCst),
        1,
        "only the query embed was ever started — the entry loop is never reached"
    );
}
