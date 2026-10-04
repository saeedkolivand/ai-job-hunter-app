//! Tests for `adapter.rs`, split by topic: the document shape and header built
//! from a whole résumé ([`structure`]), job-entry recognition ([`entries`]),
//! Projects regrouping ([`projects`]) and the headings that never reach the model
//! ([`dropped_sections`]). [`support`] holds the lookups the siblings share.

use super::*;

mod dropped_sections;
mod entries;
mod projects;
mod structure;
mod support;
