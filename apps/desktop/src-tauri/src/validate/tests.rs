//! Validation tests. The `evaluate` logic is unit-tested on synthetic extracted
//! text (deterministic, no rendering), and `validate_and_fix` is exercised
//! end-to-end against real generated PDFs/DOCX to guard against false-positive
//! blocking of valid documents.

use super::header_links::canonicalize_url;
use super::pdf_links::{page_link_annotations, topmost_n, PdfLink};
use super::readback::{evaluate, expected_from_request, normalize, Expected};
use super::*;
use crate::export::types::{DocumentType, GenerationMeta, LetterLayout, TemplateId};

mod band_links;
mod profile_links;
mod readback_checks;
mod roundtrip;
mod support;
