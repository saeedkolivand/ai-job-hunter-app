use crate::pipeline::runs::{PipelineRunStore, RunRow};

/// Same text → same bucket (so re-running one pasted posting still caps at
/// `RETENTION_RUNS_PER_JOB`); different text → different bucket.
#[test]
fn unlinked_run_key_is_deterministic_and_distinct_per_text() {
    let a1 = super::super::resolve::unlinked_run_key("Staff Engineer at Acme — full ad text");
    let a2 = super::super::resolve::unlinked_run_key("Staff Engineer at Acme — full ad text");
    let b = super::super::resolve::unlinked_run_key("Backend Engineer at Globex — full ad text");
    assert_eq!(a1, a2, "the same pasted text must land in the same bucket");
    assert_ne!(a1, b, "different pasted postings must not collide");
}

/// **Must survive the SAME normalization chokepoint `PipelineRunStore::
/// upsert_run` applies before storing.** A non-`http(s)` synthetic key would
/// be neutralized back to `""` there — right back into the shared bucket
/// this function exists to escape. Also pins the reserved-TLD shape, so it
/// can never be mistaken for a resolvable posting link.
#[test]
fn unlinked_run_key_survives_normalize_job_url_and_is_shaped_as_reserved() {
    let key = super::super::resolve::unlinked_run_key("a pasted job ad");
    assert_eq!(
        crate::applications::normalize_job_url(&key),
        key,
        "the run store normalizes every job_url on write; a non-http(s) key would be wiped to \"\""
    );
    assert!(key.starts_with("https://"));
    assert!(
        key.contains(".invalid/"),
        "must be shaped so it can never resolve to a real host"
    );
}

/// `run_store_job_url` — the run-STORE's own key: `job_url` wins whenever
/// nonempty (the ordinary linked case, `Text` or `Cache`); an empty
/// `job_url` on the `Text` path substitutes `unlinked_run_key`; an empty
/// `job_url` on the `Cache` path (the cached posting itself had none — rare,
/// pre-existing) stays empty, unchanged from before PR-3.
#[test]
fn run_store_job_url_substitutes_the_synthetic_key_only_when_unlinked_and_text_path() {
    assert_eq!(
        super::super::resolve::run_store_job_url(
            "https://boards.example/jobs/1",
            super::super::resolve::JobSource::Text("a pasted job ad")
        ),
        "https://boards.example/jobs/1",
        "a real job_url always wins, on either path"
    );
    let synthetic = super::super::resolve::run_store_job_url(
        "",
        super::super::resolve::JobSource::Text("a pasted job ad"),
    );
    assert_eq!(
        synthetic,
        super::super::resolve::unlinked_run_key("a pasted job ad"),
        "an unlinked TEXT-path run must get the synthetic key"
    );
    assert_eq!(
        super::super::resolve::run_store_job_url(
            "",
            super::super::resolve::JobSource::Cache("job-9")
        ),
        "",
        "an unlinked CACHE-path run (the cached posting had no url) stays empty, unchanged"
    );
}

/// **MEDIUM 2, pinned at the STORE level.** `PipelineRunStore::prune`
/// partitions retention on `(job_url, kind)`; leaving every unlinked run
/// under `job_url = ""` would pool every pasted posting's history into one
/// shared bucket. 3 runs of job A (same pasted text) + 1 of job B (different
/// text) both survive `prune()` fully intact; a 4th run of A evicts only A's
/// oldest, never touching B.
#[test]
fn unlinked_runs_of_different_postings_do_not_share_a_retention_bucket() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = PipelineRunStore::open(dir.path()).expect("store opens");

    let key_a = super::super::resolve::unlinked_run_key("Staff Engineer at Acme — full ad text A");
    let key_b =
        super::super::resolve::unlinked_run_key("Backend Engineer at Globex — full ad text B");
    assert_ne!(key_a, key_b, "distinct pasted postings must not collide");

    let row = |id: &str, job_url: &str, started_at: u64| RunRow {
        id: id.to_string(),
        job_url: job_url.to_string(),
        kind: super::super::RUN_KIND.to_string(),
        depth: "quality".to_string(),
        status: "completed".to_string(),
        started_at,
        finished_at: Some(started_at),
        stopped_reason: None,
        metrics_json: "{}".to_string(),
    };

    for (index, started_at) in [1_700_000_000_000u64, 1_700_000_001_000, 1_700_000_002_000]
        .into_iter()
        .enumerate()
    {
        store
            .upsert_run(&row(&format!("run-a-{index}"), &key_a, started_at))
            .expect("A's run persists");
    }
    store
        .upsert_run(&row("run-b-0", &key_b, 1_700_000_000_500))
        .expect("B's run persists");

    store.prune();

    assert_eq!(
        store.runs_for_job(&key_a).len(),
        3,
        "A's 3 runs must survive intact"
    );
    assert_eq!(
        store.runs_for_job(&key_b).len(),
        1,
        "B's 1 run must survive, untouched by A's count"
    );

    // A 4th run of A evicts only A's oldest — B stays untouched.
    store
        .upsert_run(&row("run-a-3", &key_a, 1_700_000_003_000))
        .expect("A's 4th run persists");
    store.prune();

    let a_runs = store.runs_for_job(&key_a);
    assert_eq!(a_runs.len(), 3, "retention caps A at 3");
    assert!(
        a_runs.iter().all(|r| r.id != "run-a-0"),
        "A's oldest run must be the one evicted"
    );
    assert_eq!(
        store.runs_for_job(&key_b).len(),
        1,
        "B is untouched by A's eviction"
    );
}
