//! Cross-board job clustering (ADR-029): a deterministic, pure function that
//! groups the "same job" surfaced across boards/aggregators into one cluster,
//! recomputed at every ingest (no persisted membership).
//!
//! Matching (ADR-029 §c): items are BLOCKED by exact normalized-company +
//! normalized-title-first-token, killing cross-role/cross-company merges up
//! front. Within a block a greedy pass joins each item to the first cluster
//! whose seed it matches — by cosine ≥ [`CLUSTER_COSINE_MIN`] when BOTH carry a
//! same-space embedding, else trigram-Jaccard of normalized titles ≥
//! [`CLUSTER_TITLE_TRIGRAM_JACCARD_MIN`]. A user "not a duplicate" tombstone
//! against ANY current member vetoes the join, so a split survives every
//! re-scrape (ADR-029 §h). The cached-vector hit rate is ≈0 today, so the string
//! path is primary and the cosine path is dormant-but-correct.
//!
//! The module is store- and Tauri-blind: callers snapshot tombstones + agency
//! extras and hand in plain [`ClusterInput`]s, so the whole surface is unit
//! testable without a runtime (see the L3 wiring in `commands::scrape` and the
//! L2 wiring in `autopilot`).

pub mod normalize;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::scraping::boards::aggregator::AGGREGATOR_BOARD_ID;
use crate::vector::cosine;

use normalize::{
    is_agency_with, normalize_agency_extras, normalize_company, normalize_title, title_first_token,
};

/// Cosine floor for the embedding-similarity join path (ADR-029 §c).
pub const CLUSTER_COSINE_MIN: f64 = 0.92;
/// Trigram-Jaccard floor for the string-similarity join path (ADR-029 §c).
pub const CLUSTER_TITLE_TRIGRAM_JACCARD_MIN: f64 = 0.90;

/// One posting fed to [`assign_clusters`]. `key` is the app-wide
/// `canonical_job_key` (the SAME identity tombstones are keyed on and the
/// renderer echoes back). `vector`/`space` drive the cosine path when both are
/// present and the spaces match; otherwise the normalized-title string path runs.
#[derive(Debug, Clone)]
pub struct ClusterInput {
    pub key: String,
    pub title: String,
    pub company: String,
    pub url: String,
    /// Board id (`JobPosting.source`); an aggregator source de-prioritizes the
    /// item as a cluster canonical.
    pub source: Option<String>,
    pub has_description: bool,
    pub seen_at: u64,
    /// Cached posting embedding, in the ACTIVE space only (the caller filters).
    pub vector: Option<Vec<f64>>,
    /// Embedding-space identity for `vector`; cosine runs only when two items
    /// share a non-`None` space (never mixes spaces).
    pub space: Option<String>,
}

/// A member of a cluster, echoed to the renderer so it can group rows and hand
/// `key`s back to `dedup_mark_not_duplicate`. Opaque to the UI (ADR-029 §e).
/// Serde-camelCase so it persists directly on `FoundJob` and matches the shared
/// `{ key, board?, url }` TS type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterMemberRef {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    pub url: String,
}

/// The clustering verdict for one input item (returned in INPUT order, one per
/// item, so a caller zips it back onto its own rows by index). `cluster_id` is
/// the canonical member's key; `canonical` marks the canonical member itself;
/// `members` lists every member of the cluster.
#[derive(Debug, Clone)]
pub struct ClusterAssignment {
    pub cluster_id: String,
    pub canonical: bool,
    pub is_agency: bool,
    pub members: Vec<ClusterMemberRef>,
}

/// Jaccard similarity of the char-trigram sets of two strings, each padded so
/// leading/trailing chars still form trigrams. Two empty strings are identical
/// (`1.0`); an empty vs a non-empty is `0.0`. stdlib `HashSet` only.
pub fn trigram_jaccard(a: &str, b: &str) -> f64 {
    let ta = trigrams(a);
    let tb = trigrams(b);
    match (ta.is_empty(), tb.is_empty()) {
        (true, true) => 1.0,
        (true, _) | (_, true) => 0.0,
        _ => {
            let inter = ta.intersection(&tb).count();
            let union = ta.len() + tb.len() - inter;
            if union == 0 {
                0.0
            } else {
                inter as f64 / union as f64
            }
        }
    }
}

/// Char-trigram set of `s`, padded with two leading + two trailing spaces so the
/// edges are represented. An empty string has no trigrams.
fn trigrams(s: &str) -> HashSet<[char; 3]> {
    if s.is_empty() {
        return HashSet::new();
    }
    let padded: Vec<char> = format!("  {s}  ").chars().collect();
    let mut set = HashSet::with_capacity(padded.len());
    for w in padded.windows(3) {
        set.insert([w[0], w[1], w[2]]);
    }
    set
}

