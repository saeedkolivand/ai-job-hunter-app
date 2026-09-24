//! The quality-depth stages, one module each, plus the pure primitives shared
//! between them ([`sections`]).
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

pub use self::analyze::AnalyzeJob;
pub use self::cover_letter::CoverLetter;
pub use self::draft::Draft;
pub use self::evidence::MatchEvidence;
pub use self::humanize::Humanize;
pub use self::repair::NAME as REPAIR_STAGE;
pub use self::repair::{regenerate_one_section, Repair, SectionOutcome, MAX_SECTIONS_PER_ROUND};
pub use self::strategy::{seed_company_roster, Strategy, MAX_COMPANY_PLANS};
pub use self::validate::{validate_documents, Validate};

// The PURE decisions the stages take away from the model — the roster re-seed,
// the repair loop's own arithmetic, and the rest. Re-exported for the sibling
// test module only: they are stage internals, and a production caller reaching
// for one would be duplicating a decision the owning stage already makes.
// `evidence` keeps its own tests in its own module, so nothing from it is
// re-exported here.
#[cfg(test)]
pub(crate) use self::cover_letter::research_company_brief;
#[cfg(test)]
pub(crate) use self::draft::{
    apply_projects_normalization, draft_with_language_retry, run_draft_attempt, DraftEnv,
    LanguageRetryOutcome,
};
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
