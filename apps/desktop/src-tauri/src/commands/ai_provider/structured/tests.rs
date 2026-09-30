//! Unit tests for `structured.rs` and its per-provider translator sub-modules,
//! split by topic (R8b out-of-line layout).

mod support;

mod anthropic_translator;
mod cross_provider_degrade;
mod gemini_translator;
mod openai_translator;
mod prompt_and_reask;
