//! Dense (embedding) ranking — pure cosine similarity over already-embedded
//! vectors.
//!
//! **`&[f32]` on purpose, never `&[f64]` and never `serde_json::Value`.**
//! Measured (N=2000, dim=768): a full scan over vectors stored as JSON-text
//! `f64` costs 116.2 ms, of which JSON parsing alone is 115.8 ms (~99.7%) —
//! the cosine math itself is noise. This module's API makes that mistake
//! impossible to reintroduce here: there is no code path from a JSON `Value`
//! to a similarity score that does not go through an explicit, one-time
//! `Vec<f64> -> Vec<f32>` cast at the caller's L3 boundary (where the
//! embedding provider's `Vec<f64>` naturally lives) — this module never
//! parses anything.

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
