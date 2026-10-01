//! Tests for the Typst engine — smoke tests + model-based + ATS harness.
//!
//! All tests run fully in-process (no disk, no network) via the offline
//! ResumeWorld hard-wall. Split by topic (issue #1280 batch 5a); shared
//! fixtures/helpers live in `fixtures`, `resume_fixtures`, `letter_fixtures`,
//! `pdf_introspect`, and `svg_geometry`.

mod fixtures;
mod letter_fixtures;
mod pdf_introspect;
mod resume_fixtures;
mod svg_geometry;

mod aria;
mod atelier;
mod atelier_samples;
mod ats_harness;
mod awesome_deedy;
mod cadence_regent;
mod classic_render;
mod education_style;
mod every_template;
mod lebenslauf;
mod letter_banded_navy;
mod letter_classic;
mod letter_completion_guardrail;
mod letter_layouts_misc;
mod letter_layouts_reading;
mod letter_refined;
mod letter_refined_locale;
mod letter_sidebar;
mod letter_sidebar_containment;
mod meridian_throughline;
mod portrait;
mod render_preview;
mod resolve_photo_integration;
mod saffron;
mod showcase_letter_previews;
mod showcase_resume_banner;
mod stray_typst_guard;
mod svg_measurement;
mod swiss_minimal_academic;
