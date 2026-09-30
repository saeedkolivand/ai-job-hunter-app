//! Split by what each topic exercises: [`extraction`] (`candidates`'s
//! extraction order via the `extract_json` test helper, and `repair_json`),
//! [`parse`] (`parse`'s error classification and content-free formatting),
//! and [`ordering`] (the HIGH-1/MEDIUM-3 candidate-ordering hardening).
//! Shared fixtures in [`support`].

mod extraction;
mod ordering;
mod parse;
mod support;
