//! Unit tests for `embed.rs`, split into this sibling file (R8 line-budget
//! split — mirrors the `anthropic.rs` + `anthropic_tests.rs` precedent of
//! moving the test module itself out rather than production code).
//!
//! Wired via `#[path = "embed_tests.rs"] mod tests;` in `embed.rs` — that
//! keeps this a CHILD module of `embed` in the module tree (same as an
//! inline `#[cfg(test)] mod tests { ... }` block), so `use super::*` below
//! still reaches every private item there, while this file's own filename
//! (ending `tests.rs`) excludes it from the architecture test's R8 LOC cap
//! (`tests/architecture.rs`'s `is_test` filename check) and from R3/R6's
//! non-test scans.

use super::*;

#[test]
fn truncate_chars_respects_char_boundaries() {
    let text = "héllo wörld"; // multi-byte chars
    assert_eq!(truncate_chars(text, 100), text);
    assert_eq!(truncate_chars(text, 1), "h");
    assert_eq!(truncate_chars(text, 2), "hé");
}

#[test]
fn next_truncation_len_halves_down_to_the_floor_then_gives_up() {
    assert_eq!(next_truncation_len(8000), Some(4000));
    assert_eq!(next_truncation_len(4000), Some(2000));
    assert_eq!(next_truncation_len(2000), Some(1000));
    assert_eq!(next_truncation_len(1000), Some(500));
    assert_eq!(next_truncation_len(500), None);
    assert_eq!(next_truncation_len(200), None);
}

#[test]
fn is_context_length_error_matches_known_provider_wordings() {
    // The exact Ollama wording seen in production (see app log).
    assert!(is_context_length_error(
        "Ollama 500 Internal Server Error: {\"error\":\"the input length exceeds the context length\"}"
    ));
    assert!(is_context_length_error(
        "openai: this model's maximum context length is 8192 tokens"
    ));
    // OpenAI's plain "too long" wording.
    assert!(is_context_length_error("openai: Input text is too long"));
    // Gemini's real over-length wording.
    assert!(is_context_length_error(
        "gemini: the input token count (12000) exceeds the maximum number of tokens allowed (8192)."
    ));
    assert!(is_context_length_error(
        "gemini: request too large — try a smaller resume/job ad."
    ));
    assert!(!is_context_length_error(
        "gemini: invalid or unauthorized API key."
    ));
    assert!(!is_context_length_error(
        "Ollama unreachable: connection refused"
    ));
    // 404 model-not-found must never be mistaken for a length overflow.
    assert!(!is_context_length_error(
        "gemini: model or endpoint not found — models/text-embedding-004 is not found"
    ));
}

/// `AppHandle`-free fake — succeeds once the (truncated) text is at or
/// below `success_at_or_below` chars, otherwise reports a context-length
/// overflow. Lets `embed_adaptive`'s retry loop be exercised with no
/// network/provider at all. `success_len_sum` accumulates the length of
/// every SUCCESSFUL call only — the direct measure of how much of the
/// original document actually got embedded, as opposed to `last_len`
/// (the most recent attempt, success or failure).
struct FakeEmbedAttempt {
    success_at_or_below: usize,
    calls: std::sync::atomic::AtomicUsize,
    last_len: std::sync::atomic::AtomicUsize,
    success_len_sum: std::sync::atomic::AtomicUsize,
}

