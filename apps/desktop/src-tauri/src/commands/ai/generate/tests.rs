use std::sync::Arc;

use super::*;
use crate::limits::Limiter;

// ── admit_embed_tests ───────────────────────────────────────────────────
// `admit_embed` is now just `ai_embed`'s rate/concurrency admission — the
// per-provider daily charge moved to `MeteredAttempt`
// (`commands::ai_provider::embed`, see its own tests in
// `ai_provider/embed/tests.rs`), which fires once per ACTUAL provider round-trip
// instead of once per admitted call (#1087). The rate/concurrency
// primitives themselves are already covered generically by `limits::tests`,
// so only a normal-admission smoke test remains here.

#[test]
fn admit_embed_admits_a_normal_call_under_the_production_caps() {
    let limiter = Arc::new(Limiter::new());
    assert!(admit_embed(
        &limiter,
        crate::limits::AI_EMBED_RATE_MAX,
        crate::limits::AI_EMBED_CONCURRENCY_MAX,
    )
    .is_ok());
}
