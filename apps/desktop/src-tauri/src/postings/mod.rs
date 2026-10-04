/// PostingsCache — buffers live job items streamed from the in-process scraper engine.
/// InteractionStore — records user interactions (viewed, applied, bookmarked).
///
/// Both are in-memory with optional JSON file persistence.
/// The Electron equivalents are DataRuntime.liveJobs and the NeDB jobInteractions
/// collection. This Rust version intentionally avoids a full DB dependency —
/// data is written to <dataDir>/interactions.json as a flat JSON array.
use std::collections::HashMap;

use serde_json::Value;

use crate::commands::ai_provider::EmbeddingVector;

mod interactions;

pub use self::interactions::{attach_interactions, InteractionRecord, InteractionStore};

// ── PostingsCache ─────────────────────────────────────────────────────────────

/// Live job postings received during an active scrape.
/// Cleared on `scrape_clear_postings` or on next scrape start for the same board.
#[derive(Default)]
pub struct PostingsCache {
    items: Vec<Value>,
    /// Embedding cache keyed by posting id, populated lazily by hybrid search so
    /// repeat searches over the same live postings don't re-embed. Each entry
    /// carries its embedding space so stale-space entries can be detected.
    embeddings: HashMap<String, EmbeddingVector>,
    /// Bumped on every [`Self::clear_all`] — the ONLY mutation that can make an
    /// in-flight hybrid search's already-computed results describe postings
    /// that no longer exist (a replace-scrape's first streamed item calls
    /// `clear_all` under this same lock, per `commands::scrape`'s first-item-
    /// clear latch). A search snapshots this at start and compares again
    /// before returning, refusing to answer against a corpus that was
    /// cleared mid-flight. Deliberately NOT bumped by [`Self::add`],
    /// [`Self::update_description`], or [`Self::clear_embeddings`]: `add`/
    /// `update_description` only add or patch a row, so a posting a search
    /// already found is still a valid, still-present result; `clear_embeddings`
    /// (a settings-driven embedding-space change, or `ai_reembed_all`) leaves
    /// `items` untouched, so an already-computed ranking is still correct over
    /// what's still there — bumping on it would discard a fully-correct
    /// result and falsely tell the user their corpus changed. A stale-space
    /// vector an in-flight dense arm re-seeds right after `clear_embeddings`
    /// runs is dead weight, never scored: every read is space-guarded
    /// (`EmbeddingConfig::matches`) — so this counter deliberately covers
    /// `clear_all` only, by design, not every mutation that touches the cache.
    generation: u64,
}

impl PostingsCache {
    /// Insert a streamed posting, upserting by its `"id"` string.
    ///
    /// "Show more" re-scrapes with the same search signature (`replace=false`), so
    /// the same postings stream in again and would otherwise be appended a second
    /// time — the backend cache returned by `scrape_list_postings` then contained
    /// duplicates of the first batch. To prevent that, an incoming item whose `"id"`
    /// already exists **replaces that entry in place** (preserving its position /
    /// insertion order); the latest copy wins. An item with no `"id"` or a null id
    /// always pushes — distinct id-less rows must not be collapsed onto each other.
    ///
    /// Linear scan over a `Vec` is correct here: the frontend caps the list at ~500
    /// items, so the O(n) scan is cheap and a HashMap/index map would be premature.
    /// [`Self::update_description`] does its own linear scan too, matched by `url`
    /// (not `id` — see that method's own doc for why, issue #1106).
    ///
    /// When a replace happens, any cached embedding for that id is dropped
    /// ONLY when the embedded TEXT actually changed (title + description —
    /// the same fields [`text_fields`] reads, which are the ones
    /// `documents::keywords::posting_text_blob` builds the dense-arm embed
    /// text from). This mirrors [`Self::update_description`]'s own "only
    /// invalidate on a text change" rule, and for the identical reason:
    /// "Show more" re-streams the SAME search signature, so a re-fetched
    /// posting is usually byte-identical apart from bookkeeping fields like
    /// `capturedAt` (stamped fresh on every fetch) — invalidating on every
    /// re-stream regardless would silently re-pay up to
    /// `commands::hybrid_search::DENSE_CANDIDATE_MAX` embeds for text hybrid
    /// search already had a good vector for.
    pub fn add(&mut self, item: Value) {
        if let Some(incoming_id) = item.get("id").and_then(Value::as_str).map(str::to_string) {
            if let Some(pos) = self.items.iter().position(|existing| {
                existing.get("id").and_then(Value::as_str) == Some(incoming_id.as_str())
            }) {
                let text_changed = text_fields(&self.items[pos]) != text_fields(&item);
                self.items[pos] = item;
                if text_changed {
                    self.embeddings.remove(&incoming_id);
                }
                return;
            }
        }
        self.items.push(item);
    }

    pub fn get_all(&self) -> &[Value] {
        &self.items
    }