impl FakeEmbedAttempt {
    fn new(success_at_or_below: usize) -> Self {
        Self {
            success_at_or_below,
            calls: std::sync::atomic::AtomicUsize::new(0),
            last_len: std::sync::atomic::AtomicUsize::new(0),
            success_len_sum: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl EmbedAttempt for FakeEmbedAttempt {
    async fn attempt(&self, text: &str) -> AppResult<(Vec<f64>, Usage)> {
        let len = text.chars().count();
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.last_len
            .store(len, std::sync::atomic::Ordering::SeqCst);
        if len <= self.success_at_or_below {
            self.success_len_sum
                .fetch_add(len, std::sync::atomic::Ordering::SeqCst);
            Ok((vec![0.1, 0.2, 0.3], Usage::default()))
        } else {
            Err(AppError::Provider(
                "mock 500: the input length exceeds the context length".to_string(),
            ))
        }
    }
}

#[tokio::test]
async fn embed_adaptive_retries_and_succeeds_on_shorter_input() {
    let attempt = FakeEmbedAttempt::new(3000);
    let text = "a".repeat(8000);
    let result = embed_adaptive(&attempt, &text, 8000, &mut Usage::default()).await;
    assert!(result.is_ok());
    // 8000 (fail) -> 4000 (fail) -> 2000 (succeeds, <= 3000). The
    // successful 2000-char cap then STICKS for the rest of this chunk
    // too (M1 fix) instead of discarding the remaining 6000 chars: 3
    // more successful 2000-char sends. 2 failures + 4 successes = 6.
    assert_eq!(attempt.calls.load(std::sync::atomic::Ordering::SeqCst), 6);
    assert_eq!(
        attempt.last_len.load(std::sync::atomic::Ordering::SeqCst),
        2000
    );
    // The WHOLE document was embedded (4 x 2000), not just its first
    // successful-cap's worth.
    assert_eq!(
        attempt
            .success_len_sum
            .load(std::sync::atomic::Ordering::SeqCst),
        8000
    );
}

#[tokio::test]
async fn embed_adaptive_never_drops_the_unsent_remainder_of_a_chunk() {
    // The reported production scenario: cap=8000 (Ollama's default), a
    // document long enough to split into several top-level chunks, and
    // a provider whose REAL token window only accepts much shorter input
    // (nomic-embed-text's 2048-token window on a token-dense language).
    // Before the fix, once a smaller cap succeeded, everything past that
    // smaller cap within the SAME top-level chunk was silently dropped
    // forever — a 24,000-char document would embed only 6,000 chars
    // (3 chunks x first-successful-2000) while still being tagged as a
    // complete, indexed vector.
    let attempt = FakeEmbedAttempt::new(2000); // provider accepts <= 2000 chars
    let text = "a".repeat(24_000);
    let result = embed_adaptive(&attempt, &text, 8000, &mut Usage::default()).await;
    assert!(result.is_ok());
    assert_eq!(
        attempt
            .success_len_sum
            .load(std::sync::atomic::Ordering::SeqCst),
        24_000,
        "every char of the document must eventually be embedded"
    );
}

#[tokio::test]
async fn embed_adaptive_learns_the_working_cap_once_not_per_chunk() {
    // 3 top-level 8000-char chunks (24,000 / 8000, no growth needed),
    // provider only accepts <= 2000 chars. WITHOUT hoisting the learned
    // cap out of `embed_chunk_adaptive`, every chunk independently
    // re-pays the 8000 -> 4000 -> 2000 discovery ladder (2 wasted failing
    // calls each = 6 total); WITH it, only chunk 1 pays that cost —
    // chunks 2 and 3 start straight at the already-learned 2000-char cap.
    let attempt = FakeEmbedAttempt::new(2000);
    let text = "a".repeat(24_000);
    let result = embed_adaptive(&attempt, &text, 8000, &mut Usage::default()).await;
    assert!(result.is_ok());
    // chunk 1: 2 failures (8000, 4000) + 4 successes (2000 chars x4) = 6.
    // chunk 2 and chunk 3: 4 successes each, ZERO failures (learned) = 8.
    // 6 + 8 = 14 — NOT the 18 it would be if every chunk re-discovered
    // the cap independently.
    assert_eq!(attempt.calls.load(std::sync::atomic::Ordering::SeqCst), 14);
}

#[tokio::test]
async fn embed_adaptive_does_not_let_a_chunks_remainder_poison_the_learned_cap() {
    // The reported production bug, specifically on the `bounded_split_cap`
    // GROWTH path (document long enough to need more than
    // `MAX_CHUNKS_PER_DOCUMENT` chunks at the nominal cap — the growth
    // ratio doesn't divide evenly by the provider's real limit, so every
    // chunk ends with a short remainder). A chunk's tiny final remainder
    // used to permanently shrink the "learned" cap the NEXT chunk started
    // at, collapsing the whole rest of the document toward 1-char sends
    // and hitting the attempt ceiling with only a sliver actually
    // embedded. 300,000 chars at cap=8000 needs 38 chunks (> 32), so
    // `bounded_split_cap` grows the chunk size to 9,375; the provider
    // only accepts <= 5,000 chars, forcing exactly one real discovery
    // (9375 fails, halves to 4687, which succeeds) — that 4687 must
    // carry forward, not the 1-char tail left over after it.
    let attempt = FakeEmbedAttempt::new(5000);
    let text = "a".repeat(300_000);
    let result = embed_adaptive(&attempt, &text, 8000, &mut Usage::default()).await;
    assert!(
        result.is_ok(),
        "must complete, not hit the {MAX_TOTAL_EMBED_ATTEMPTS}-call ceiling"
    );
    assert_eq!(
        attempt
            .success_len_sum
            .load(std::sync::atomic::Ordering::SeqCst),
        300_000,
        "the whole document must be embedded"
    );
    // Chunk 1: [9375 FAIL, 4687 OK, 4687 OK, 1 OK] = 4 calls, learns 4687.
    // Chunks 2-32 (31 more): each starts AT the learned 4687, so
    // [4687 OK, 4687 OK, 1 OK] = 3 calls, ZERO re-discovery failures.
    // 4 + 31*3 = 97 — nowhere near the un-fixed near-200-and-abort.
    assert_eq!(
        attempt.calls.load(std::sync::atomic::Ordering::SeqCst),
        97,
        "the learned cap must carry across chunks, not be re-discovered near-per-char"
    );
}

#[tokio::test]
async fn embed_adaptive_aborts_with_a_clear_error_past_the_total_attempt_ceiling() {
    // A document long enough that even the DISCOVERED (floor) cap still
    // needs hundreds of successful sends to fully cover it — this must
    // abort with a clear error rather than making the caller
    // (`ai_reembed_all`'s cancellation check runs BETWEEN documents, not
    // within one) unresponsive for however long that would take. 25
    // top-level 8,000-char chunks (200,000 / 8,000, no growth needed);
    // only the floor length (500 chars) ever succeeds, so covering the
    // whole document would need ~400 successful sends plus the initial
    // discovery failures — far past the 200-call ceiling.
    let attempt = FakeEmbedAttempt::new(EMBED_TRUNCATION_FLOOR_CHARS);
    let text = "a".repeat(200_000);
    let err = embed_adaptive(&attempt, &text, 8000, &mut Usage::default())
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains(&MAX_TOTAL_EMBED_ATTEMPTS.to_string()),
        "error should cite the ceiling: {msg}"
    );
    assert!(msg.to_ascii_lowercase().contains("provider calls"));
    // Must stop EXACTLY at the ceiling, never run past it.
    assert_eq!(
        attempt.calls.load(std::sync::atomic::Ordering::SeqCst),
        MAX_TOTAL_EMBED_ATTEMPTS
    );
}

#[tokio::test]
async fn embed_adaptive_bounds_chunk_count_for_a_pathologically_large_document() {
    // A 2 MB document at cap=8000 would be 250 SEQUENTIAL top-level
    // chunks (each up to 3 HTTP attempts x up to 5 adaptive-halving
    // steps worst case) — it was 1 call before this whole feature
    // existed. `usize::MAX` as the success threshold means every attempt
    // succeeds immediately, isolating the CHUNK-COUNT bound from the
    // halving logic tested elsewhere.
    let attempt = FakeEmbedAttempt::new(usize::MAX);
    let text = "a".repeat(2_000_000);
    let result = embed_adaptive(&attempt, &text, 8000, &mut Usage::default()).await;
    assert!(result.is_ok());
    let calls = attempt.calls.load(std::sync::atomic::Ordering::SeqCst);
    assert!(calls <= 32, "expected at most 32 calls, got {calls}");
    assert_eq!(
        attempt
            .success_len_sum
            .load(std::sync::atomic::Ordering::SeqCst),
        2_000_000,
        "the whole document must still be embedded, not truncated to fit the chunk cap"
    );
}

#[tokio::test]
async fn embed_adaptive_gives_up_at_the_floor_with_a_clear_error() {
    // A threshold below the floor never succeeds — the loop must stop.
    let attempt = FakeEmbedAttempt::new(100);
    let text = "a".repeat(8000);
    let err = embed_adaptive(&attempt, &text, 8000, &mut Usage::default())
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains(&EMBED_TRUNCATION_FLOOR_CHARS.to_string()),
        "error should mention the floor length: {msg}"
    );
    assert!(msg.to_ascii_lowercase().contains("too long"));
    // 8000 -> 4000 -> 2000 -> 1000 -> 500 (floor): 5 attempts, then give up.
    assert_eq!(attempt.calls.load(std::sync::atomic::Ordering::SeqCst), 5);
}

#[tokio::test]
async fn embed_adaptive_never_inflates_a_short_documents_reported_length() {
    // A 300-char document (well under the 8000-char provider cap) must
    // never be reported/retried against a cap it was never actually sent
    // at. Before the fix, `cap` halved 8000→4000→2000→1000→500 while the
    // ACTUAL bytes sent stayed 300 the whole time (5 byte-identical
    // requests), and the give-up error falsely said "500 characters".
    let attempt = FakeEmbedAttempt::new(100); // never succeeds — forces a give-up
    let text = "a".repeat(300);
    let err = embed_adaptive(&attempt, &text, 8000, &mut Usage::default())
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("300"),
        "error must cite the ACTUAL sent length: {msg}"
    );
    for phantom in ["8000", "4000", "2000", "1000"] {
        assert!(
            !msg.contains(phantom),
            "error must not cite a cap the document was never sent at: {msg}"
        );
    }
    // 300 chars is already at/below the floor (500) — no retries are
    // possible, so exactly ONE request is made, not five.
    assert_eq!(attempt.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        attempt.last_len.load(std::sync::atomic::Ordering::SeqCst),
        300
    );
}

