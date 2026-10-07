//! The résumé pipeline's stage prompts — the ONLY place a quality-depth stage
//! body is written.
//!
//! ## No third copy
//!
//! Every rule that is shared with the renderer-driven prompts is INTERPOLATED
//! from [`super::prompt_blocks`], the file `pnpm gen:prompts` freezes by calling
//! the real `@ajh/prompts` exports. A stage prompt that restated
//! "every claim must be traceable to the résumé" in its own words would be the
//! third copy of a rule that already has one source of truth, and the copies
//! drift in the direction that matters (the strictest wording gets softened by
//! whoever paraphrases last).
//!
//! ## ADR-010: what is trusted and what is fenced
//!
//! * The SYSTEM slot is a fixed Rust string. Nothing that came off a job board,
//!   out of a user's file, or out of a model ever reaches it.
//! * The USER slot carries the untrusted material, each blob inside its own
//!   `<tag>` fence built by `fenced`, which caps it and neutralizes every
//!   known fence tag and tool-result marker inside it.
//! * **Prior-stage model output is UNTRUSTED and fenced too.** `job_analysis`,
//!   `evidence_map` and `resume_strategy` are model text derived from a scraped
//!   posting; treating them as trusted just because this app produced the JSON
//!   would launder an injected instruction through one hop. Their tags are
//!   registered in `crate::prompt_fence::FENCE_TAG_PATTERNS` so a forged sibling
//!   cannot ride in inside another block either. `repair_user`'s
//!   `<document_context>` is the same category: a sibling-section anchor cut
//!   from the SAME generated document a repair round is correcting.
//!
//! One file per stage — [`analyze_job`], [`strategy`], [`draft`], [`repair`],
//! [`cover_letter`], [`humanize`] — sharing the caps and language helpers in
//! [`shared`]. This file only re-exports, so every external caller keeps
//! importing from `pipeline::resume::prompts::*` unchanged.

mod analyze_job;
mod cover_letter;
mod draft;
mod humanize;
mod repair;
mod shared;
mod strategy;

pub use analyze_job::{analyze_job_user, ANALYZE_JOB_SYSTEM};
pub use cover_letter::{letter_system, letter_user};
pub use draft::{draft_system, draft_user};
pub use humanize::{
    humanize_patch_schema, humanize_rewrite_system, humanize_system, humanize_user, HumanizeTier,
    HUMANIZE_PATCH_EXAMPLE,
};
pub use repair::{repair_system, repair_user};
pub use strategy::{company_roster_block, strategy_system, strategy_user};

pub(in crate::pipeline::resume) use cover_letter::LETTER_INTENT;
pub(in crate::pipeline::resume) use draft::draft_language_retry_note;
pub(in crate::pipeline::resume) use humanize::HUMANIZE_DOCUMENT_CAP;
pub(in crate::pipeline::resume) use shared::SIBLING_CONTEXT_CAP;

// Test-only surface: `language_name` and `section_order_prompt_list` are
// called directly by other prompt functions in this module's submodules
// (never through this re-export) — only `pipeline::resume::tests` imports
// them by this path, so a non-test build has no user of the re-export.
#[cfg(test)]
pub(in crate::pipeline::resume) use draft::section_order_prompt_list;
#[cfg(test)]
pub(in crate::pipeline::resume) use shared::language_name;