    /// Patch the `description` of every cached posting whose own `url` field
    /// normalizes (via [`crate::applications::normalize_job_url`]) to
    /// `normalized_url`, in place (issue #1106).
    ///
    /// Aggregator list scrapes store only a truncated snippet; once the detail
    /// pane resolves the full description we write it back here so the match
    /// scorer (which reads title+description+requirements from this cache) sees
    /// the full text. We mutate the EXISTING entry rather than pushing a new one:
    /// [`Self::add`] upserts by id (it would replace, not duplicate, the row), so
    /// routing the patch through `add` would needlessly rebuild the whole value.
    /// Each item is stored as a JSON object, so we patch the `description` field
    /// on the matching object directly.
    ///
    /// Addressed by `url`, not the board-synthetic `id` this method used before:
    /// `id` has no meaning off this in-memory cache (no Agent/MCP read command
    /// ever exposed it — `extension_bridge::agent_read`'s own module doc: "`url`
    /// is the CROSS-RESOURCE KEY — not an id"), so `url` is the one identity a
    /// caller — the renderer or the agent CLI — can actually supply.
    /// `normalized_url` must already be normalized: `scrape_update_description`
    /// normalizes once and reuses that same value for this cache AND for
    /// `AutopilotStore::update_found_job_descriptions`, never re-derived per item.
    ///
    /// Returns `true` when AT LEAST ONE entry was updated, `false` when no
    /// item's url matches (no row is created in either case). Every matching
    /// item is patched, not just the first — two board-synthetic ids can
    /// legitimately share one url within a single session.
    pub fn update_description(&mut self, normalized_url: &str, description: &str) -> bool {
        // Two-pass approach to avoid holding a simultaneous mutable borrow on
        // `self.items` while also mutating `self.embeddings` (which is keyed by
        // `id`, not `url` — each matched item's OWN id is collected in pass 1).
        let mut stale_embedding_ids: Vec<String> = Vec::new();
        let mut found = false;
        for item in &mut self.items {
            let item_url = item.get("url").and_then(Value::as_str).unwrap_or("");
            if crate::applications::normalize_job_url(item_url) != normalized_url {
                continue;
            }
            let Some(obj) = item.as_object_mut() else {
                continue;
            };
            found = true;
            // Only invalidate the cached embedding when the text actually
            // changes. If the full description is identical to what's
            // already stored (e.g. a duplicate resolve-on-open call) we
            // keep the embedding; otherwise the stale snippet embedding
            // would be reused on the next score after a description update,
            // defeating the resolve-on-open re-score.
            let existing = obj.get("description").and_then(Value::as_str).unwrap_or("");
            if existing != description {
                if let Some(id) = obj.get("id").and_then(Value::as_str) {
                    stale_embedding_ids.push(id.to_string());
                }
            }
            obj.insert(
                "description".to_string(),
                Value::String(description.to_string()),
            );
        }
        // Pass 2: drop stale embeddings now that `self.items` borrow is released.
        for id in stale_embedding_ids {
            self.embeddings.remove(&id);
        }
        found
    }

    pub fn clear_all(&mut self) {
        self.items.clear();
        self.embeddings.clear();
        self.generation = self.generation.wrapping_add(1);
    }

    /// The current corpus generation — see the field doc on [`Self::generation`].
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn get_embedding(&self, id: &str) -> Option<EmbeddingVector> {
        self.embeddings.get(id).cloned()
    }

    pub fn set_embedding(&mut self, id: String, vector: EmbeddingVector) {
        self.embeddings.insert(id, vector);
    }

    /// Drop cached embeddings (keeping items) — used when the embedding space
    /// changes so stale-space vectors aren't reused. Deliberately does NOT
    /// bump [`Self::generation`] — see that field's doc for why: `items` is
    /// untouched, so an in-flight search's already-computed ranking is still
    /// correct, and bumping here would falsely report a changed corpus.
    pub fn clear_embeddings(&mut self) {
        self.embeddings.clear();
    }

    /// Merge cross-board cluster annotations onto cached items IN PLACE, keyed by
    /// posting `id` (ADR-029). Each `by_id` value is a JSON object of annotation
    /// fields (`clusterId`, `clusterCanonical`, `clusterMembers` `[{key,board,url}]`,
    /// `isAgency`) computed by `recluster_postings_cache`; every field is copied
    /// onto the matching item. An id not in the cache is skipped (a cluster was
    /// recomputed for a row a newer search already evicted) — no row is ever
    /// created, matching the cache's ephemeral, upsert-by-id lifecycle.
    pub fn apply_cluster_annotations(&mut self, by_id: &HashMap<String, Value>) {
        if by_id.is_empty() {
            return;
        }
        for item in &mut self.items {
            let Some(id) = item.get("id").and_then(Value::as_str).map(str::to_string) else {
                continue;
            };
            let Some(annotation) = by_id.get(&id).and_then(Value::as_object) else {
                continue;
            };
            if let Some(obj) = item.as_object_mut() {
                for (field, value) in annotation {
                    obj.insert(field.clone(), value.clone());
                }
            }
        }
    }
}

/// The fields whose change should invalidate a posting's cached embedding —
/// see [`PostingsCache::add`]. `(title, description)`, the same two fields
/// `documents::keywords::posting_text_blob` reads to build the dense-arm
/// embed text; anything else changing (location, board metadata, `capturedAt`)
/// is irrelevant to what got embedded.
fn text_fields(item: &Value) -> (String, String) {
    let field = |name: &str| {
        item.get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    (field("title"), field("description"))
}

#[cfg(test)]
mod tests;
