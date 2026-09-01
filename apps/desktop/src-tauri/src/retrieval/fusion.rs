//! Reciprocal Rank Fusion (RRF) — combine several best-first id lists into
//! one ranking, using only each list's RANK per id, never the raw score that
//! produced it. That is what lets this module combine a BM25 list and a
//! cosine-similarity list without knowing either scale exists.

use std::collections::HashMap;

/// The RRF smoothing constant `k`, from Cormack, Clarke & Buettcher,
/// "Reciprocal Rank Fusion Outperforms Condorcet and Individual Rank
/// Learning Methods" (SIGIR 2009): `score(d) = sum(1 / (k + rank(d)))` over
/// every list `d` appears in.
///
/// `60` is the paper's own reported value — chosen there for being robust
/// across its benchmark collections, not tuned to this app's corpus (which
/// has no click-through data to tune against) — and it is the value most
/// citing implementations default to (Elasticsearch's and OpenSearch's own
/// RRF both ship `k=60`). An established constant, stated as one rather than
/// invented.
pub const RRF_K: f64 = 60.0;

/// Fuse `rank_lists` (each best-first; an empty list is a no-op, so a
/// skipped/unavailable arm degrades the fusion to whichever arms DID run,
/// with no special-casing at this call site) into one ranking.
///
/// An id absent from a given list contributes `0` for that list — standard
/// RRF — so appearing in only one of two lists is a real signal, not a
/// disqualification. Returns `(id, fused_score)` sorted by score descending;
/// ties break on id, ascending, for a deterministic order.
pub fn reciprocal_rank_fusion(rank_lists: &[Vec<String>]) -> Vec<(String, f64)> {
    let mut scores: HashMap<&str, f64> = HashMap::new();
    for list in rank_lists {
        for (i, id) in list.iter().enumerate() {
            // 1-based rank, per the RRF formula.
            *scores.entry(id.as_str()).or_insert(0.0) += 1.0 / (RRF_K + (i + 1) as f64);
        }
    }
    let mut fused: Vec<(String, f64)> = scores
        .into_iter()
        .map(|(id, s)| (id.to_string(), s))
        .collect();
    fused.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    fused
}
