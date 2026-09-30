//! Unit tests for the `openai` adapter, split by topic (R8b: no inline
//! test-mod body; R8 LOC cap per topic file). Shared fixtures live in
//! [`support`].

mod support;

mod body_effort;
mod capabilities;
mod endpoint_url;
mod list_models;
mod sampling;
mod structured;
mod transport;
mod web_search;
mod wire_stream;
mod wire_turn;
