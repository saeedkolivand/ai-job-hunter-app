//! The quality-depth stages, one module each, plus the two pure primitives
//! more than one of them needs ([`sections`] and [`verbatim`]).
//!
//! Every stage is a [`Stage<QualityCtx>`](crate::pipeline::Stage) so the run
//! goes through `Pipeline::run_hooked` — which is what gives the L3 shell its
//! cancellation seam (`before`) and its `pipeline:stage` emit point (`after`)
//! without any stage knowing either exists.

mod analyze;
mod cover_letter;
mod draft;
mod evidence;
mod humanize;
mod repair;
pub mod sections;
mod strategy;
mod validate;
pub mod verbatim;

pub use self::analyze::AnalyzeJob;
pub use self::cover_letter::CoverLetter;
pub use self::draft::Draft;
pub use self::evidence::MatchEvidence;
pub use self::humanize::Humanize;
pub use self::repair::NAME as REPAIR_STAGE;
pub use self::repair::{regenerate_one_section, Repair, SectionOutcome, MAX_SECTIONS_PER_ROUND};
pub use self::strategy::{seed_company_roster, Strategy, MAX_COMPANY_PLANS};
pub use self::validate::{validate_documents, Validate};

// The PURE decisions the stages take away from the model — the verbatim drop +
// status overwrite, the roster re-seed, and the repair loop's own arithmetic.
// Re-exported for the sibling test module only: they are stage internals, and a
// production caller reaching for one would be doing the grounding a second
// time, in a second place.
#[cfg(test)]
pub(crate) use self::draft::apply_projects_normalization;
#[cfg(test)]
pub(crate) use self::evidence::ground;
#[cfg(test)]
pub(crate) use self::humanize::{
    exceeds_humanize_cap, humanize_is_worse, humanize_one, is_usable_rewrite,
    should_humanize_letter, voice_count, voice_findings,
};
#[cfg(test)]
pub(crate) use self::repair::{criticals_by_section, repair_loop, round_is_worse};
#[cfg(test)]
pub(crate) use self::strategy::reseed;
#[cfg(test)]
pub(crate) use self::validate::code_histogram;
