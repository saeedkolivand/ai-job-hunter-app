//! Cover-letter domain helpers.
//!
//! Only the web company-research enricher lives here now — it is consumed by the
//! `ai_research_company` command and folded into the cover-letter "fit" paragraph
//! and company-specific application answers.
//!
//! The legacy server-side cover-letter pipeline (the `generate_cover_letter`
//! command, its inline prompt, and the leakage validator) was removed: the live
//! cover letter is generated through the TS prompt layer and the streaming
//! `generate_pipeline`, which is market-aware. Nothing called the old path.

pub mod research;
