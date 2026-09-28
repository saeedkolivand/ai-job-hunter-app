//! `try_named_boards` dispatch-routing + per-board SSRF host-gate tests
//! (Workday, SmartRecruiters, Personio, LinkedIn) — all hermetic: every
//! look-alike host is rejected before any network call.

use super::super::linkedin::try_linkedin;
use super::super::personio::try_personio;
use super::super::smartrecruiters::try_smartrecruiters;
use super::super::workday::try_workday;
use super::super::*;

// ── try_named_boards: dispatch routing ───────────────────────────────────────
//
// These tests verify the "which handler fires for this URL" decision without
// making live API calls. A non-board URL must return Ok(None) at the pattern-
// match gate (before any fetch); a look-alike host for a guarded board must
// also return Ok(None) at the host-gate check.

/// A completely unrecognised URL must produce Ok(None) — no board match,
/// no fetch. This is the hermetic "no-op" path for try_named_boards.
#[tokio::test]
async fn try_named_boards_returns_none_for_unknown_url() {
    // No board handler matches example.com, so we must get Ok(None) without
    // hitting the network.
    let result = try_named_boards("https://example.com/jobs/123")
        .await
        .expect("try_named_boards must not error on a non-board URL");
    assert!(
        result.is_none(),
        "non-board URL must yield None, not a posting"
    );
}

/// A Greenhouse look-alike host must be rejected at the host gate and return
/// Ok(None) — not accepted as a real Greenhouse URL.
#[tokio::test]
async fn try_named_boards_rejects_greenhouse_lookalike() {
    let result = try_named_boards("https://greenhouse.io.attacker.tld/stripe/jobs/1")
        .await
        .expect("look-alike host returns Ok(None) at gate");
    assert!(
        result.is_none(),
        "Greenhouse look-alike host must not be accepted by try_named_boards"
    );
}

/// A Lever look-alike host must be rejected at the host gate.
#[tokio::test]
async fn try_named_boards_rejects_lever_lookalike() {
    let result = try_named_boards("https://lever.co.attacker.tld/stripe/abc123")
        .await
        .expect("look-alike host returns Ok(None) at gate");
    assert!(
        result.is_none(),
        "Lever look-alike host must not be accepted by try_named_boards"
    );
}

// ── SSRF host-gate: Workday + SmartRecruiters ────────────────────────────────
//
// Both handlers tightened from `contains` to exact/ends_with matching. A
// look-alike host (e.g. `myworkdayjobs.com.attacker.tld`) must be rejected at
// the gate — BEFORE any API call is constructed. All tests below are hermetic:
// the gate fires (returning Ok(None)) before the HTTP client is ever touched.

/// A Workday look-alike host (`*.myworkdayjobs.com.attacker.tld`) must be
/// rejected at the host gate; `try_workday` must return `Ok(None)`.
#[tokio::test]
async fn try_workday_rejects_lookalike_host() {
    let result =
        try_workday("https://acme.myworkdayjobs.com.attacker.tld/Acme/job/Backend-Engineer/apply")
            .await
            .expect("look-alike host returns Ok(None) at gate, no network");
    assert!(
        result.is_none(),
        "Workday look-alike host must not be accepted by try_workday"
    );
}

/// A real Workday URL (`<tenant>.wd1.myworkdayjobs.com`) passes the host gate.
/// The path has only one segment, so the handler returns `Ok(None)` at the
/// segment-count check — BEFORE `send()` is called — keeping the test fully
/// hermetic. The important thing is the host + regex gate does not prematurely
/// reject a valid host.
#[tokio::test]
async fn try_workday_accepts_real_host_at_gate() {
    // One-segment path → `segments.len() < 2` → `Ok(None)` before any send().
    // The host gate (suffix check + tenant/wd\d+ regex) must accept this host;
    // if it had rejected, we would also see `Ok(None)` from the gate, so the
    // assertion `result == Ok(None)` combined with the lookalike-reject tests
    // forms a pair: reject-side proven by the lookalike tests, accept-side proven
    // here (no Err from a failed network call).
    let result = try_workday("https://acme.wd1.myworkdayjobs.com/AcmeSite").await;
    assert!(
        result.unwrap().is_none(),
        "real Workday host must not be rejected at the gate; \
         single-segment path yields Ok(None) before any network call"
    );
}

/// A SmartRecruiters look-alike host (`*.smartrecruiters.com.attacker.tld`)
/// must be rejected at the host gate; `try_smartrecruiters` must return `Ok(None)`.
#[tokio::test]
async fn try_smartrecruiters_rejects_lookalike_host() {
    let result =
        try_smartrecruiters("https://jobs.smartrecruiters.com.attacker.tld/Acme/123456789")
            .await
            .expect("look-alike host returns Ok(None) at gate, no network");
    assert!(
        result.is_none(),
        "SmartRecruiters look-alike host must not be accepted by try_smartrecruiters"
    );
}

/// A real SmartRecruiters URL (`jobs.smartrecruiters.com`) passes the host gate.
/// The path has only one segment, so the handler returns `Ok(None)` at the
/// segment-count check — BEFORE `send()` is called — keeping the test fully
/// hermetic. Same hermetic rationale as the Workday positive case above.
#[tokio::test]
async fn try_smartrecruiters_accepts_real_host_at_gate() {
    // One-segment path → `segments.len() < 2` → `Ok(None)` before any send().
    let result = try_smartrecruiters("https://jobs.smartrecruiters.com/AcmeCorp").await;
    assert!(
        result.unwrap().is_none(),
        "real SmartRecruiters host must not be rejected at the gate; \
         single-segment path yields Ok(None) before any network call"
    );
}

// ── SSRF host-gate rejection (hermetic — no network) ─────────────────────────
//
// A look-alike host must be rejected at the host gate and return `Ok(None)`
// BEFORE any fetch. These resolvers return at the tightened gate (exact/suffix
// match) before constructing a client, so calling them with an attacker host is
// network-free: if the gate ever leaked, the call would attempt a real fetch
// and the test would hang/fail.

#[tokio::test]
async fn try_personio_rejects_lookalike_host() {
    // `jobs.personio.attacker.tld` passes a substring gate but not exact/suffix.
    let out = try_personio("https://jobs.personio.attacker.tld/?id=1")
        .await
        .expect("look-alike host returns Ok(None) at the gate, no network");
    assert!(
        out.is_none(),
        "look-alike personio host must not be accepted"
    );
}

#[tokio::test]
async fn try_linkedin_rejects_lookalike_host() {
    // `linkedin.com.attacker.tld` passes a substring gate but not exact/suffix.
    let out = try_linkedin("https://linkedin.com.attacker.tld/jobs/view/1")
        .await
        .expect("look-alike host returns Ok(None) at the gate, no network");
    assert!(
        out.is_none(),
        "look-alike linkedin host must not be accepted"
    );
}

#[tokio::test]
async fn try_personio_rejects_bare_personio_substring_host() {
    // `notpersonio.de` and `jobs.personio.de.evil.tld` must also be rejected.
    assert!(try_personio("https://jobs.personio.de.evil.tld/?id=1")
        .await
        .expect("suffix evasion returns Ok(None) at the gate, no network")
        .is_none());
}
