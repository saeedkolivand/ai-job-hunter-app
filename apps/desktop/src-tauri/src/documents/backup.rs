//! The backup surface: [`DocumentStore`]'s `DataStore` impl (export / import) and
//! the exported-vector reader it restores through.
//!
//! Split out of `documents/mod.rs` (R8's hard LOC cap); everything here moved
//! verbatim.

use crate::commands::ai_provider::{EmbeddingSpace, EmbeddingVector};
use crate::data_store::DataStore;
use crate::error::AppResult;
use crate::observability::sanitize_reason;

use super::{DocumentRecord, DocumentStore};

/// Read an exported document's optional embedding vector out of its JSON row.
///
/// Legacy exports carry no `vectorSpace` — they predate cloud embeddings and were
/// all Ollama/nomic-embed-text, which is what the fallback records. The export
/// JSON never carries a `version` (see `export()` below), so every imported
/// vector is tagged `version: 0` — never [`EMBEDDING_VECTOR_VERSION`] — so
/// `EmbeddingConfig::matches` always treats a just-imported vector as stale
/// and re-embeds it. Conservative on purpose: we genuinely don't know which
/// format version produced a vector from another install/app version.
fn parse_exported_vector(item: &serde_json::Value) -> Option<EmbeddingVector> {
    let values: Vec<f64> = item
        .get("vector")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_f64())
        .collect();
    if values.is_empty() {
        return None;
    }
    let dim = values.len();
    let space = item
        .get("vectorSpace")
        .map(|s| EmbeddingSpace {
            provider: s
                .get("provider")
                .and_then(|v| v.as_str())
                .unwrap_or("ollama")
                .to_string(),
            model: s
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("nomic-embed-text")
                .to_string(),
            dim: s
                .get("dim")
                .and_then(|v| v.as_u64())
                .map(|d| d as usize)
                .unwrap_or(dim),
            version: 0,
        })
        .unwrap_or_else(|| EmbeddingSpace {
            provider: "ollama".to_string(),
            model: "nomic-embed-text".to_string(),
            dim,
            version: 0,
        });
    Some(EmbeddingVector { values, space })
}

impl DataStore for DocumentStore {
    fn key(&self) -> &'static str {
        "documents"
    }

    fn export(&self) -> serde_json::Value {
        let docs: Vec<serde_json::Value> = self
            .list()
            .into_iter()
            .map(|rec| {
                let mut obj = serde_json::to_value(&rec).unwrap_or_else(|_| serde_json::json!({}));
                if let Some(ev) = self.get_vector(&rec.id) {
                    obj["vector"] = serde_json::json!(ev.values);
                    obj["vectorSpace"] = serde_json::json!({
                        "provider": ev.space.provider,
                        "model": ev.space.model,
                        "dim": ev.space.dim,
                    });
                }
                obj
            })
            .collect();
        serde_json::json!(docs)
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        let items = data.as_array().ok_or("documents: expected an array")?;
        // Deserialize EVERY row before mutating the store. `clear_all` wipes
        // documents, vectors, posting_vectors AND match_scores, so a malformed
        // row (hand-edited bundle, newer schema, corruption) reached after that
        // call used to destroy the user's whole document library + embeddings and
        // still return Err — nothing left to restore from. The sibling stores
        // (applications, ai_generations, dedup, discovered) all validate up-front
        // for exactly this reason; documents was the lone outlier.
        let parsed: Vec<(DocumentRecord, Option<EmbeddingVector>)> = items
            .iter()
            .map(|item| {
                let record: DocumentRecord =
                    serde_json::from_value(item.clone()).map_err(|e| e.to_string())?;
                Ok((record, parse_exported_vector(item)))
            })
            .collect::<AppResult<_>>()?;

        self.clear_all();
        let mut count = 0;
        let mut default_id: Option<String> = None;
        for (record, vector) in &parsed {
            if record.is_default {
                default_id = Some(record.id.clone());
            }
            // A pre-#955 bundle carries BOTH the corrupt text and the vector
            // derived from it. `insert()` repairs the text and, if it
            // changed, deletes any latent vector for this id — but that
            // happens BEFORE the `upsert_vector` below would restore the
            // bundle's own (corrupt-derived) one, so it must be skipped
            // here too or it would just get written right back.
            let text_was_repaired = matches!(
                crate::extraction::pdf::repair_utf16_mojibake(&record.text),
                std::borrow::Cow::Owned(_)
            );
            self.insert(record)?;
            if let Some(vector) = vector {
                if !text_was_repaired {
                    // A `<namespace>:` id is refused by the document-index write
                    // guard (`is_synthetic_scoring_id`). Unreachable for a bundle
                    // this app produced — `export()` only walks real `documents`
                    // rows — but a hand-edited backup must not BRICK the restore:
                    // `clear_all()` has already run, so propagating here would
                    // leave the library half-restored with nothing to retry from.
                    // Skip the one vector (it re-embeds on demand) and keep going.
                    if let Err(e) = self.upsert_vector(&record.id, vector) {
                        log::warn!(
                            "[documents] import: skipping the embedding of one restored document ({})",
                            sanitize_reason(&e.to_string())
                        );
                    }
                }
            }
            count += 1;
        }
        // insert() auto-defaults the first row; restore the originally-default doc.
        if let Some(id) = default_id {
            self.set_default(&id)?;
        }
        Ok(count)
    }
}
