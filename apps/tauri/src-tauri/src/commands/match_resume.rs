use std::collections::HashSet;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::documents::keywords::{apply_stemmer, keywords, make_stemmer};
use crate::documents::{
    embed, posting_vector_or_embed, sha256_hex, DocumentRecord, DocumentStore, EmbeddingConfig,
    MatchScoreKey,
};
use crate::ipc_contracts::matching::{MatchResumeBatchRequest, MatchResumeRequest};
use crate::ipc_contracts::resume::ResumeExtractTextRequest;
use crate::postings::PostingsCache;

/// Score a resume against a job posting.
///
/// Returns a `MatchScore` (see packages/shared types): a semantic score from
/// embedding cosine similarity, an ATS score from job-keyword coverage, a
/// weighted `combined` score, the missing keywords (`gaps`), and short
/// recommendations. Degrades gracefully to keyword-only when Ollama is offline.
/// Cache-busting version for the match_scores result cache. Bump whenever the
/// 0.6/0.4 weighting, the combined-score formula, or the keyword/stemmer logic
/// changes — any of which would make a previously-cached score stale.
const MATCH_FORMULA_VERSION: i64 = 1;

/// Server-side cap on `match_resume_batch` job ids. Generous vs. realistic
/// boards — `livePostings` caps at 500 — but bounds a malicious direct IPC
/// invoke (Zod validation at the wire boundary is type-only, not enforced
/// server-side) from fanning out an unbounded CPU/embed-spend batch.
const MATCH_BATCH_MAX: usize = 1000;

/// Map the `semantic_scoring_enabled` request flag to the `semantic_enabled`
/// cache-key column: `Some(false)` disables semantic scoring (`0`); every other
/// value (`None` / `Some(true)`) keeps it on (`1`). Single source of this bit so
/// the cache key and the skip-branch can't drift; unit-tested directly.
fn semantic_enabled_bit(flag: Option<bool>) -> i64 {
    if flag == Some(false) {
        0
    } else {
        1
    }
}

