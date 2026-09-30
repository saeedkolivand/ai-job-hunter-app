//! Tests for the shell layer's own decisions: the wire contract's shape, the
//! report wrapper the renderer reads, and the two run-stopping seams.
//!
//! Each guard names the mutation that makes it fail; each was applied and
//! reverted, not assumed.
//!
//! Split by topic: the wire contract shape ([`wire`]); the quality report
//! wrapper ([`report_wrapper`], [`report_fabrications`], [`report_review`]);
//! `hooks`'s cancel/timeout/terminal-state decisions ([`hooks_apply`],
//! [`hooks_run_timeout`], [`hooks_terminal`]); provider admission
//! ([`admission`]); `resolve`'s clamp/id-wins/store decisions
//! ([`resolve_clamp`], [`resolve_source`], [`resolve_job`],
//! [`resolve_unlinked`]); the run store round trip ([`run_store_roundtrip`]);
//! a moved/edited aggregate document ([`aggregate_divergence`]); the
//! Notification Center card ([`notify`]); the regenerate-section command's
//! Projects normalization and résumé-provenance guard
//! ([`regenerate_normalize`], [`regenerate_wrote_resume`]); and
//! `persist_document`'s Application lookup + save gates ([`persist_gate`],
//! [`persist_lookup`]). Shared fixtures in [`support`].

mod admission;
mod aggregate_divergence;
mod hooks_apply;
mod hooks_run_timeout;
mod hooks_terminal;
mod notify;
mod persist_gate;
mod persist_lookup;
mod regenerate_normalize;
mod regenerate_wrote_resume;
mod report_fabrications;
mod report_review;
mod report_wrapper;
mod resolve_clamp;
mod resolve_job;
mod resolve_source;
mod resolve_unlinked;
mod run_store_roundtrip;
mod support;
mod wire;
