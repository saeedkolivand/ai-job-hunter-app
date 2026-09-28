//! Derivation + persistence tests for the per-board reliability history.
//!
//! Every assertion is anchored to an ABSOLUTE expected value (a literal
//! timestamp, a literal streak length, a literal status) rather than to a second
//! derived value — a regression that broke both sides of a `derived == derived`
//! comparison would keep such a test green.

mod derivation;
mod flaky_window;
mod persistence;
mod support;
