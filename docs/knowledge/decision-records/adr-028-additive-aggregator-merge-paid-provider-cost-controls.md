# ADR-028 — Additive aggregator merge and paid-provider cost controls

**Status:** Accepted
**Date:** 2026-06-30
**Deciders:** project-steward

## Context

The aggregator board (`scraping/boards/aggregator/mod.rs`) previously ran an Adzuna→JSearch
fallback chain. LinkedIn is unreachable by either provider. Adding LinkedIn coverage via Apify
(`ApifyLinkedInProvider`) introduces a new class of provider: paid, slow (up to 300 s timeout),
pay-per-result, reaching a board the primary chain cannot. This creates new risks: silent billing
on autopilot runs, 3x cost from default retries on a non-idempotent POST, and a 300 s
un-cancellable window without explicit select logic.

## Decision

### (a) Additive merge instead of fallback-only

`search_with_providers` now runs the primary chain (Adzuna→JSearch) and, if the LinkedIn provider
is configured, appends its results. `dedupe_by_url` (LinkedIn tracking-param-aware) removes
cross-provider duplicates. Fallback-only was rejected: Adzuna rarely errors for supported
countries, so LinkedIn would almost never run in a fallback model.

### (b) Double opt-in gate: token AND explicit toggle

`ApifyLinkedInProvider::is_configured()` returns `true` only when BOTH conditions hold: an Apify
token is present in the OS keychain AND the `apify_linkedin_enabled` settings toggle is on.
Token-presence alone is insufficient. This prevents silent billing during scheduled autopilot
runs where a user may not realize a paid call fires on every cycle.

### (c) Platform-enforced spend caps, not actor-input fields alone

Cost is bounded by Apify **platform query parameters** (`maxItems`, `maxTotalChargeUsd`) embedded
in the run endpoint URL, in addition to the actor-input `count` field. The platform enforces
`maxItems` and `maxTotalChargeUsd` server-side; a user-overridden actor may silently ignore the
actor-input `count`. Belt-and-suspenders: both caps are set on every call.

### (d) retries: 0 on the billed non-idempotent POST

`run-sync-get-dataset-items` is non-idempotent and billed per result. The shared `FetchOptions`
default is `retries: 2` (designed for idempotent GETs). Applying the default would silently
trigger up to three actor runs and 3x cost. The Apify call sets `retries: 0` explicitly with an
inline comment explaining why.

### (e) Cancellable mid-flight via tokio::select!

The Apify call can block up to 300 s. The shared fetch layer checks `signal.cancelled()` only at
retry boundaries; with `retries: 0` there are none. A `tokio::select!` races the fetch against
`signal.cancelled()` so a user cancel is honoured within one poll cycle rather than waiting for
the full 300 s response window.

### (f) Actor id: curated default, grammar-validated override

User-supplied actor id overrides are grammar-validated at construction via `is_valid_apify_actor_id`
(see `ApifyLinkedInProvider::new()`); if invalid, construction falls back to the default
`curious_coder~linkedin-jobs-scraper` with a warning. The default actor's validity is guaranteed
by the dedicated test `apify_default_actor_passes_validator`, not by a runtime check.

## Alternatives rejected

- **Fallback-only LinkedIn** — LinkedIn would almost never run; Adzuna errors rarely for
  supported countries. The user's intent is additive coverage, not a safety net.
- **Run on token presence alone** — silent billing on every scheduled autopilot run. The double
  gate (token + toggle) is the minimum required.
- **Actor-input `count` as sole cap** — Apify does not guarantee any actor honours its input
  fields, especially a user-overridden one. Platform query params are the enforceable ceiling.

## Consequences

Any future paid or metered external provider must:

1. Use **platform-enforced** spend/item caps (query params the vendor controls), not actor-input
   fields the provider may ignore.
2. Set `retries: 0` on any billed, non-idempotent POST.
3. Wrap long-blocking calls in `tokio::select!` against `signal.cancelled()` at the call site.
4. Require explicit double opt-in (credential present AND user toggle on), never run on
   credential presence alone.

Owning symbols: `ApifyLinkedInProvider`, `search_with_providers` in
`scraping/boards/aggregator/mod.rs`.
