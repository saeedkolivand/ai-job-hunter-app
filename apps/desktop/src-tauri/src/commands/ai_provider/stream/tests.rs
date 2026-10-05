//! Unit tests for `stream.rs` and its `finish`/`text` sub-modules, split by
//! topic (R8b out-of-line layout). Mirrors the `openai.rs` + `openai/tests.rs`
//! precedent of moving the test module itself out rather than production code.

mod support;

mod finish_and_messages;
mod loop_core;
mod usage_and_answer;
