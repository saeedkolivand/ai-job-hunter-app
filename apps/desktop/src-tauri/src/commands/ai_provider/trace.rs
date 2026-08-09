//! Structured per-request tracing for provider calls.
//!
//! Split out of `ai_provider/mod.rs` (which sits at its LOC cap) — this is a
//! self-contained tracing concern with no provider logic in it. Re-exported from
//! the parent, so every `RequestTrace::begin` call site is unchanged.

use super::ProviderId;

/// Monotonic id source for [`RequestTrace`]. Process-local and reset on restart —
/// it only has to disambiguate requests within one log file.
static REQUEST_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Structured per-request log over the shared [`crate::observability::Span`].
/// Emits a `→` line at dispatch and a `←` line with status + duration at
/// completion, e.g.:
/// `[ai] ← req=17 provider=openai model=gpt-4o endpoint=/chat/completions … status=200 duration=1842ms ok=true`
pub struct RequestTrace {
    span: crate::observability::Span,
}

impl RequestTrace {
    pub fn begin(
        provider: ProviderId,
        model: &str,
        endpoint: &str,
        base_url: &str,
        streaming: bool,
    ) -> Self {
        // Process-local sequence number, stamped on BOTH the `→` and `←` lines.
        // Without it concurrent requests are unpairable: with several generations
        // in flight the log is a shuffled interleaving of starts and ends, and
        // working out which end belongs to which start means subtracting every
        // reported `duration` from its timestamp by hand. A counter rather than
        // the job id, because research, embeddings and model listing have no job
        // of their own — this covers every provider request uniformly.
        let req = REQUEST_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let fields = format!(
            "req={} provider={} model={} endpoint={} baseUrl={} streaming={}",
            req,
            provider.as_str(),
            model,
            endpoint,
            base_url,
            streaming
        );
        Self {
            span: crate::observability::Span::begin("ai", fields),
        }
    }

    pub fn end(&self, status: Option<u16>, ok: bool) {
        let status = status
            .map(|s| s.to_string())
            .unwrap_or_else(|| "-".to_string());
        self.span.end_with(&format!("status={status}"), ok);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two traces begun in the same process must never share an id — that is the
    /// entire point of the field, and an `Ordering::Relaxed` fetch_add is only
    /// correct here because uniqueness (not ordering) is what is required.
    #[test]
    fn each_request_gets_a_distinct_id() {
        let before = REQUEST_SEQ.load(std::sync::atomic::Ordering::Relaxed);
        let _a = RequestTrace::begin(ProviderId::Ollama, "m", "/e", "http://h", false);
        let _b = RequestTrace::begin(ProviderId::Ollama, "m", "/e", "http://h", false);
        let after = REQUEST_SEQ.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            after - before,
            2,
            "each begin() must consume exactly one id"
        );
    }
}
