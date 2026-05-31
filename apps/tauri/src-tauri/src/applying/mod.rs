//! Job application automation.
//!
//! Drives a per-board Chromium instance (via `chromiumoxide`) re-using the
//! persistent profile captured during `board_login::open_login`. Each board's
//! `Applier` impl knows how to navigate the posting URL and fill the form.

pub mod boards;
pub mod captcha_handler;
pub mod error_handler;
pub mod form_filler;
pub mod registry;
pub mod runtime;
pub mod selectors;
pub mod types;

#[allow(unused_imports)]
pub use registry::ApplierRegistry;
