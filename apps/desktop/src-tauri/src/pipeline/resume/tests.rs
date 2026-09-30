//! Tests for the résumé pipeline's PURE decisions — the ones that hold whatever
//! a model returns.
//!
//! Every guard here was mutation-checked: the comment on each test names the
//! change that makes it fail, and each was applied and reverted rather than
//! assumed. A test that passes with its feature deleted is not a guard.
//!
//! Split by topic, one file per area of the pipeline this crate tests:
//! stage-cache-key discipline ([`cache`]), strategy/evidence-map prompts
//! ([`strategy`]), the `ResumeStrategy` wire shape ([`types`]), section
//! splice/replacement-acceptance ([`sections_splice`]) and the sibling-context
//! anchor ([`sections_context`]), the pipeline's own stage list
//! ([`pipeline_stages`]) and per-stage completer routing ([`routing`]),
//! prompt fencing/forgery-resistance ([`prompts_fence`]) and the
//! language/localization prompts ([`prompts_draft_language`],
//! [`prompts_draft_shape`], [`prompts_letter`]), the `cover_letter` research
//! seam ([`cover_letter`]), `QualityCtx`/`RunLedger` ([`ctx`]), the run
//! deadline ([`deadline`]), `draft` ([`draft`]), the repair judge
//! ([`repair_judge`], [`repair_judge_cross_section`]) and loop
//! ([`repair_loop`], [`repair_loop_deadline`], [`repair_loop_replies`]), the
//! humanize predicates ([`humanize_predicates`]) and one-document attempt
//! ([`humanize_attempt_gates`], [`humanize_attempt_outcomes`]), source/project
//! seeding ([`source_seed`]), and whole-run integration shapes
//! ([`pipeline_integration`]).

mod cache;
mod cover_letter;
mod ctx;
mod deadline;
mod draft;
mod humanize_attempt_gates;
mod humanize_attempt_outcomes;
mod humanize_predicates;
mod pipeline_integration;
mod pipeline_stages;
mod prompts_draft_language;
mod prompts_draft_shape;
mod prompts_fence;
mod prompts_letter;
mod repair_judge;
mod repair_judge_cross_section;
mod repair_loop;
mod repair_loop_deadline;
mod repair_loop_replies;
mod routing;
mod sections_context;
mod sections_splice;
mod source_seed;
mod strategy;
mod support;
mod types;
