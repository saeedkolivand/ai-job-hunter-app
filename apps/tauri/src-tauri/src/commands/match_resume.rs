use std::collections::HashSet;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::documents::{cosine_similarity, embed, DocumentStore};
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

    // Semantic: reuse the resume's stored vector when present, else embed its text.
    let resume_vec = store.get_vector(&req.resume_id);
    let resume_vec = match resume_vec {
        Some(v) => Some(v),
        None => embed(&resume.text).await,
    };
    let job_vec = embed(&job_text).await;
    let semantic = match (&resume_vec, &job_vec) {
        (Some(a), Some(b)) => (cosine_similarity(a, b).clamp(0.0, 1.0) * 100.0).round(),
        _ => 0.0, // Ollama offline — fall back to keyword-only.
    };

    // ATS: how many job keywords appear in the resume text.
    let job_keywords = keywords(&job_text);
    let resume_words = keywords(&resume.text);
    let (ats, gaps) = keyword_coverage(&job_keywords, &resume_words);

    let combined = if job_vec.is_some() {
        (0.6 * semantic + 0.4 * ats).round()
    } else {
        ats // no semantic signal available
    };

    let recommendations = recommendations(&gaps);
    let explanation = format!(
        "Semantic similarity {semantic:.0}%, keyword coverage {ats:.0}% across {} job keywords.",
        job_keywords.len()
    );

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

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "you", "your", "are", "our", "will", "have", "this", "that",
    "from", "they", "their", "them", "all", "but", "not", "who", "can", "out", "use", "any",
    "has", "had", "was", "were", "what", "when", "which", "while", "into", "over", "than", "such",
    "able", "work", "role", "team", "join", "must", "etc", "via", "per",
];

/// Extract a deduplicated set of meaningful lowercase keywords (length > 3,
/// excluding common stopwords).
fn keywords(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '+' && c != '#')
        .map(|w| w.trim_matches(|c: char| c == '+' || c == '#').to_lowercase())
        .filter(|w| w.len() > 3 && !STOPWORDS.contains(&w.as_str()))
        .collect()
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

    #[test]
    fn keywords_filters_short_and_stopwords() {
        let kw = keywords("Rust and TypeScript with the React framework");
        assert!(kw.contains("rust"));
        assert!(kw.contains("typescript"));
        assert!(kw.contains("react"));
        assert!(kw.contains("framework"));
        assert!(!kw.contains("and")); // stopword
        assert!(!kw.contains("the")); // stopword
        assert!(!kw.contains("with")); // stopword
    }

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
}
