use std::collections::HashSet;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::documents::keywords::{apply_stemmer, keywords, make_stemmer};
use crate::documents::{embed, DocumentStore};
use crate::ipc_contracts::matching::MatchResumeRequest;
use crate::ipc_contracts::resume::ResumeExtractTextRequest;
use crate::postings::PostingsCache;

/// Score a resume against a job posting.
///
/// Returns a `MatchScore` (see packages/shared types): a semantic score from
/// embedding cosine similarity, an ATS score from job-keyword coverage, a
/// weighted `combined` score, the missing keywords (`gaps`), and short
/// recommendations. Degrades gracefully to keyword-only when Ollama is offline.
#[tauri::command]
pub async fn match_resume(app: AppHandle, req: MatchResumeRequest) -> Value {
    let store = app.state::<DocumentStore>();
    let Some(resume) = store.get(&req.resume_id) else {
        return json!({ "error": format!("resume not found: {}", req.resume_id) });
    };

    let Some(job_text) = job_text_for(&app, &req.job_id) else {
        return json!({ "error": format!("job not found in cache: {}", req.job_id) });
    };

    // Optional, local-only translation: when the JD language differs from the
    // resume locale and a local provider is configured, translate before keyword
    // extraction (and embedding) so matching happens in the resume language.
    // Always falls back to the original text on any failure. Cloud providers are
    // excluded, so this never incurs an unexpected API cost.
    let target_lang = resume.locale.as_deref().unwrap_or("en");
    let translated = crate::commands::translation::translate_if_needed(
        &app,
        &req.job_id,
        &job_text,
        target_lang,
    )
    .await;
    let job_text = translated; // shadow with the owned String; downstream code unchanged

    let skip_semantic = req.semantic_scoring_enabled == Some(false);
    let active = store.embedding_config();
    let (resume_vec, job_vec) = if skip_semantic {
        (None, None)
    } else {
        let rv = match store.get_vector(&req.resume_id) {
            Some(v) if active.matches(&v.space) => Some(v),
            _ => {
                let v = embed(&app, &resume.text).await;
                if let Some(ref ev) = v {
                    let _ = store.upsert_vector(&req.resume_id, ev);
                }
                v
            }
        };
        let jv = embed(&app, &job_text).await;
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
    // Legacy documents (imported before the cache existed) fall back to live
    // extraction from the stored text.
    let resume_words: HashSet<String> = match &resume.keywords_json {
        Some(json) => match serde_json::from_str::<Vec<String>>(json) {
            Ok(tokens) => apply_stemmer(tokens.into_iter().collect(), &stemmer),
            Err(_) => keywords(&resume.text, &stemmer), // corrupted cache → recompute
        },
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

    json!({
        "resumeId": req.resume_id,
        "jobId": req.job_id,
        "ats": ats,
        "semantic": semantic,
        "combined": combined,
        "gaps": gaps,
        "recommendations": recommendations,
        "explanation": explanation,
    })
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
}
