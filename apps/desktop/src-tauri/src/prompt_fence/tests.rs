//! Tests for [`super`] — the ADR-010 fencing primitives, moved verbatim out
//! of `agent::tools::test` (PR-5 step 1) so they keep guarding [`fenced`]/
//! [`neutralize_transcript_boundaries`] after `agent` is deleted. Every
//! assertion below is byte-identical to the pre-move version; only the
//! module path changed.
//!
//! Split by topic: forged `<tag>` boundaries, the block's own and its siblings'
//! ([`tag_forgery`]); the `[tool_result:…]` marker pass, the idempotence of the
//! combined transform and `strip_fence_wrapper` ([`transcript`]); and the
//! registry of known tags itself ([`registry`]).

use super::*;

mod registry;
mod tag_forgery;
mod transcript;
