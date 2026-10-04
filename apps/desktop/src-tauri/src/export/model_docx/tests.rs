//! Golden invariants for the model-based DOCX backend. Each test builds a real
//! DOCX and inspects the unzipped OOXML parts (`document.xml` and the hyperlink
//! relationships), since DOCX is flow-based and has no pixel geometry to compare.
//!
//! Split by topic (issue #1280 batch 5b); shared fixtures/helpers live in
//! [`support`].

mod support;

mod content;
mod layout;
mod sizing;
