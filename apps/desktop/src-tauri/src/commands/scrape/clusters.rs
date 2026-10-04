//! Cross-board duplicate clustering over the live postings cache (ADR-029 §b).
//! Split out of `commands/scrape.rs` for R8 (issue #1280); `scrape.rs` re-exports
//! `recluster_postings_cache`, so it keeps its `commands::scrape::` path.

use std::collections::HashMap;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::postings::PostingsCache;
use crate::scraping::cluster::{
    assign_clusters, posting_cluster_input, ClusterAssignment, ClusterInput,
};

/// Recompute cross-board clusters over the live [`PostingsCache`] and patch each
/// item's cluster annotations in place (ADR-029 §b). Store-aware — it needs the
/// user's tombstone verdicts, the agency extras, and the cached posting vectors —
/// so it lives in the L3 command layer, not the store-blind engine. Idempotent
/// and side-effect-free beyond the cache patch; safe to call after every ingest.
///
/// NEVER embeds: a cached-vector lookup is a MISS unless the row is present AND
/// in the active embedding space, in which case its vector feeds the cosine path;
/// otherwise the pair falls onto the trigram string path. A missing store, an
/// empty cache, or a read error degrades to "no annotations", never a failure.
pub fn recluster_postings_cache(app: &AppHandle) {
    // Durable + preference inputs (best-effort snapshots).
    let tombstones = app
        .try_state::<crate::dedup::DedupStore>()
        .map(|s| s.all_pairs())
        .unwrap_or_default();
    let extra_agency = app
        .try_state::<crate::job_preferences::JobPreferencesStore>()
        .map(|s| s.get().extra_agency_companies.unwrap_or_default())
        .unwrap_or_default();

    // Snapshot the cache items under-lock, then release before the per-item
    // DocumentStore vector reads (never hold two store locks at once).
    let Some(cache) = app.try_state::<Mutex<PostingsCache>>() else {
        return;
    };
    let items: Vec<Value> = cache.lock().get_all().to_vec();
    if items.is_empty() {
        return;
    }

    // Active embedding space — cached posting vectors in ANY other space are a
    // miss (mirrors `posting_vector_is_fresh`'s space check, without the
    // text-hash requirement, and WITHOUT ever embedding).
    let doc_store = app.try_state::<crate::documents::DocumentStore>();
    let active = doc_store.as_ref().map(|s| s.embedding_config());

    let mut ids: Vec<String> = Vec::with_capacity(items.len());
    let mut inputs: Vec<ClusterInput> = Vec::with_capacity(items.len());
    for item in &items {
        // The cache stores serialized `JobPosting`s; deserialize through the SAME
        // type production ingests so the cluster-input mapping can't drift (the
        // shared `posting_cluster_input` seam is also exercised by the aggregator
        // acceptance test). A cache entry that isn't a well-formed posting — or
        // carries no id to annotate — is skipped, never breaking the whole run.
        let Ok(posting) = serde_json::from_value::<crate::scraping::JobPosting>(item.clone())
        else {
            continue;
        };
        if posting.id.trim().is_empty() {
            continue;
        }

        let (vector, space) = match (doc_store.as_ref(), active.as_ref()) {
            (Some(store), Some(cfg)) => store
                .get_posting_vector(&posting.id)
                .filter(|(v, _)| cfg.matches(&v.space))
                .map(|(v, _)| (Some(v.values), Some(v.space.to_string())))
                .unwrap_or((None, None)),
            _ => (None, None),
        };

        ids.push(posting.id.clone());
        inputs.push(posting_cluster_input(&posting, vector, space));
    }

    let assignments = assign_clusters(inputs, &tombstones, &extra_agency);

    // Zip verdicts back onto ids by index (assign_clusters preserves input order).
    let by_id: HashMap<String, Value> = ids
        .into_iter()
        .zip(assignments.iter())
        .map(|(id, a)| (id, cluster_annotation_json(a)))
        .collect();
    cache.lock().apply_cluster_annotations(&by_id);
}

/// Serialize a [`ClusterAssignment`] to the annotation object patched onto a
/// cache item: `clusterId`, `clusterCanonical`, `clusterMembers` `[{key,board?,url}]`,
/// `isAgency` (ADR-029 §e). `board` is omitted when absent.
fn cluster_annotation_json(a: &ClusterAssignment) -> Value {
    let members: Vec<Value> = a
        .members
        .iter()
        .map(|m| {
            let mut obj = serde_json::Map::new();
            obj.insert("key".to_string(), json!(m.key));
            if let Some(board) = &m.board {
                obj.insert("board".to_string(), json!(board));
            }
            obj.insert("url".to_string(), json!(m.url));
            Value::Object(obj)
        })
        .collect();
    json!({
        "clusterId": a.cluster_id,
        "clusterCanonical": a.canonical,
        "clusterMembers": members,
        "isAgency": a.is_agency,
    })
}
