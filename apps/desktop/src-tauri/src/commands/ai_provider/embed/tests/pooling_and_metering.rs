//! `embed_adaptive`'s whole-document mean-pool/L2-normalize behavior, and
//! `MeteredAttempt`'s per-real-round-trip daily charge (#1087).

use super::super::*;
use super::support::FakeEmbedAttempt;

/// Returns a distinct, caller-scripted vector per call (in call order), so
/// a test can verify BOTH which text each chunk actually received and how
/// the resulting per-chunk vectors were combined.
struct SequencedEmbedAttempt {
    call_texts: std::sync::Mutex<Vec<String>>,
    vectors: Vec<Vec<f64>>,
}

#[async_trait]
impl EmbedAttempt for SequencedEmbedAttempt {
    async fn attempt(&self, text: &str) -> AppResult<(Vec<f64>, Usage)> {
        let mut texts = self.call_texts.lock().unwrap();
        let i = texts.len();
        texts.push(text.to_string());
        let v = self
            .vectors
            .get(i)
            .cloned()
            .expect("test provided fewer scripted vectors than chunks");
        Ok((
            v,
            Usage {
                input_tokens: 10,
                output_tokens: 0,
                thinking_tokens: None,
            },
        ))
    }
}

#[tokio::test]
async fn embed_adaptive_embeds_every_chunk_of_a_long_document_not_just_its_prefix() {
    // cap=10, a 25-char document -> 3 chunks (10 + 10 + 5). A naive single
    // truncation would have sent ONLY the first 10 chars and silently
    // dropped the rest while still tagging the result as "complete".
    let text = format!("{}{}{}", "a".repeat(10), "b".repeat(10), "c".repeat(5));
    let attempt = SequencedEmbedAttempt {
        call_texts: std::sync::Mutex::new(Vec::new()),
        vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![1.0, 1.0]],
    };

    let mut usage = Usage::default();
    let values = embed_adaptive(&attempt, &text, 10, &mut usage)
        .await
        .unwrap();

    let texts = attempt.call_texts.lock().unwrap();
    assert_eq!(texts.len(), 3, "the whole document must be embedded");
    assert_eq!(*texts, vec!["a".repeat(10), "b".repeat(10), "c".repeat(5)]);

    // Mean of [1,0], [0,1], [1,1] = [2/3, 2/3] -> L2-normalized both
    // components are equal and the result has unit length.
    assert!((values[0] - values[1]).abs() < 1e-9);
    let norm = (values[0] * values[0] + values[1] * values[1]).sqrt();
    assert!(
        (norm - 1.0).abs() < 1e-9,
        "pooled vector must be L2-normalized"
    );

    // REAL usage summed across every chunk call — 3 chunks x 10 tokens.
    assert_eq!(usage.input_tokens, 30);
}

// ── MeteredAttempt (per-real-round-trip daily charge, #1087) ────────────

#[tokio::test]
async fn metered_attempt_charges_once_per_real_provider_round_trip() {
    // Same fixture as `embed_adaptive_retries_and_succeeds_on_shorter_input`
    // above (2 context-length failures + 4 successes = 6 real attempts) —
    // the charge must fire exactly once per attempt, retries included, not
    // once per document. Mutation check: deleting `MeteredAttempt::attempt`'s
    // `(self.charge)()?;` line leaves `charges` at 0 while `inner.calls` is
    // 6, and the final assertion fails.
    let inner = FakeEmbedAttempt::new(3000);
    let charges = std::sync::atomic::AtomicUsize::new(0);
    let charge = || {
        charges.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    };
    let metered = MeteredAttempt {
        inner: &inner,
        charge: Some(&charge),
    };
    let text = "a".repeat(8000);
    let result = embed_adaptive(&metered, &text, 8000, &mut Usage::default()).await;
    assert!(result.is_ok());
    let calls = inner.calls.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        calls, 6,
        "sanity: same call count as the un-metered fixture"
    );
    assert_eq!(
        charges.load(std::sync::atomic::Ordering::SeqCst),
        calls,
        "the daily-budget charge must fire exactly once per real round-trip"
    );
}

#[tokio::test]
async fn metered_attempt_refuses_before_the_real_call_once_the_budget_is_exhausted() {
    // A refused charge must stop the call BEFORE the (billed) provider
    // round-trip runs at all — the inner attempt must never be reached.
    let inner = FakeEmbedAttempt::new(usize::MAX); // would always succeed if reached
    let charge = || Err(AppError::RateLimited("daily ceiling".to_string()));
    let metered = MeteredAttempt {
        inner: &inner,
        charge: Some(&charge),
    };
    let err = embed_adaptive(&metered, "short text", 8000, &mut Usage::default())
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::RateLimited(_)));
    assert_eq!(
        inner.calls.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "the real provider round-trip must never run once the charge is refused"
    );
}

#[tokio::test]
async fn embed_adaptive_single_chunk_document_is_returned_unpooled() {
    // Under the cap -> one chunk -> the provider's own vector passes
    // through mean-pooling as a no-op, then gets L2-normalized.
    let attempt = SequencedEmbedAttempt {
        call_texts: std::sync::Mutex::new(Vec::new()),
        vectors: vec![vec![3.0, 4.0]],
    };
    let mut usage = Usage::default();
    let values = embed_adaptive(&attempt, "short doc", 8000, &mut usage)
        .await
        .unwrap();
    assert_eq!(attempt.call_texts.lock().unwrap().len(), 1);
    assert!((values[0] - 0.6).abs() < 1e-9);
    assert!((values[1] - 0.8).abs() < 1e-9);
    assert_eq!(usage.input_tokens, 10);
}