/// Score a single resume against one job posting, returning a `MatchScore`
/// JSON value (or a `{ "error": … }` object when the job isn't cached).
///
/// This is the per-job kernel shared by [`match_resume`] (one job) and
/// [`match_resume_batch`] (N jobs in one IPC call). `resume_raw_keywords` is the
/// parsed `keywords_json` (parsed ONCE by the caller); `None` — absent or corrupt
/// JSON — falls back to live extraction from `resume.text`, preserving the legacy
/// behaviour. `active` is the embedding config and `semantic_enabled` the
/// already-derived cache-key bit, both hoisted by the caller so a batch resolves
/// them once.
///
/// Errors-never-cached invariant: the only error return (job-not-found) happens
/// before any `get_match_score`/`upsert_match_score`, so an error path can never
/// read or pollute the result cache.
async fn score_one(
    app: &AppHandle,
    store: &DocumentStore,
    resume: &DocumentRecord,
    resume_raw_keywords: Option<&[String]>,
    active: &EmbeddingConfig,
    job_id: &str,
    semantic_enabled: i64,
) -> Value {
    let Some(job_text) = job_text_for(app, job_id) else {
        return json!({ "error": format!("job not found in cache: {}", job_id) });
    };

    // Optional, local-only translation: when the JD language differs from the
    // resume locale and a local provider is configured, translate before keyword
    // extraction (and embedding) so matching happens in the resume language.
    // Always falls back to the original text on any failure. Cloud providers are
    // excluded, so this never incurs an unexpected API cost.
    let target_lang = resume.locale.as_deref().unwrap_or("en");
    let translated =
        crate::commands::translation::translate_if_needed(app, job_id, &job_text, target_lang)
            .await;
    let job_text = translated; // shadow with the owned String; downstream code unchanged

    // `semantic_enabled` is the cache-key bit; `skip_semantic` is its inverse.
    let skip_semantic = semantic_enabled == 0;

    // Self-invalidating result cache: the key captures every input that can
    // change the score (ids, embedding space, semantic on/off, formula version,
    // and a hash of the final job text). A hit skips embedding + cosine +
    // keyword work entirely. The job-not-found error above is returned before
    // this point and is never cached.
    let job_text_hash = sha256_hex(&job_text);
    let cache_key = MatchScoreKey {
        resume_id: &resume.id,
        job_id,
        provider: &active.provider,
        model: &active.model,
        semantic_enabled,
        formula_version: MATCH_FORMULA_VERSION,
        job_text_hash: &job_text_hash,
    };
    if let Some(cached) = store.get_match_score(&cache_key) {
        return cached;
    }
    let (resume_vec, job_vec) = if skip_semantic {
        (None, None)
    } else {
        let rv = match store.get_vector(&resume.id) {
            Some(v) if active.matches(&v.space) => Some(v),
            _ => {
                let v = embed(app, &resume.text).await;
                if let Some(ref ev) = v {
                    let _ = store.upsert_vector(&resume.id, ev);
                }
                v
            }
        };
        let jv = posting_vector_or_embed(app, job_id, &job_text).await;
        (rv, jv)
    };
    let semantic = match (&resume_vec, &job_vec) {
        (Some(a), Some(b)) => crate::commands::ai_provider::compare(a, b)
            .map(|s| (s.clamp(0.0, 1.0) * 100.0).round())
            .unwrap_or(0.0),
        _ => 0.0, // embeddings unavailable or disabled.
    };

    // ATS: how many job keywords appear in the resume text. Both sides are
    // stemmed with one stemmer whose language is detected from the JD, since the
    // job-ad language defines the matching context.
    let stemmer = make_stemmer(&job_text);
    let job_keywords = keywords(&job_text, &stemmer);
    // Reuse the résumé's cached normalized keywords when present, stemming them
    // with the JD-derived stemmer so matching stays in the job-ad's language.
    // Legacy documents (imported before the cache existed) and corrupt JSON
    // arrive here as `None` and fall back to live extraction from the stored text.
    let resume_words: HashSet<String> = match resume_raw_keywords {
        Some(tokens) => apply_stemmer(tokens.iter().cloned().collect(), &stemmer),
        None => keywords(&resume.text, &stemmer),
    };
    let (ats, gaps) = keyword_coverage(&job_keywords, &resume_words);

    let combined = if job_vec.is_some() {
        (0.6 * semantic + 0.4 * ats).round()
    } else {
        ats // no semantic signal available
    };

    let recommendations = recommendations(&gaps);
    let explanation = if skip_semantic {
        format!(
            "Keyword coverage {ats:.0}% across {} job keywords (semantic scoring disabled).",
            job_keywords.len()
        )
    } else {
        format!(
            "Semantic similarity {semantic:.0}%, keyword coverage {ats:.0}% across {} job keywords.",
            job_keywords.len()
        )
    };

    let result = json!({
        "resumeId": resume.id,
        "jobId": job_id,
        "ats": ats,
        "semantic": semantic,
        "combined": combined,
        "gaps": gaps,
        "recommendations": recommendations,
        "explanation": explanation,
    });
    if let Ok(s) = serde_json::to_string(&result) {
        store.upsert_match_score(&cache_key, &s).ok();
    }
    result
}

/// Parse the résumé's cached normalized keywords (`keywords_json`) into a token
/// list. Absent OR corrupt JSON → `None`, which makes [`score_one`] fall back to
/// live extraction from `resume.text` (the legacy behaviour).
fn parse_resume_keywords(resume: &DocumentRecord) -> Option<Vec<String>> {
    resume
        .keywords_json
        .as_deref()
        .and_then(|j| serde_json::from_str::<Vec<String>>(j).ok())
}

#[tauri::command]
pub async fn match_resume(app: AppHandle, req: MatchResumeRequest) -> Value {
    let store = app.state::<DocumentStore>();
    // INVARIANT (errors-never-cached): every error early-return MUST precede the
    // first `get_match_score`/`upsert_match_score` call. The resume-not-found
    // guard below returns before any cache access; `score_one`'s job-not-found
    // early-return likewise precedes its first cache call. So an error path can
    // never read or pollute the result cache. See
    // `errors_never_populate_match_scores_cache` in documents/test.rs, which
    // pins the store-level non-pollution half.
    let Some(resume) = store.get(&req.resume_id) else {
        return json!({ "error": format!("resume not found: {}", req.resume_id) });
    };

    // Parse the résumé's cached keywords ONCE (absent/corrupt → None → live
    // extraction fallback inside `score_one`).
    let resume_raw_keywords = parse_resume_keywords(&resume);
    let active = store.embedding_config();
    let semantic_enabled = semantic_enabled_bit(req.semantic_scoring_enabled);

    score_one(
        &app,
        &store,
        &resume,
        resume_raw_keywords.as_deref(),
        &active,
        &req.job_id,
        semantic_enabled,
    )
    .await
}

