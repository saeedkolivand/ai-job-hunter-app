//! Unit tests for `trace.rs`.

use super::*;

/// Two traces begun in the same process must never share an id — that is the
/// entire point of the field, and an `Ordering::Relaxed` fetch_add is only
/// correct here because uniqueness (not ordering) is what is required.
///
/// Asserts the two ids DIFFER rather than a delta on the global counter:
/// `REQUEST_SEQ` is process-wide and Rust runs tests in parallel threads, so
/// any other test creating a trace between the two reads would fail a
/// delta assertion while uniqueness — the actual invariant — still holds.
#[test]
fn each_request_gets_a_distinct_id() {
    let a = RequestTrace::begin(ProviderId::Ollama, "m", "/e", "http://h", false);
    let b = RequestTrace::begin(ProviderId::Ollama, "m", "/e", "http://h", false);
    assert_ne!(a.id, b.id, "two traces must never share a request id");
}