/// Order two keys into the canonical `(key_a, key_b)` tombstone shape
/// (`key_a < key_b`) — the SAME invariant `DedupStore::pair` enforces on write.
fn ordered_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// Whether the user has marked `a` and `b` "not a duplicate".
fn has_tombstone(tombstones: &HashSet<(String, String)>, a: &str, b: &str) -> bool {
    if a == b {
        return false;
    }
    tombstones.contains(&ordered_pair(a, b))
}

/// Per-item precomputed normalized fields + the original index.
struct Prepared {
    idx: usize,
    norm_title: String,
    /// `(normalized_company, title_first_token)`; `None` when either is empty →
    /// the item is forced into its own singleton cluster (ADR-029 §c).
    block: Option<(String, String)>,
}

/// A cluster under construction within one block: `seed` is the first (most
/// canonical) member; `members` are original indices in join order.
struct Cluster {
    seed: usize,
    members: Vec<usize>,
}

/// Assign every input item to a cluster (ADR-029 §c–e). Returns one
/// [`ClusterAssignment`] per input, in INPUT ORDER, so a caller can zip the
/// verdicts straight back onto its own rows by index.
///
/// `tombstones` are unordered `canonical_job_key` pairs (`key_a < key_b`); a
/// tombstone against ANY current member vetoes a join. `extra_agency` are the
/// user's additional agency company names (normalized the same way as built-ins).
pub fn assign_clusters(
    items: Vec<ClusterInput>,
    tombstones: &HashSet<(String, String)>,
    extra_agency: &[String],
) -> Vec<ClusterAssignment> {
    // 1) Precompute normalized fields + block keys.
    let prepared: Vec<Prepared> = items
        .iter()
        .enumerate()
        .map(|(idx, it)| {
            let norm_title = normalize_title(&it.title);
            let norm_company = normalize_company(&it.company);
            let first_token = title_first_token(&it.title);
            let block = if norm_company.is_empty() || first_token.is_empty() {
                None
            } else {
                Some((norm_company, first_token))
            };
            Prepared {
                idx,
                norm_title,
                block,
            }
        })
        .collect();

    // 2) Group item indices by block; unblockable items each get a unique group.
    let mut blocks: HashMap<(String, String), Vec<usize>> = HashMap::new();
    let mut singletons: Vec<usize> = Vec::new();
    for p in &prepared {
        match &p.block {
            Some(key) => blocks.entry(key.clone()).or_default().push(p.idx),
            None => singletons.push(p.idx),
        }
    }

    // idx -> (cluster_id, canonical, members) once each block is resolved.
    let mut cluster_of: HashMap<usize, (String, bool, Vec<ClusterMemberRef>)> = HashMap::new();

    // 3) Resolve each block into clusters.
    for member_indices in blocks.into_values() {
        resolve_block(
            member_indices,
            &items,
            &prepared,
            tombstones,
            &mut cluster_of,
        );
    }
    // Singletons: each is its own cluster (no comparison to run).
    for idx in singletons {
        let member = member_ref(&items[idx]);
        cluster_of.insert(idx, (items[idx].key.clone(), true, vec![member]));
    }

    // 4) Emit assignments in input order. The agency extras are normalized ONCE
    //    here and reused for every posting's `is_agency` check (O(N) instead of
    //    re-normalizing the whole extras list per posting).
    let normalized_extras = normalize_agency_extras(extra_agency);
    items
        .iter()
        .enumerate()
        .map(|(idx, it)| {
            let (cluster_id, canonical, members) = cluster_of
                .remove(&idx)
                .unwrap_or_else(|| (it.key.clone(), true, vec![member_ref(it)]));
            ClusterAssignment {
                cluster_id,
                canonical,
                is_agency: is_agency_with(&it.company, &normalized_extras),
                members,
            }
        })
        .collect()
}

