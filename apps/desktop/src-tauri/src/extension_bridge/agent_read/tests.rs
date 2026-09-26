//! `agent_read`'s own test suite — split by topic under the R8 LOC cap, mirroring the
//! production split of `agent_read.rs` into `job.rs`/`job/resolve.rs`, `automations.rs`,
//! `reply.rs`, `throttle.rs`, alongside the pre-existing `best_matches.rs`/`found_jobs.rs`
//! splits. `support` holds the fixtures more than one topic module needs.

mod automations;
mod best_matches_limit;
mod best_matches_paging;
mod best_matches_projection;
mod bounded_refusals;
mod extension_tier;
mod forbidden_key_sweep;
mod job_projection;
mod job_resolve_a;
mod job_resolve_b;
mod job_resolve_c;
mod retry_identity;
mod schema;
pub(in crate::extension_bridge::agent_read) mod support;
mod throttle;
