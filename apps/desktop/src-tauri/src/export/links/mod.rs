//! Hyperlink / label helpers for contact lines.
//!
//! The implementations moved to [`crate::model::rich`] (the unified rich-text
//! model). These thin re-exports keep existing renderer imports
//! (`crate::export::links::{…}`) working unchanged.

pub use crate::model::rich::{split_urls, Span};
