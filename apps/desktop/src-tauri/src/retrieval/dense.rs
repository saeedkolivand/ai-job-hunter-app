//! Dense (embedding) ranking — pure cosine similarity over already-embedded
//! vectors.
//!
//! **`&[f32]` on purpose, never `&[f64]` and never `serde_json::Value`.**
//! This is NOT a performance fix — the measured 116.2 ms JSON-parsing
//! bottleneck (N=2000, dim=768) is a property of the PERSISTED
//! `posting_vectors` SQLite table (JSON-text `f64` columns), which this
//! module never touches: `postings::PostingsCache` holds live vectors as
//! native `EmbeddingVector` structs in memory, so there is no JSON to parse
//! on this path at all. The `&[f32]` signature is a type-level guarantee
//! instead: it makes it impossible for a future caller to hand this module
//! an `EmbeddingSpace`-bearing type at all — the caller (L3, where the
//! embedding provider's `Vec<f64>` naturally lives) must cast down to bare
//! `f32` values before crossing into L1, which is also the point where the
//! embedding-space comparison the caller must not skip (see
//! `commands::hybrid_search`'s `dense_pair`) has to happen — this module
//! never sees an `EmbeddingSpace` and could not enforce that rule itself.

/// Cosine similarity between two equal-length vectors, in `[-1.0, 1.0]` for
/// any non-degenerate pair. `None` when the vectors can't be compared: a
/// dimension mismatch (different embedding spaces — the caller's job to
/// avoid, not this function's to guess at) or either vector has zero
/// magnitude (no direction to compare against).
pub fn cosine(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.is_empty() || a.len() != b.len() {
        return None;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return None;
    }
    Some(dot / (norm_a.sqrt() * norm_b.sqrt()))
}

/// Rank `candidates` by cosine similarity to `query`, best (most similar)
/// first. A candidate [`cosine`] can't score (dimension mismatch, zero
/// vector) is dropped rather than sorted arbitrarily — the same
/// degrade-silently-not-wrongly posture as the rest of this crate's scoring
/// paths. Ties break on id, ascending, so the order is deterministic across
/// runs and hosts (float equality is otherwise not a stable sort key).
pub fn rank_by_similarity(query: &[f32], candidates: &[(String, Vec<f32>)]) -> Vec<String> {
    let mut scored: Vec<(&str, f32)> = candidates
        .iter()
        .filter_map(|(id, v)| cosine(query, v).map(|score| (id.as_str(), score)))
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    scored.into_iter().map(|(id, _)| id.to_string()).collect()
}
