//! The résumé pipeline's Rust-side building blocks.
//!
//! Today this is only [`prompt_blocks`] — the pure prompt rule blocks frozen
//! from `@ajh/prompts` by `pnpm gen:prompts`. The stages that compose them are
//! Phase 3; this module exists now so the generated file has a home and the
//! codegen check can cover it from the start.

pub mod prompt_blocks;
