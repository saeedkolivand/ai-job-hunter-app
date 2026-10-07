//! Unit tests for the `ollama` adapter, split by topic (R8b: no inline
//! test-mod body; R8 LOC cap per topic file). Shared fixtures live in
//! [`support`].

mod collect;
mod support;

mod body_effort;
mod capabilities;
mod embed;
mod inspect;
mod models;
mod sampling;
mod search;
mod structured;
mod wire;
