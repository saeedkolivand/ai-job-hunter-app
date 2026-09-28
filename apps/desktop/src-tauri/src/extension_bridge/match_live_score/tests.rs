use super::*;
use serde_json::json;

fn doc(id: &str, title: &str, is_default: bool, created_at: u64) -> DocumentRecord {
    DocumentRecord {
        id: id.to_string(),
        title: title.to_string(),
        name: String::new(),
        locale: None,
        text: "resume text".to_string(),
        pages: None,
        created_at,
        indexed: false,
        is_default,
        keywords_json: None,
    }
}

// ── resolve_resume ────────────────────────────────────────────────────────

#[test]
fn resolve_resume_prefers_is_default_over_most_recent() {
    let docs = vec![
        doc("newest", "Newest", false, 200),
        doc("default", "Default resume", true, 100),
    ];
    let picked = resolve_resume(&docs).expect("must pick a resume");
    assert_eq!(
        picked.id, "default",
        "is_default must win even though it's not first/newest"
    );
}

#[test]
fn resolve_resume_falls_back_to_most_recent_when_no_default() {
    // Mirrors DocumentStore::list()'s ordering contract: already created_at
    // DESC, so the FIRST entry is the most recent — this fn must not re-sort.
    let docs = vec![
        doc("most-recent", "Most recent", false, 200),
        doc("older", "Older", false, 100),
    ];
    let picked = resolve_resume(&docs).expect("must pick a resume");
    assert_eq!(picked.id, "most-recent");
}

#[test]
fn resolve_resume_none_when_no_documents() {
    assert!(resolve_resume(&[]).is_none());
}

// ── parse_job_text / posting_job_text ────────────────────────────────────

#[test]
fn parse_job_text_none_for_blank_page() {
    // `parse_from_html` always returns `Some` for a successfully-parsed
    // document (title may be an empty string — see its doc), so a blank
    // page yields `Some(JobPosting { title: "", description: None, .. })`.
    // `posting_text_blob` then has nothing usable, so `parse_job_text`
    // must propagate `None` from THAT step, not panic or fabricate text.
    assert!(parse_job_text(
        "https://example.com/not-a-job",
        "<html><body></body></html>"
    )
    .is_none());
}

#[test]
fn posting_job_text_joins_title_description_requirements() {
    let posting = JobPosting {
        id: "job-1".to_string(),
        external_id: None,
        title: "Senior Rust Engineer".to_string(),
        company: "Acme".to_string(),
        location: None,
        url: "https://example.com/job/1".to_string(),
        source: "url".to_string(),
        description: Some("Build reliable systems.".to_string()),
        requirements: Some(vec!["Rust".to_string(), "Tokio".to_string()]),
        posted_at: None,
        captured_at: 0,
        extra: std::collections::HashMap::new(),
    };
    let text = posting_job_text(&posting).expect("posting has usable text");
    assert!(text.contains("Senior Rust Engineer"));
    assert!(text.contains("Build reliable systems."));
    assert!(text.contains("Rust"));
    assert!(text.contains("Tokio"));
}

#[test]
fn posting_job_text_none_when_everything_blank() {
    let posting = JobPosting {
        id: "job-2".to_string(),
        external_id: None,
        title: String::new(),
        company: "Acme".to_string(),
        location: None,
        url: "https://example.com/job/2".to_string(),
        source: "url".to_string(),
        description: None,
        requirements: None,
        posted_at: None,
        captured_at: 0,
        extra: std::collections::HashMap::new(),
    };
    assert!(posting_job_text(&posting).is_none());
}

// ── adhoc_job_id ──────────────────────────────────────────────────────────

#[test]
fn adhoc_job_id_is_stable_and_prefixed() {
    let a = adhoc_job_id("https://example.com/job/1");
    let b = adhoc_job_id("https://example.com/job/1");
    assert_eq!(a, b, "same url must yield the same cache key");
    assert!(
        a.starts_with("adhoc:"),
        "must be prefixed so it can never collide with a real posting id"
    );
}

#[test]
fn adhoc_job_id_differs_per_url() {
    let a = adhoc_job_id("https://example.com/job/1");
    let b = adhoc_job_id("https://example.com/job/2");
    assert_ne!(a, b);
}

// ── canonicalized_normalized_url (cache-key parity with handle_import) ───

/// A raw url carrying www / a trailing slash / a tracking query param, and
/// its ALREADY-normalized form, must compute the IDENTICAL ad-hoc cache
/// key — the MEDIUM cache-key-parity fix: a "Check fit" click and an
/// import on the same page must hit the same `match_scores` row.
#[test]
fn resolve_match_live_cache_key_matches_import_normalization() {
    let raw = "https://www.acme.example/jobs/42/?utm_source=ext";
    let already_normalized = "https://acme.example/jobs/42";
    assert_eq!(
        adhoc_job_id(&canonicalized_normalized_url(raw)),
        adhoc_job_id(&canonicalized_normalized_url(already_normalized)),
        "raw and pre-normalized url variants must hit the same ad-hoc cache key"
    );

    // A #fragment variant (e.g. a same-page anchor like #apply) must also
    // collapse to the same cache key — normalize_job_url strips fragments too.
    let with_fragment = "https://www.acme.example/jobs/42/#apply";
    assert_eq!(
        adhoc_job_id(&canonicalized_normalized_url(with_fragment)),
        adhoc_job_id(&canonicalized_normalized_url(already_normalized)),
        "a #fragment url variant must hit the same ad-hoc cache key"
    );
}

#[test]
fn canonicalized_normalized_url_matches_normalize_job_url_for_a_plain_url() {
    // No SPA/list-view rewrite applies to a plain job-detail-shaped url, so
    // this must equal a direct `normalize_job_url` call (no surprise host).
    let url = "https://www.acme.example/jobs/42/";
    assert_eq!(
        canonicalized_normalized_url(url),
        crate::applications::normalize_job_url(url)
    );
}

// ── build_match_ok (extraction + gap clamping) ───────────────────────────

#[test]
fn build_match_ok_clamps_gaps_to_max() {
    // A synthetic score_one-shaped Value with MORE than MAX_GAPS entries —
    // exercises the REAL `.take(MAX_GAPS)` clamp, not a pre-clamped mock.
    let many_gaps: Vec<String> = (0..20).map(|i| format!("kw{i}")).collect();
    let result = json!({
        "combined": 50.0,
        "ats": 40.0,
        "gaps": many_gaps,
    });
    let ok = build_match_ok(&result, "Resume".to_string());
    assert_eq!(ok.gaps.len(), MAX_GAPS, "gaps must be clamped to MAX_GAPS");
    assert_eq!(
        ok.gaps,
        many_gaps[..MAX_GAPS],
        "the clamp must keep the FIRST MAX_GAPS entries"
    );
    assert_eq!(ok.combined, 50.0);
    assert_eq!(ok.ats, 40.0);
}

#[test]
fn build_match_ok_passes_through_fewer_than_max_gaps_unclamped() {
    let result = json!({ "combined": 10.0, "ats": 5.0, "gaps": ["one", "two"] });
    let ok = build_match_ok(&result, "Resume".to_string());
    assert_eq!(ok.gaps, vec!["one".to_string(), "two".to_string()]);
}

#[test]
fn build_match_ok_defaults_missing_numeric_fields_to_zero() {
    let result = json!({});
    let ok = build_match_ok(&result, "Resume".to_string());
    assert_eq!(ok.combined, 0.0);
    assert_eq!(ok.ats, 0.0);
    assert!(ok.gaps.is_empty());
}