/// Score one resume against MANY job postings in a single IPC call.
///
/// Default keyword-only scoring is CPU-cheap, but the per-row renderer scheduler
/// otherwise serialises N `match_resume` IPC round-trips. This scores every
/// posting in one pass: the résumé record, its parsed keywords, the embedding
/// config, and the semantic-enabled bit are resolved ONCE, then each job is
/// scored sequentially via [`score_one`]. Sequential (not `join_all`) so the
/// opt-in semantic branch can't fan out a burst of concurrent embeds.
///
/// Returns a JSON array of `MatchScore` values, one per requested job in order.
/// A missing job yields a `{ "error": … }` element and does NOT abort the loop.
/// Résumé-not-found returns a single `{ "error": … }` object (not an array).
#[tauri::command]
pub async fn match_resume_batch(app: AppHandle, req: MatchResumeBatchRequest) -> Value {
    let store = app.state::<DocumentStore>();
    let Some(resume) = store.get(&req.resume_id) else {
        return json!({ "error": format!("resume not found: {}", req.resume_id) });
    };

    let resume_raw_keywords = parse_resume_keywords(&resume);
    let active = store.embedding_config();
    let semantic_enabled = semantic_enabled_bit(req.semantic_scoring_enabled);

    let job_ids: &[String] = if req.job_ids.len() > MATCH_BATCH_MAX {
        tracing::warn!(
            requested = req.job_ids.len(),
            cap = MATCH_BATCH_MAX,
            "match_resume_batch job_ids exceeds cap; truncating to first {MATCH_BATCH_MAX}"
        );
        &req.job_ids[..MATCH_BATCH_MAX]
    } else {
        &req.job_ids
    };

    let mut results: Vec<Value> = Vec::with_capacity(job_ids.len());
    for job_id in job_ids {
        let score = score_one(
            &app,
            &store,
            &resume,
            resume_raw_keywords.as_deref(),
            &active,
            job_id,
            semantic_enabled,
        )
        .await;
        results.push(score);
    }
    Value::Array(results)
}

/// Build a searchable text blob for a cached job posting (title + description +
/// requirements). Returns None if the posting isn't in the live cache.
fn job_text_for(app: &AppHandle, job_id: &str) -> Option<String> {
    let cache = app.state::<Mutex<PostingsCache>>();
    let guard = cache.lock();
    let posting = guard
        .get_all()
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(job_id))?;

    let mut parts: Vec<String> = Vec::new();
    if let Some(t) = posting.get("title").and_then(|v| v.as_str()) {
        parts.push(t.to_string());
    }
    if let Some(d) = posting.get("description").and_then(|v| v.as_str()) {
        parts.push(d.to_string());
    }
    if let Some(reqs) = posting.get("requirements").and_then(|v| v.as_array()) {
        for r in reqs {
            if let Some(s) = r.as_str() {
                parts.push(s.to_string());
            }
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("\n"))
}

/// Returns (coverage % 0–100, up-to-15 missing keywords sorted).
fn keyword_coverage(job: &HashSet<String>, resume: &HashSet<String>) -> (f64, Vec<String>) {
    if job.is_empty() {
        return (0.0, Vec::new());
    }
    let mut gaps: Vec<String> = job.difference(resume).cloned().collect();
    gaps.sort();
    let matched = job.len() - gaps.len();
    let coverage = (matched as f64 / job.len() as f64 * 100.0).round();
    gaps.truncate(15);
    (coverage, gaps)
}

fn recommendations(gaps: &[String]) -> Vec<String> {
    if gaps.is_empty() {
        return vec!["Strong keyword coverage — no obvious gaps.".to_string()];
    }
    let preview: Vec<&str> = gaps.iter().take(8).map(String::as_str).collect();
    vec![format!(
        "Consider adding evidence of: {}.",
        preview.join(", ")
    )]
}

