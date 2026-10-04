//! Fixtures shared by the `PostingsCache` tests.

use crate::commands::ai_provider::{EmbeddingSpace, EmbeddingVector, EMBEDDING_VECTOR_VERSION};

// ── helpers ──────────────────────────────────────────────────────────────────

pub(super) fn fake_embedding() -> EmbeddingVector {
    EmbeddingVector {
        values: vec![0.1, 0.2, 0.3],
        space: EmbeddingSpace {
            provider: "test".to_string(),
            model: "test-model".to_string(),
            dim: 3,
            version: EMBEDDING_VECTOR_VERSION,
        },
    }
}
