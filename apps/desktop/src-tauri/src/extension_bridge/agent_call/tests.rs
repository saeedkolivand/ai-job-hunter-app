//! `agent_call`'s own test suite — split by topic under the R8 LOC cap (`docs/architecture-rules.md`),
//! mirroring the production split of `agent_call.rs` into `policy_lookup.rs`, `refusal.rs`,
//! `reply.rs`, `dispatch.rs` and the pre-existing `fence.rs`/`reshape.rs` splits. Every topic
//! module reaches the unit under test the same way this hub always did — `use super::super::*`
//! (now two levels up) plus the same handful of sibling globs the original single file used.

mod agent_call_frame_cap;
mod dispatch_classify;
mod dispatch_confirm;
mod dispatch_contact_profile;
mod fence_basic;
mod fence_document_shape_a;
mod fence_document_shape_b;
mod fence_extra_basic;
mod fence_extra_fixtures;
mod fence_extra_leaves;
mod fence_shape_answer;
mod fence_shape_job_record;
mod fence_shape_scrape;
mod fence_tags;
mod fence_unfence;
mod policy_lookup;
mod refusal;
mod reply_clamp;
mod reply_core;
mod reply_extension;
mod reshape_base64;
mod reshape_contact_profile;
mod reshape_drop_fields;
mod reshape_order;
mod reshape_paging;
mod reshape_scalar_fence;
mod support;