/// Greedy-cluster the items of one block and record each item's verdict.
fn resolve_block(
    mut member_indices: Vec<usize>,
    items: &[ClusterInput],
    prepared: &[Prepared],
    tombstones: &HashSet<(String, String)>,
    cluster_of: &mut HashMap<usize, (String, bool, Vec<ClusterMemberRef>)>,
) {
    // Sort by canonical preference (ADR-029 §e): has_description desc →
    // non-aggregator source first → seen_at desc → key asc. The last key is
    // total, so the order (and thus every cluster seed / id) is deterministic.
    member_indices.sort_by(|&a, &b| {
        let ia = &items[a];
        let ib = &items[b];
        ib.has_description
            .cmp(&ia.has_description)
            .then_with(|| is_aggregator(ia).cmp(&is_aggregator(ib)))
            .then_with(|| ib.seen_at.cmp(&ia.seen_at))
            .then_with(|| ia.key.cmp(&ib.key))
    });

    let mut clusters: Vec<Cluster> = Vec::new();
    for &idx in &member_indices {
        let mut joined = None;
        for (ci, cluster) in clusters.iter().enumerate() {
            // Tombstone veto: a split against ANY current member forbids the join.
            let vetoed = cluster
                .members
                .iter()
                .any(|&m| has_tombstone(tombstones, &items[idx].key, &items[m].key));
            if vetoed {
                continue;
            }
            if similar(items, prepared, idx, cluster.seed) {
                joined = Some(ci);
                break;
            }
        }
        match joined {
            Some(ci) => clusters[ci].members.push(idx),
            None => clusters.push(Cluster {
                seed: idx,
                members: vec![idx],
            }),
        }
    }

    for cluster in &clusters {
        let cluster_id = items[cluster.seed].key.clone();
        let members: Vec<ClusterMemberRef> = cluster
            .members
            .iter()
            .map(|&m| member_ref(&items[m]))
            .collect();
        for &m in &cluster.members {
            cluster_of.insert(m, (cluster_id.clone(), m == cluster.seed, members.clone()));
        }
    }
}

/// Whether item `a` joins the cluster seeded by `b`: cosine ≥ floor when both
/// carry a same-space vector, else trigram-Jaccard of normalized titles ≥ floor.
fn similar(items: &[ClusterInput], prepared: &[Prepared], a: usize, b: usize) -> bool {
    let ia = &items[a];
    let ib = &items[b];
    match (&ia.vector, &ib.vector, &ia.space, &ib.space) {
        (Some(va), Some(vb), Some(sa), Some(sb)) if sa == sb => {
            cosine(va, vb) >= CLUSTER_COSINE_MIN
        }
        _ => {
            trigram_jaccard(&prepared[a].norm_title, &prepared[b].norm_title)
                >= CLUSTER_TITLE_TRIGRAM_JACCARD_MIN
        }
    }
}

/// True when the item's board id is the aggregator (Adzuna → JSearch) source.
fn is_aggregator(it: &ClusterInput) -> bool {
    it.source.as_deref() == Some(AGGREGATOR_BOARD_ID)
}

fn member_ref(it: &ClusterInput) -> ClusterMemberRef {
    ClusterMemberRef {
        key: it.key.clone(),
        board: it.source.clone(),
        url: it.url.clone(),
    }
}

/// Map a scraped [`JobPosting`](crate::scraping::JobPosting) (+ an optional
/// active-space embedding) to a [`ClusterInput`] — the SINGLE production seam so
/// the manual-scrape ingest (`commands::scrape::recluster_postings_cache`) and
/// the cross-board acceptance test share ONE mapping and can't drift apart.
/// `key` is the app-wide `canonical_job_key`, `source` is the trimmed board id
/// (empty → `None`), `has_description` reflects non-blank text, and `seen_at`
/// comes from `captured_at` (a negative timestamp clamps to 0). Autopilot maps
/// from `FoundJob` (a different shape) via its own `found_job_cluster_inputs`.
pub(crate) fn posting_cluster_input(
    posting: &crate::scraping::JobPosting,
    vector: Option<Vec<f64>>,
    space: Option<String>,
) -> ClusterInput {
    let source = {
        let s = posting.source.trim();
        (!s.is_empty()).then(|| s.to_string())
    };
    ClusterInput {
        key: crate::scraping::boards::common::canonical_job_key(
            &posting.url,
            &posting.title,
            &posting.company,
        ),
        title: posting.title.clone(),
        company: posting.company.clone(),
        url: posting.url.clone(),
        source,
        has_description: posting
            .description
            .as_deref()
            .is_some_and(|d| !d.trim().is_empty()),
        seen_at: u64::try_from(posting.captured_at).unwrap_or(0),
        vector,
        space,
    }
}

/// Number of clusters whose members are ALL first-seen this run — the
/// "New jobs: N" count (ADR-029 §f). A known job resurfacing on another board
/// makes its cluster non-all-new, contributing 0. `new_keys` holds the
/// `canonical_job_key`s the caller considers new.
pub fn new_cluster_count(assignments: &[ClusterAssignment], new_keys: &HashSet<String>) -> u32 {
    let mut counted: HashSet<&str> = HashSet::new();
    let mut count = 0u32;
    for a in assignments {
        if !counted.insert(a.cluster_id.as_str()) {
            continue; // this cluster was already tallied
        }
        if !a.members.is_empty() && a.members.iter().all(|m| new_keys.contains(&m.key)) {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests;
