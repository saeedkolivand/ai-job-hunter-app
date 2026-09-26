//! Unit tests for `assist_registry`, split by behaviour. [`support`] holds
//! the one shared fixture every other topic needs.

mod support;

mod cancel;
mod isolation;
mod races;
mod register;
mod start_and_register;
