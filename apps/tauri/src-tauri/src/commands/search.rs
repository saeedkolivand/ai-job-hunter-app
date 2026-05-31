use std::collections::HashSet;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::commands::ai_provider::compare;
use crate::documents::embed;
use crate::ipc_contracts::search::HybridSearchRequest;
use crate::postings::PostingsCache;

/// Hybrid search over the live job postings: weighted blend of semantic
/// (query-embedding cosine) and keyword overlap. Returns `SearchHit[]`
/// (`{ id, score, payload }`) sorted by score, capped at `topK`.
///
/// Only the `jobs` collection is searchable today; others return `[]`.
/// Degrades to keyword-only when Ollama is offline.
#[tauri::command]
pub async fn search_hybrid(app: AppHandle, req: HybridSearchRequest) -> Value {
    if req.collection != "jobs" {
        return json!([]);
    }
    let query = req.query.trim();
    if query.is_empty() {
        return json!([]);
    }

    let postings: Vec<Value> = {
        let cache = app.state::<Mutex<PostingsCache>>();
        let guard = cache.lock();
        guard.get_all().to_vec()
    };
    if postings.is_empty() {
        return json!([]);
    }

    let query_vec = embed(&app, query).await;
    let query_kw = keywords(query);
    let weight = req.semantic_weight.clamp(0.0, 1.0);

    let mut hits: Vec<(f64, Value)> = Vec::new();
    for posting in &postings {
        let Some(id) = posting.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let text = posting_text(posting);

        let keyword = keyword_score(&query_kw, &keywords(&text));
        let semantic = match &query_vec {
            Some(q) => match posting_embedding(&app, id, &text).await {
                Some(v) => compare(q, &v).map(|s| s.clamp(0.0, 1.0)).unwrap_or(0.0),
                None => 0.0,
            },
            None => 0.0,
        };

        let score = if query_vec.is_some() {
            weight * semantic + (1.0 - weight) * keyword
        } else {
            keyword
        };

        if score > 0.0 {
            hits.push((score, posting.clone()));
        }
    }

    hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    hits.truncate(req.top_k.max(1) as usize);

    let out: Vec<Value> = hits
        .into_iter()
        .map(|(score, payload)| {
            let id = payload.get("id").and_then(|v| v.as_str()).unwrap_or("");
            json!({ "id": id, "score": (score * 1000.0).round() / 1000.0, "payload": payload })
        })
        .collect();
    json!(out)
}

/// Title + description text for a posting.
fn posting_text(posting: &Value) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(t) = posting.get("title").and_then(|v| v.as_str()) {
        parts.push(t);
    }
    if let Some(d) = posting.get("description").and_then(|v| v.as_str()) {
        parts.push(d);
    }
    parts.join("\n")
}

/// Embedding for a posting, cached in `PostingsCache` so repeat searches over
/// the same live postings don't re-embed. The cache entry carries its space; a
/// cached vector from a stale space is discarded and re-embedded.
async fn posting_embedding(
    app: &AppHandle,
    id: &str,
    text: &str,
) -> Option<crate::commands::ai_provider::EmbeddingVector> {
    let active = app
        .state::<crate::documents::DocumentStore>()
        .embedding_config();
    {
        let cache = app.state::<Mutex<PostingsCache>>();
        let cached = cache.lock().get_embedding(id);
        if let Some(v) = cached {
            if active.matches(&v.space) {
                return Some(v);
            }
        }
    }
    if text.trim().is_empty() {
        return None;
    }
    let vector = embed(app, text).await?;
    {
        let cache = app.state::<Mutex<PostingsCache>>();
        cache.lock().set_embedding(id.to_string(), vector.clone());
    }
    Some(vector)
}

/// Lowercase tokens longer than 2 chars.
fn keywords(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '+' && c != '#')
        .map(|w| {
            w.trim_matches(|c: char| c == '+' || c == '#')
                .to_lowercase()
        })
        .filter(|w| w.len() > 2)
        .collect()
}

/// Fraction of query keywords present in the candidate text (0–1).
fn keyword_score(query: &HashSet<String>, candidate: &HashSet<String>) -> f64 {
    if query.is_empty() {
        return 0.0;
    }
    let matched = query.intersection(candidate).count();
    matched as f64 / query.len() as f64
}

#[cfg(test)]
mod test {
    use super::*;

    fn set(words: &[&str]) -> HashSet<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn keyword_score_full_and_partial() {
        let q = set(&["rust", "react", "docker"]);
        assert_eq!(keyword_score(&q, &set(&["rust", "react", "docker"])), 1.0);
        assert!((keyword_score(&q, &set(&["rust", "react"])) - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(keyword_score(&q, &set(&["python"])), 0.0);
        assert_eq!(keyword_score(&HashSet::new(), &set(&["rust"])), 0.0);
    }

    #[test]
    fn keywords_drops_short_tokens() {
        let kw = keywords("Go is a fun language");
        assert!(kw.contains("fun"));
        assert!(kw.contains("language"));
        assert!(!kw.contains("go")); // 2 chars
        assert!(!kw.contains("is"));
    }
}