#[tokio::test]
async fn embed_adaptive_does_not_retry_a_non_context_length_error() {
    struct AlwaysUnauthorized(std::sync::atomic::AtomicUsize);

    #[async_trait]
    impl EmbedAttempt for AlwaysUnauthorized {
        async fn attempt(&self, _text: &str) -> AppResult<(Vec<f64>, Usage)> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(AppError::Config(
                "gemini: invalid or unauthorized API key.".to_string(),
            ))
        }
    }

    let attempt = AlwaysUnauthorized(std::sync::atomic::AtomicUsize::new(0));
    let err = embed_adaptive(&attempt, "short text", 8000, &mut Usage::default())
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Config(_)));
    assert_eq!(attempt.0.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn embed_adaptive_leaves_partial_usage_in_the_output_param_when_a_later_chunk_fails() {
    // Simulates a 429 on chunk 3 of 4 (a non-context-length error, never
    // retried): the FIRST 2 chunks' REAL provider-reported usage must
    // still be visible to the caller via the `usage` OUTPUT param, never
    // silently discarded just because the overall call ultimately
    // failed — `embed_text` records this before propagating the error,
    // so a partial embed no longer drops already-billed spend from the
    // ledger (the pre-chunking code made exactly one call per document,
    // so a failure billed nothing to begin with; this restores that
    // "never lose real usage" property for the new multi-chunk path).
    struct FailAfterNSuccesses {
        succeed_calls: usize,
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl EmbedAttempt for FailAfterNSuccesses {
        async fn attempt(&self, _text: &str) -> AppResult<(Vec<f64>, Usage)> {
            let n = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n < self.succeed_calls {
                Ok((
                    vec![0.1, 0.2],
                    Usage {
                        input_tokens: 100,
                        output_tokens: 0,
                    },
                ))
            } else {
                Err(AppError::RateLimited("mock 429".to_string()))
            }
        }
    }

    let attempt = FailAfterNSuccesses {
        succeed_calls: 2,
        calls: std::sync::atomic::AtomicUsize::new(0),
    };
    let text = "a".repeat(40); // cap=10 -> 4 top-level chunks, no growth
    let mut usage = Usage::default();
    let result = embed_adaptive(&attempt, &text, 10, &mut usage).await;
    assert!(
        result.is_err(),
        "the 3rd chunk's failure must still propagate"
    );
    assert_eq!(
        usage.input_tokens, 200,
        "usage from the 2 chunks that succeeded before the failure must survive"
    );
}

// ── split_into_chunks / l2_normalize (pure helpers) ─────────────────────

#[test]
fn split_into_chunks_covers_the_whole_text_without_dropping_a_tail() {
    let text = "a".repeat(25);
    let chunks = split_into_chunks(&text, 10);
    assert_eq!(chunks, vec!["a".repeat(10), "a".repeat(10), "a".repeat(5)]);
    // Every char of the original text is present across the chunks.
    assert_eq!(chunks.concat().chars().count(), 25);
}

#[test]
fn split_into_chunks_is_a_single_chunk_when_under_the_cap() {
    assert_eq!(split_into_chunks("short", 8000), vec!["short"]);
}

#[test]
fn split_into_chunks_respects_char_boundaries() {
    let text = "é".repeat(7); // multi-byte char, 2 bytes each
    let chunks = split_into_chunks(&text, 3);
    assert_eq!(chunks, vec!["é".repeat(3), "é".repeat(3), "é".to_string()]);
}

#[test]
fn split_into_chunks_of_empty_text_is_one_empty_chunk() {
    // So the caller still makes its usual single provider call rather
    // than zero (preserves prior single-empty-call behavior).
    assert_eq!(split_into_chunks("", 8000), vec![""]);
}

#[test]
fn bounded_split_cap_is_a_no_op_within_the_chunk_limit() {
    // 24,000 chars at cap=8000 is 3 chunks — well under the 32 ceiling.
    assert_eq!(bounded_split_cap(24_000, 8000), 8000);
}

#[test]
fn bounded_split_cap_grows_the_cap_to_stay_within_the_limit_without_dropping_text() {
    // 2 MB at cap=8000 would need 250 chunks — far over the 32 ceiling.
    let total = 2_000_000;
    let cap = bounded_split_cap(total, 8000);
    let needed = total.div_ceil(cap);
    assert!(needed <= 32, "grown cap {cap} still needs {needed} chunks");
    // The grown cap must still cover the WHOLE document across at most
    // 32 chunks — growing the chunk size, never truncating the document.
    assert!(cap * 32 >= total);
}

#[test]
fn l2_normalize_scales_to_unit_length() {
    let mut v = vec![3.0, 4.0]; // 3-4-5 triangle
    l2_normalize(&mut v);
    assert!((v[0] - 0.6).abs() < 1e-9);
    assert!((v[1] - 0.8).abs() < 1e-9);
    let norm = (v[0] * v[0] + v[1] * v[1]).sqrt();
    assert!((norm - 1.0).abs() < 1e-9);
}

#[test]
fn l2_normalize_is_a_no_op_on_an_all_zero_vector() {
    let mut v = vec![0.0, 0.0];
    l2_normalize(&mut v);
    assert_eq!(v, vec![0.0, 0.0]);
}

// ── embed_adaptive: chunk-and-mean-pool (the whole-document fix) ────────

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