#[tauri::command]
pub async fn resume_extract_text(req: ResumeExtractTextRequest) -> Value {
    match crate::extraction::route(&req.name, &req.bytes) {
        Ok(r) => json!({ "text": r.text, "confidence": format!("{:?}", r.confidence) }),
        Err(crate::extraction::types::ExtractionError::ScannedPdfWithoutOcr) => {
            json!({ "error": "scanned_pdf", "message": "PDF appears to be scanned. Please upload a text-based PDF or DOCX." })
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn set(words: &[&str]) -> HashSet<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    // Keyword-extraction behaviour (stopwords, synonyms, short terms) is now
    // owned and tested by `crate::documents::keywords`. These cover the
    // coverage/gap math that still lives here.

    #[test]
    fn coverage_full_when_resume_has_all() {
        let job = set(&["rust", "react", "docker"]);
        let resume = set(&["rust", "react", "docker", "extra"]);
        let (cov, gaps) = keyword_coverage(&job, &resume);
        assert_eq!(cov, 100.0);
        assert!(gaps.is_empty());
    }

    #[test]
    fn coverage_reports_gaps() {
        let job = set(&["rust", "react", "docker", "kubernetes"]);
        let resume = set(&["rust", "react"]);
        let (cov, gaps) = keyword_coverage(&job, &resume);
        assert_eq!(cov, 50.0);
        assert_eq!(gaps, vec!["docker".to_string(), "kubernetes".to_string()]);
    }

    #[test]
    fn coverage_empty_job_is_zero() {
        let (cov, gaps) = keyword_coverage(&HashSet::new(), &set(&["rust"]));
        assert_eq!(cov, 0.0);
        assert!(gaps.is_empty());
    }

    // Corrupt keywords_json must not silently produce an empty resume word-set.
    // Verifies that the match-branch falls back to live extraction so ATS
    // score is computed from the resume text rather than an empty HashSet.
    #[test]
    fn corrupt_keywords_json_falls_back_to_live_extraction() {
        use crate::documents::keywords::make_stemmer;

        let resume_text = "experienced rust and typescript developer";
        let stemmer = make_stemmer(resume_text);

        // Simulate the deserialization branch directly: malformed JSON that
        // would previously silent-default to Vec::new() / empty HashSet.
        let corrupt_json = "not valid json [[[";
        let resume_words: HashSet<String> = match serde_json::from_str::<Vec<String>>(corrupt_json)
        {
            Ok(tokens) => apply_stemmer(tokens.into_iter().collect(), &stemmer),
            Err(_) => keywords(resume_text, &stemmer),
        };

        // The fallback must not be empty — the resume text has real content.
        assert!(
            !resume_words.is_empty(),
            "corrupt keywords_json must fall back to live extraction, not an empty set"
        );

        // A job keyword present in the resume text must be covered.
        let job = keywords("rust developer typescript", &stemmer);
        let (cov, _gaps) = keyword_coverage(&job, &resume_words);
        assert!(
            cov > 0.0,
            "ATS coverage must be > 0 when resume text contains matching terms"
        );
    }

    // Pins the production `semantic_enabled_bit` helper (used by both the cache
    // key and the skip-branch): `Some(false)` → 0 (disabled); `Some(true)` and
    // `None` → 1 (enabled). Tests the real fn, not an inline re-implementation.
    #[test]
    fn semantic_enabled_bit_maps_flag_to_key_column() {
        assert_eq!(semantic_enabled_bit(Some(false)), 0, "explicit disable → 0");
        assert_eq!(semantic_enabled_bit(Some(true)), 1, "explicit enable → 1");
        assert_eq!(semantic_enabled_bit(None), 1, "default (unset) → enabled");
    }

    // The batch request must deserialize from the camelCase wire shape the
    // renderer sends (`resumeId`/`jobIds`/`semanticScoringEnabled`). Pins the
    // generated serde contract without needing an AppHandle.
    #[test]
    fn match_resume_batch_request_deserializes_camel_case() {
        let json = r#"{"resumeId":"r","jobIds":["a","b"],"semanticScoringEnabled":false}"#;
        let req: MatchResumeBatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.resume_id, "r");
        assert_eq!(req.job_ids, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(req.semantic_scoring_enabled, Some(false));
    }

    // A bump to MATCH_FORMULA_VERSION must change the cache key, so a score
    // cached under the current version is a miss under the next one. Exercises
    // self-invalidation end-to-end against a real store.
    #[test]
    fn formula_version_bump_invalidates_cached_score() {
        use crate::documents::{sha256_hex, DocumentStore, MatchScoreKey};
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

        let hash = sha256_hex("job text");
        let key = |fv: i64| MatchScoreKey {
            resume_id: "r",
            job_id: "j",
            provider: "ollama",
            model: "nomic-embed-text",
            semantic_enabled: 1,
            formula_version: fv,
            job_text_hash: &hash,
        };

        // Cache a score under the current formula version → hit.
        store
            .upsert_match_score(&key(MATCH_FORMULA_VERSION), "{\"combined\":50}")
            .unwrap();
        assert!(store.get_match_score(&key(MATCH_FORMULA_VERSION)).is_some());

        // The next formula version is a different key → miss (stale on bump).
        assert!(store
            .get_match_score(&key(MATCH_FORMULA_VERSION + 1))
            .is_none());
    }

    // MATCH_FORMULA_VERSION guard: if a maintainer bumps the constant they MUST
    // also bump the expected value here and invalidate any affected caches.
    // Failing here is intentional — it's the reminder that a bump is breaking.
    #[test]
    fn formula_version_constant_is_pinned() {
        assert_eq!(
            MATCH_FORMULA_VERSION, 1,
            "MATCH_FORMULA_VERSION changed — update this assert AND invalidate \
             cached match scores (clear match_scores table or bump the stored version)"
        );
    }

    // Round-trip parity: a 7-field MatchScore JSON blob survives
    // upsert_match_score → get_match_score with every field name and type intact.
    // Guards against a future rename/drop of any result-cache field.
    #[test]
    fn match_score_round_trip_preserves_all_seven_fields() {
        use crate::documents::{sha256_hex, DocumentStore, MatchScoreKey};
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

        let hash = sha256_hex("round trip job text");
        let key = MatchScoreKey {
            resume_id: "resume-rt",
            job_id: "job-rt",
            provider: "ollama",
            model: "nomic-embed-text",
            semantic_enabled: 1,
            formula_version: MATCH_FORMULA_VERSION,
            job_text_hash: &hash,
        };

        // Build a known 7-field score JSON that mirrors the shape score_one produces.
        let score_json = serde_json::json!({
            "resumeId":       "resume-rt",
            "jobId":          "job-rt",
            "ats":            60.0_f64,
            "semantic":       75.0_f64,
            "combined":       70.0_f64,
            "gaps":           ["kubernetes", "terraform"],
            "recommendations": ["Consider adding evidence of: kubernetes, terraform."]
        });
        store
            .upsert_match_score(&key, &serde_json::to_string(&score_json).unwrap())
            .unwrap();

        let got = store
            .get_match_score(&key)
            .expect("score must be present after upsert");

        assert_eq!(
            got["resumeId"], "resume-rt",
            "resumeId field must survive round-trip"
        );
        assert_eq!(
            got["jobId"], "job-rt",
            "jobId field must survive round-trip"
        );
        assert_eq!(
            got["ats"], 60.0_f64,
            "ats field must survive round-trip as a number"
        );
        assert_eq!(
            got["semantic"], 75.0_f64,
            "semantic field must survive round-trip as a number"
        );
        assert_eq!(
            got["combined"], 70.0_f64,
            "combined field must survive round-trip as a number"
        );
        assert!(
            got["gaps"].is_array(),
            "gaps must survive round-trip as an array"
        );
        assert_eq!(
            got["gaps"].as_array().unwrap().len(),
            2,
            "gaps array length must be preserved"
        );
        assert!(
            got["recommendations"].is_array(),
            "recommendations must survive round-trip as an array"
        );
        // Distinct values: ats != semantic != combined — guards against field swap.
        assert_ne!(
            got["ats"], got["combined"],
            "ats and combined must be distinct"
        );
        assert_ne!(
            got["semantic"], got["combined"],
            "semantic and combined must be distinct"
        );
    }

    // Pins the server-side DoS cap and the slice-truncation logic used by
    // `match_resume_batch` (pure slice math — no AppHandle needed). The cap
    // bounds a malicious direct IPC invoke; truncation must keep input order.
    #[test]
    fn match_batch_cap_truncates_preserving_order() {
        assert_eq!(MATCH_BATCH_MAX, 1000, "cap value is pinned");

        let v: Vec<String> = (0..MATCH_BATCH_MAX + 5).map(|i| i.to_string()).collect();
        let capped = &v[..MATCH_BATCH_MAX];

        assert_eq!(capped.len(), MATCH_BATCH_MAX, "truncated to the cap");
        assert_eq!(capped.first(), Some(&"0".to_string()), "first preserved");
        assert_eq!(
            capped.last(),
            Some(&(MATCH_BATCH_MAX - 1).to_string()),
            "last of the capped slice preserved in order"
        );
    }
}
