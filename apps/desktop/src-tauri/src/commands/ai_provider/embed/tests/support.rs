//! Shared fixtures for `embed`'s test topics.

use super::super::*;

/// `AppHandle`-free fake — succeeds once the (truncated) text is at or
/// below `success_at_or_below` chars, otherwise reports a context-length
/// overflow. Lets `embed_adaptive`'s retry loop be exercised with no
/// network/provider at all. `success_len_sum` accumulates the length of
/// every SUCCESSFUL call only — the direct measure of how much of the
/// original document actually got embedded, as opposed to `last_len`
/// (the most recent attempt, success or failure).
pub(super) struct FakeEmbedAttempt {
    pub(super) success_at_or_below: usize,
    pub(super) calls: std::sync::atomic::AtomicUsize,
    pub(super) last_len: std::sync::atomic::AtomicUsize,
    pub(super) success_len_sum: std::sync::atomic::AtomicUsize,
}

impl FakeEmbedAttempt {
    pub(super) fn new(success_at_or_below: usize) -> Self {
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
