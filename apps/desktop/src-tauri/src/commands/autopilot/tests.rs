//! Unit tests for `commands::autopilot`, split by topic under the R8 LOC cap: the phase-1 keyword
//! [`filters`] and [`retain`]ed rows, the [`build_found_job`] projection, the command module's own
//! plumbing ([`plumbing`]), the phase-2 re-rank ([`rerank_loop`], [`rerank_phase`],
//! [`rerank_candidates`], [`rerank_cost`]) and the [`resume_cache`] cleanup. [`support`] holds the
//! fixtures they share.

mod build_found_job;
mod filters;
mod plumbing;
mod rerank_candidates;
mod rerank_cost;
mod rerank_loop;
mod rerank_phase;
mod resume_cache;
mod retain;
mod support;
