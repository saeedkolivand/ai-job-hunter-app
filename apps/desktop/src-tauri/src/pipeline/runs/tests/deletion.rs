//! Per-posting / bulk deletion, all keyed on the NORMALIZED url: what
//! `runs_for_job` lists is what `delete_for_job`/`delete_for_jobs` removes,
//! a selection past SQLite's host-parameter limit still cascades in full,
//! and a legacy (pre-normalization) row is repaired once at open.

use tempfile::TempDir;

use crate::pipeline::runs::PipelineRunStore;

use super::support::{event, run, store};

/// **Deleting an application takes its run trail with it.**
///
/// The trail is not a cache: at max depth a run persists the FULL re-seeded
/// strategy (the whole employment history) and the full evidence map (verbatim
/// quotes out of the résumé) in `artifact_json`, and nothing else ever removes
/// them for a posting the user deleted — `prune` only evicts the fourth run of
/// a posting still being run, and `export` ships every event row into the
/// user's backups.
///
/// The match is on the NORMALIZED url on BOTH sides, because an `Application`
/// is keyed by `normalize_job_url` while a run row carries the postings cache's
/// raw url. A `WHERE job_url = ?` would look right and delete nothing for every
/// posting whose link carries a tracking param.
///
/// Mutation check: compare the raw `job_url` strings instead of the normalized
/// ones and the `utm_source` run survives; drop the events DELETE and the
/// orphan assertion fails.
#[test]
fn deleting_a_job_removes_its_runs_and_their_artifacts() {
    let (_dir, store) = store();
    // Two runs of the SAME posting, written with the raw urls the postings
    // cache hands the pipeline — one of them carrying tracking noise.
    store
        .upsert_run(&run("run-a", "https://boards.example/jobs/42", 1_000))
        .unwrap();
    store
        .upsert_run(&run(
            "run-b",
            "https://www.Boards.example/jobs/42?utm_source=newsletter#apply",
            2_000,
        ))
        .unwrap();
    store
        .append_event(&event("run-a", 0, r#"{"full":{"perCompany":[]}}"#))
        .unwrap();
    store
        .append_event(&event("run-b", 0, r#"{"full":{"items":[]}}"#))
        .unwrap();
    // …and one run of a DIFFERENT posting, which must survive untouched.
    store
        .upsert_run(&run("other", "https://boards.example/jobs/99", 3_000))
        .unwrap();
    store.append_event(&event("other", 0, "{}")).unwrap();

    let removed = store.delete_for_job("https://boards.example/jobs/42");

    assert_eq!(removed, 2, "both spellings of the same posting go");
    assert!(store
        .runs_for_job("https://boards.example/jobs/42")
        .is_empty());
    assert!(
        store.events_for_run("run-a").is_empty() && store.events_for_run("run-b").is_empty(),
        "the artifacts are the point — a deleted run whose events survive keeps the résumé quotes"
    );
    assert_eq!(
        store.runs_for_job("https://boards.example/jobs/99").len(),
        1
    );
    assert_eq!(store.events_for_run("other").len(), 1);

    // An unlinked application has no posting url; matching it against every
    // empty-url run would delete other people's history rather than nothing.
    store.upsert_run(&run("unlinked", "", 4_000)).unwrap();
    assert_eq!(store.delete_for_job(""), 0);
    assert_eq!(store.delete_for_job("   "), 0);
    assert!(store.run("unlinked").is_some(), "…and it survives");
}

/// The same empty-key guard on the READ side.
///
/// `normalized_job_url` maps anything it cannot read as an http(s) url to `""`
/// — a `javascript:` scheme, a control-character paste, whitespace — and `""`
/// is also what every UNLINKED run is stored under. `resume_pipeline_list_for_job`
/// takes its url from the renderer, so without the guard a junk url would list
/// every unlinked run in the store: other postings' history, under a url that
/// names none of them.
///
/// Mutation check: drop the `wanted.is_empty()` early return in `runs_for_job`
/// and every case below returns the unlinked runs.
#[test]
fn a_junk_url_lists_no_runs_rather_than_every_unlinked_one() {
    let (_dir, store) = store();
    store.upsert_run(&run("unlinked-1", "", 1_000)).unwrap();
    store.upsert_run(&run("unlinked-2", "", 2_000)).unwrap();
    store
        .upsert_run(&run("linked", "https://boards.example/jobs/1", 3_000))
        .unwrap();

    for junk in [
        "",
        "   ",
        "javascript:alert(1)",
        "data:text/html,x",
        "\u{1}\u{2}",
    ] {
        assert!(
            store.runs_for_job(junk).is_empty(),
            "runs_for_job({junk:?}) must not answer with someone else's history"
        );
    }
    // …and a real url still resolves, so the guard is about the empty key.
    assert_eq!(store.runs_for_job("https://boards.example/jobs/1").len(), 1);
}

/// **A selection larger than SQLite's host-parameter limit still cascades.**
///
/// `delete_for_jobs` builds one `IN (?, ?, …)` per posting. SQLite refuses to
/// prepare past `SQLITE_MAX_VARIABLE_NUMBER` (32 766 bundled, 999 on older
/// builds), and the failure is silent in the direction that matters: the delete
/// returns 0, the run-trail purge does not happen, and the artifacts it was
/// meant to remove stay on disk. The user's Documents-page selection is
/// unbounded, so the statement has to be chunked.
///
/// **What this test can and cannot reach, measured rather than assumed.** The
/// batch below crosses four chunk boundaries, so it pins that chunking deletes
/// everything and stays atomic — but it does NOT reach SQLite's own limit, and
/// running the unchunked mutation against it PASSES. Seeding 32 766+ postings
/// to make that mutation fail costs minutes of suite time for a bound the
/// assertion below states directly and for free.
///
/// Mutation check: raise `MAX_SQL_PARAMS` above the connection's reported
/// `SQLITE_LIMIT_VARIABLE_NUMBER` and the first assertion fails; drop the
/// chunking and the count/atomicity assertions still hold at this size (which
/// is why the limit assertion is here at all).
#[test]
fn a_selection_past_the_sql_parameter_limit_still_purges_every_trail() {
    let (_dir, store) = store();
    // Comfortably past both the 999 and the 32 766 limits' chunk boundary, and
    // past MAX_SQL_PARAMS several times over.
    let count = crate::db::MAX_SQL_PARAMS * 4 + 7;
    let urls: Vec<String> = (0..count)
        .map(|i| format!("https://boards.example/jobs/{i}"))
        .collect();
    for (i, url) in urls.iter().enumerate() {
        let id = format!("run-{i}");
        store.upsert_run(&run(&id, url, 1_000 + i as u64)).unwrap();
        store
            .append_event(&event(&id, 0, r#"{"full":{"perCompany":[]}}"#))
            .unwrap();
    }
    // …plus one posting nobody selected.
    store
        .upsert_run(&run("keep", "https://boards.example/keep", 9_000))
        .unwrap();
    store.append_event(&event("keep", 0, "{}")).unwrap();

    // The bound the chunking exists for. SQLite refuses to PREPARE a statement
    // with more host parameters than `SQLITE_MAX_VARIABLE_NUMBER` — 32 766 on
    // the bundled build, but 999 on anything older, and rusqlite's runtime
    // accessor for it sits behind a feature this crate does not enable. The
    // conservative floor is the one worth pinning: a chunk size safe there is
    // safe everywhere, and this fails the moment someone raises the constant
    // past it.
    const SQLITE_OLDEST_VARIABLE_LIMIT: usize = 999;
    // A const block: both operands are compile-time constants, and clippy is
    // right that a runtime `assert!` on two of them proves nothing at test time
    // that it would not prove at build time.
    const _: () = assert!(
        crate::db::MAX_SQL_PARAMS < SQLITE_OLDEST_VARIABLE_LIMIT,
        "MAX_SQL_PARAMS must stay under the oldest SQLite host-parameter limit"
    );
    assert!(
        urls.len() > SQLITE_OLDEST_VARIABLE_LIMIT,
        "the premise: this selection would blow that limit as one statement"
    );

    let removed = store.delete_for_jobs(&urls);

    assert_eq!(
        removed, count,
        "every selected posting's runs go in one call"
    );
    assert!(
        (0..count).all(|i| store.events_for_run(&format!("run-{i}")).is_empty()),
        "no event row may survive — they are the artifacts the purge exists for"
    );
    assert_eq!(store.runs_for_job("https://boards.example/keep").len(), 1);
    assert_eq!(store.events_for_run("keep").len(), 1);
}

/// **What the list shows and what the delete removes are the SAME set.**
///
/// They were not. `execute` wrote the postings cache's RAW url while
/// `delete_for_job` normalized before comparing, so a delete correctly took the
/// trail of a posting whose link carried a `utm_*` param — and `runs_for_job`,
/// called with the application's own normalized key, could not find that run to
/// list it. A store whose delete and list disagree about which rows belong to a
/// posting reports one thing and does another.
///
/// The seam is `normalized_job_url`, applied where a url ENTERS (the write) and
/// where one arrives (the by-url readers), so every spelling of a posting
/// resolves to one set of rows.
///
/// Mutation check: store `run.job_url` raw in `upsert_run` and the
/// normalized-lookup assertion fails; drop the normalization in `runs_for_job`
/// and the RAW-lookup one does (that is the arm the review's "keep readers
/// exact-match" prescription would have broken — the renderer passes
/// `posting.url`, not the normalized key).
#[test]
fn every_spelling_of_a_posting_resolves_to_the_same_runs() {
    let (_dir, store) = store();
    const RAW: &str = "https://www.Boards.example/jobs/42?utm_source=newsletter#apply";
    const NORMALIZED: &str = "https://boards.example/jobs/42";

    store.upsert_run(&run("run-1", RAW, 1_000)).unwrap();
    store.append_event(&event("run-1", 0, "{}")).unwrap();

    // The row is STORED in one spelling…
    assert_eq!(
        store.run("run-1").expect("the run").job_url,
        NORMALIZED,
        "the write site is the seam"
    );
    // …and every spelling finds it, including the raw one the renderer holds.
    for spelling in [RAW, NORMALIZED, "https://Boards.example/jobs/42/"] {
        assert_eq!(
            store.runs_for_job(spelling).len(),
            1,
            "runs_for_job({spelling}) must resolve"
        );
    }
    // The list and the delete agree: what one showed, the other removes.
    assert_eq!(store.runs_for_job(NORMALIZED).len(), 1);
    assert_eq!(store.delete_for_job(RAW), 1);
    assert!(store.runs_for_job(NORMALIZED).is_empty());
    assert!(store.events_for_run("run-1").is_empty());
}

/// **A RESTORED bundle is a write, and it goes through the same seam.**
///
/// `import` is the restore path, and it bound `run.job_url` raw while
/// `upsert_run` normalized — so restoring any backup taken before the
/// normalization landed (they all carry the postings cache's raw urls:
/// `utm_*`, fragments, `www.`, uppercase host) wrote rows in a spelling no
/// reader and no delete could reach. `data_import` runs against the LIVE
/// managed store with no re-open, so `normalize_existing_job_urls` does not get
/// a chance to repair them either.
///
/// What that cost, end to end: the runs panel came back empty for those
/// postings, `delete_for_job` matched zero rows and still reported SUCCESS, so
/// both delete cascades silently no-opped — and the next restart's sweep then
/// normalized rows whose owner the user had already deleted, leaving the full
/// strategy and evidence map (employment history, verbatim résumé quotes) on
/// disk permanently and riding into every later backup.
///
/// The two round-trip guards above could not catch it: their fixtures are
/// pre-normalized on both sides, so a raw-binding import looks identical.
///
/// Mutation check: bind `run.job_url` raw in `import` and both assertions fail.
#[test]
fn an_imported_run_is_normalized_like_any_other_write() {
    use crate::data_store::DataStore;

    let (_dir, store) = store();
    const RAW: &str = "https://www.Boards.example/jobs/7?utm_campaign=x";
    const NORMALIZED: &str = "https://boards.example/jobs/7";

    let bundle = serde_json::json!({
        "runs": [{
            "id": "restored",
            "jobUrl": RAW,
            "kind": "resume",
            "depth": "max",
            "status": "completed",
            "startedAt": 1_700_000_000_000i64,
            "finishedAt": 1_700_000_100_000i64,
            "stoppedReason": "done",
            "metricsJson": "{}",
        }],
        "events": [{
            "runId": "restored",
            "seq": 0,
            "ts": 1_700_000_000_000i64,
            "stage": "strategy",
            "phase": "finish",
            "artifactJson": r#"{"full":{"perCompany":[{"company":"Acme"}]}}"#,
        }],
    });
    assert_eq!(store.import(&bundle).expect("the bundle restores"), 1);

    assert_eq!(
        store.run("restored").expect("the run").job_url,
        NORMALIZED,
        "a restore is a write, and the write site is the seam"
    );
    assert_eq!(
        store.runs_for_job(NORMALIZED).len(),
        1,
        "a restored run must be listable"
    );
    assert_eq!(
        store.delete_for_job(NORMALIZED),
        1,
        "…and deletable — a delete that matches nothing still reports success, \
         so an unreachable row is PII with no owner and no eviction"
    );
    assert!(store.events_for_run("restored").is_empty());
}

/// A row written by a BUILD THAT PREDATES the normalization is repaired once, at
/// open — otherwise it stays invisible to every reader that now normalizes, and
/// its artifacts stay undeleteable.
///
/// Written straight through `rusqlite` rather than through `upsert_run`,
/// because `upsert_run` is exactly the thing that would normalize it and there
/// would be nothing to repair.
///
/// Mutation check: delete the `normalize_existing_job_urls` call from `open`
/// and the legacy row is unreachable after the reopen.
#[test]
fn a_legacy_row_written_before_the_normalization_is_repaired_at_open() {
    let dir = TempDir::new().unwrap();
    const RAW: &str = "https://www.Boards.example/jobs/7?utm_campaign=x";
    const NORMALIZED: &str = "https://boards.example/jobs/7";
    {
        let store = PipelineRunStore::open(dir.path()).unwrap();
        store
            .upsert_run(&run("legacy", "placeholder", 1_000))
            .unwrap();
        store.append_event(&event("legacy", 0, "{}")).unwrap();
        // Put the row back the way an older build wrote it.
        store
            .conn
            .lock()
            .execute(
                "UPDATE pipeline_runs SET job_url = ?1 WHERE id = ?2",
                rusqlite::params![RAW, "legacy"],
            )
            .unwrap();
        assert_eq!(store.run("legacy").expect("the run").job_url, RAW);
    }

    let store = PipelineRunStore::open(dir.path()).unwrap();
    assert_eq!(
        store.run("legacy").expect("the run").job_url,
        NORMALIZED,
        "the sweep rewrites a stale spelling once, at open"
    );
    assert_eq!(store.runs_for_job(NORMALIZED).len(), 1);
    assert_eq!(store.delete_for_job(NORMALIZED), 1);
    assert!(store.events_for_run("legacy").is_empty());
}

/// The BATCH cascade behind `ai_generations_remove_bulk` — the Documents page's
/// multi-select delete.
///
/// One posting's trail going is [`PipelineRunStore::delete_for_job`]'s job;
/// this pins that several go together and that a posting nobody deleted keeps
/// its history, which is the property a loop written at three call sites would
/// eventually get wrong at one of them.
///
/// Mutation check: sum nothing (return 0) and the count assertion fails; delete
/// every run regardless of url and the survivor assertion does.
#[test]
fn deleting_several_jobs_purges_exactly_those_trails() {
    let (_dir, store) = store();
    for (id, url) in [
        ("run-a", "https://boards.example/jobs/1"),
        ("run-b", "https://boards.example/jobs/2"),
        ("run-keep", "https://boards.example/jobs/3"),
    ] {
        store.upsert_run(&run(id, url, 1_000)).unwrap();
        store
            .append_event(&event(id, 0, r#"{"full":{"perCompany":[]}}"#))
            .unwrap();
    }

    let removed = store.delete_for_jobs(&[
        "https://boards.example/jobs/1".to_string(),
        "https://boards.example/jobs/2".to_string(),
    ]);

    assert_eq!(removed, 2);
    assert!(store.events_for_run("run-a").is_empty());
    assert!(store.events_for_run("run-b").is_empty());
    assert_eq!(
        store.runs_for_job("https://boards.example/jobs/3").len(),
        1,
        "a posting nobody deleted keeps its history"
    );
    assert_eq!(store.events_for_run("run-keep").len(), 1);
    // Nothing to delete is not an error, and deletes nothing.
    assert_eq!(store.delete_for_jobs(&[]), 0);
    assert_eq!(store.events_for_run("run-keep").len(), 1);
}
