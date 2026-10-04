//! `keyword_coverage` and `coverage_score`: the share of a posting's keywords a résumé covers.

use super::*;

fn set(words: &[&str]) -> HashSet<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn keyword_coverage_full_when_resume_has_all() {
    let job = set(&["rust", "react", "docker"]);
    let resume = set(&["rust", "react", "docker", "extra"]);
    let (cov, gaps) = keyword_coverage(&job, &resume).expect("non-empty job must return Some");
    assert_eq!(cov, 100.0);
    assert!(gaps.is_empty());
}

#[test]
fn keyword_coverage_reports_sorted_gaps() {
    let job = set(&["rust", "react", "docker", "kubernetes"]);
    let resume = set(&["rust", "react"]);
    let (cov, gaps) = keyword_coverage(&job, &resume).expect("non-empty job must return Some");
    assert_eq!(cov, 50.0);
    assert_eq!(gaps, vec!["docker".to_string(), "kubernetes".to_string()]);
}

#[test]
fn keyword_coverage_empty_job_returns_none() {
    // Empty JD keyword set → None (distinguishable from 0% real mismatch).
    assert!(
        keyword_coverage(&HashSet::new(), &set(&["rust"])).is_none(),
        "empty job keyword set must return None, not Some(0.0)"
    );
}

#[test]
fn keyword_coverage_caps_gaps_at_fifteen() {
    let job: HashSet<String> = (0..30).map(|i| format!("skill{i:02}")).collect();
    let (cov, gaps) =
        keyword_coverage(&job, &HashSet::new()).expect("non-empty job must return Some");
    assert_eq!(cov, 0.0);
    assert_eq!(gaps.len(), 15, "gaps must be truncated to 15");
}

/// `coverage_score` is the embedding-free Jobs-page ATS kernel: a résumé that
/// contains all the JD's keywords scores high; an unrelated one scores 0.
#[test]
fn coverage_score_matches_and_misses() {
    let full = coverage_score(
        "experienced rust kubernetes docker engineer",
        "rust kubernetes docker",
    );
    assert_eq!(full, 100.0, "résumé covering all JD keywords → 100");

    let none = coverage_score("java spring developer", "rust kubernetes docker");
    assert_eq!(none, 0.0, "no overlap → 0");

    let partial = coverage_score("rust developer", "rust kubernetes docker");
    assert!(
        partial > 0.0 && partial < 100.0,
        "partial overlap must be strictly between 0 and 100; got {partial}"
    );
}

/// `coverage_score` must agree with the underlying `keyword_coverage` kernel
/// (single source of the formula — guards against the two drifting apart).
#[test]
fn coverage_score_agrees_with_keyword_coverage_kernel() {
    let resume = "rust developer with docker";
    let job = "rust kubernetes docker terraform";
    let stemmer = make_stemmer(job);
    let (kernel, _gaps) = keyword_coverage(&keywords(job, &stemmer), &keywords(resume, &stemmer))
        .expect("non-empty job must return Some");
    assert_eq!(coverage_score(resume, job), kernel);
}
