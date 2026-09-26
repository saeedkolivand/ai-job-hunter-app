//! Unit tests for `answer_assist`, split by behaviour. [`support`] holds the
//! fixtures shared across topics.
//!
//! Some topics here (`parsing`, `rewrite_resolution`) exercise the sibling
//! `answer_assist_parse.rs`/`answer_assist_topic.rs` pure functions AT THIS
//! VERB'S OWN CALL SITE, since that is where they always lived.

mod support;

mod compose_retry_core;
mod compose_retry_guards;
mod context;
mod gate_and_errors;
mod grounding_cancel;
mod grounding_salary;
mod grounding_web_notes;
mod parsing;
mod prompt;
mod registry_ownership;
mod reply;
mod rewrite_resolution;
mod topic_validation;
