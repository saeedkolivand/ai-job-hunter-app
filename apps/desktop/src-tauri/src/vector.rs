//! Shared vector math (L0 shared infra).
//!
//! Pure, dependency-free cosine similarity. Hoisted here so BOTH the AI provider
//! layer (embedding comparison, `commands::ai_provider::compare`) and the
//! scraping cluster layer (cross-board dedup, `scraping::cluster`) reuse ONE
//! implementation without a lower→higher layer import (architecture rule R7):
//! `scraping` (L1) reaching into `commands` (L3) for `cosine` would be an upward
//! edge, so the function lives in L0 and both layers depend downward on it.
//!
//! `commands::ai_provider` re-exports `cosine` (`pub use crate::vector::cosine`)
//! so its existing callers are unchanged; the space-checked `compare` wrapper
//! stays there alongside `EmbeddingVector`.

/// Raw cosine similarity between two vectors.
///
/// Returns `0.0` for a length mismatch, an empty input, or a zero-magnitude
/// vector — incomparable/degenerate inputs never yield a spurious non-zero score.
/// Prefer `commands::ai_provider::compare` for stored embeddings so the embedding
/// SPACE is checked before the raw math.
pub fn cosine(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::cosine;

    #[test]
    fn identical_vectors_score_one() {
        let a = [1.0, 2.0, 3.0];
        assert!((cosine(&a, &a) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn orthogonal_vectors_score_zero() {
        assert!((cosine(&[1.0, 0.0], &[0.0, 1.0]) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn degenerate_inputs_score_zero() {
        assert_eq!(cosine(&[], &[]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0); // length mismatch
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0); // zero magnitude
    }
}
