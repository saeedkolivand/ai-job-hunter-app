# ADR-003: Centralized platform / net / error layers (L0)

Last updated: 2026-07-16

**Status:** Accepted · Enforced by `cargo test --test architecture` · See [`docs/architecture-rules.md`](../../architecture-rules.md)

## Context

Scattered `std::env::var`, ad-hoc `reqwest::Client` instances, and stringly-typed errors (`Result<_, String>`) make config, networking, and error handling inconsistent and hard to audit (security, retries, observability).

## Decision

Centralize L0 infrastructure: **env/config** only in `platform/` (`config.rs` `data_dir()`), **HTTP** only in `net/` (`http.rs` `shared()`), **typed errors** via `error.rs` (`AppError`/`AppResult`). These are hard rules enforced in CI.

## Consequences

- One place to apply timeouts, proxies, retries, redaction, and path policy.
- Violations fail CI and are HIGH findings at review time (`rust-backend-architect`, deterministic Tier-0 arch-guard in the review-gate).
- New code composes these layers rather than re-implementing them (the user's centralized-architecture preference).
