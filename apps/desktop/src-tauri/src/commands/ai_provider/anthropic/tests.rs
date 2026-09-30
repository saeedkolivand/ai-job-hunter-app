//! Unit tests for the `anthropic` adapter, split by topic (R8b: no inline
//! test-mod body; R8 LOC cap per topic file). Shared fixtures live in
//! [`support`].

mod support;

mod body;
mod chat_body;
mod effort;
mod list_models;
mod sampling;
mod thinking;
mod transport;
mod wire;
