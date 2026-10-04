//! Content-validation tests.
//!
//! Two layers, on purpose:
//!
//! * **Fixture integration tests** run the whole `validate_content` dispatcher
//!   over realistic en/de résumé + job-ad + generated triples. Each defect
//!   fixture differs from `en_generated_clean.txt` by exactly ONE edit, so a
//!   test that fires the wrong code says so loudly.
//!
//!   The `*_generated_clean.txt` pair are genuine REWORDINGS of their sources —
//!   same employers, dates, links and figures, different sentences. They used
//!   to be near-byte-copies, which made the suite's strongest assertion
//!   (`clean_resume_produces_no_issues_at_all`, an empty report) prove only that
//!   the validators do not fire on text they have already seen. Real generator
//!   output is never a copy, and every false positive this file records was
//!   found on rephrased text.
//! * **Threshold tests** pin every named `const`, because a silently-loosened
//!   threshold is how a validator stops validating.
//!
//! The most important test in the file is
//! `clean_resume_produces_no_issues_at_all`: this module's failure mode is not
//! missing a defect, it is inventing one.

use super::language::MIN_CHARS_FOR_LANGUAGE_CHECK;
use super::*;

mod ats_header;
mod ats_structure;
mod cert_prose;
mod certs;
mod consistency_checks;
mod coverage;
mod credential_calibration;
mod credential_corpus;
mod employment;
mod fixtures;
mod institutions;
mod language_corroboration;
mod language_evidence;
mod language_limits;
mod language_samples;
mod language_sections;
mod links;
mod metric_bands;
mod metrics;
mod project_links;
mod report_contract;
mod sections;
mod skills_labels;
mod support;
mod tenure_claims;
mod tenure_sources;
mod thresholds;
mod voice_checks;
mod voice_lexicon;
