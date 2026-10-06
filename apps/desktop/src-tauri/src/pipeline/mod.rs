//! Reusable workflow orchestration on top of the centralized AI provider layer.
//!
//! Feature generators (resume, cover letter, future workflows) are expressed as
//! a [`Pipeline`] of modular [`Stage`]s rather than bespoke per-feature code.
//! Every stage runs on shared platform infrastructure:
//!
//! * providers / streaming / auth / capabilities — via [`Completer`] and the
//!   centralized [`crate::commands::ai_provider`] layer
//! * research / enrichment — [`enrichment`]
//! * caching — [`cache`]
//! * tracing — [`StageTrace`] (per-stage) on top of the provider `RequestTrace`
//!
//! There is no feature-specific provider, auth, or request flow.
//!
//! [`Completer`] itself is split by concern: construction, accounting, and
//! web-search-backed research ([`completer`]); the actual provider-call
//! methods ([`completion`]). [`Stage`]/[`Pipeline`] orchestration lives in
//! [`stage`].

pub mod budget;
pub mod cache;
mod completer;
mod completion;
pub mod enrichment;
pub mod json;
pub mod resume;
pub mod runs;
mod stage;

pub use completer::Completer;
pub use stage::{Pipeline, Stage, StageHooks, StageInfo, StageOutcome};

// Reachable at `pipeline::X` only for OTHER modules' test code
// (`pipeline::resume::tests`, `commands::ai_provider::tests::effort_tiers`, and
// this module's own `tests`) — every non-test caller already sits inside
// `completer`/`completion` and uses the local name directly, so a non-test
// build has no reader of this path at all.
#[cfg(test)]
pub(crate) use completer::{effort_or_cheapest, low_effort_level};
#[cfg(test)]
pub(crate) use completion::{complete_json_with, text_request};

#[cfg(test)]
mod tests;
