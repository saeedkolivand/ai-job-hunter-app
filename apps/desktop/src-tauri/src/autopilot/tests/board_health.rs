//! The board-health verdict is a display-time derivation of the live store: it must never be
//! persisted into a record or ride out in a backup bundle.

use super::super::*;
use super::support::*;
use crate::scraping::{BoardHealth, BoardHealthStatus, BoardScrapeSummary};

/// A "wwr" summary that errored with HTTP 500 and carries a stale `Failing` verdict — what an
/// intermediate build, a hand edit or a tampered bundle can leave on a record. The streak length
/// doubles as the failed-run tally.
fn planted_summary(
    consecutive_failures: u32,
    last_error: &str,
    last_run_id: &str,
    verified_runs: u32,
) -> BoardScrapeSummary {
    let mut summary = board_summary("wwr", 0, Some("HTTP 500"), None, None);
    summary.health = Some(BoardHealth {
        status: BoardHealthStatus::Failing,
        consecutive_failures,
        last_success_at: None,
        last_verified_at: Some(1_767_225_600_000),
        failing_since: Some(1_767_225_600_000),
        last_error: Some(last_error.into()),
        last_run_id: Some(last_run_id.into()),
        verified_runs,
        failed_runs: consecutive_failures,
    });
    summary
}

#[test]
fn record_run_strips_board_health_so_it_never_reaches_a_backup() {
    let (temp, store) = temp_store();
    let ap = create_ap(&store, "ap", "aggregator", 0.0, "manual");

    // A run whose summary carries this machine's cross-run reliability verdict.
    let mut summary = board_summary("aggregator", 0, Some("429 Too Many Requests"), None, None);
    summary.health = Some(BoardHealth {
        status: BoardHealthStatus::Failing,
        consecutive_failures: 4,
        last_success_at: Some(1_767_225_600_000),
        last_verified_at: Some(1_767_225_600_000),
        failing_since: Some(1_767_225_600_000),
        last_error: Some("429 Too Many Requests".into()),
        last_run_id: Some("job-secret".into()),
        verified_runs: 9,
        failed_runs: 4,
    });
    store.record_run(
        &ap.id,
        0,
        0,
        Vec::new(),
        vec![summary],
        &no_tombstones(),
        &[],
    );

    // Read the RAW on-disk bytes directly — not through any store's `get()`.
    // `AutopilotStore::load` ALSO scrubs `health` on every read now (closing
    // the same leak's third sink, an on-disk file from an intermediate
    // build), so reading back through a store — even a freshly reopened one
    // with a cold cache — can no longer prove `record_run` itself did the
    // stripping: `load`'s scrub would mask a `record_run` regression too.
    // Only the literal bytes `write_to_disk` produced prove THAT.
    let on_disk = std::fs::read_to_string(temp.path().join("autopilots.json")).unwrap();
    // The run's own diagnostics survive untouched…
    assert!(
        on_disk.contains("429 Too Many Requests"),
        "the run's own diagnostics must survive on disk; got: {on_disk}"
    );
    // …but the health verdict is a display-time derivation of the LIVE store and
    // must not be frozen into a persisted record.
    assert!(
        !on_disk.contains("\"health\""),
        "record_run must not persist the board-health verdict; got: {on_disk}"
    );
    assert!(
        !on_disk.contains("job-secret"),
        "record_run must not persist the board-health run id; got: {on_disk}"
    );

    // The load-bearing consequence: `AutopilotStore::export` writes
    // `lastRunSummaries` verbatim into the backup bundle, so a leak here would
    // replay THIS machine's failure streaks (and `lastRunId`) on another one.
    let bundle = {
        use crate::data_store::DataStore as _;
        serde_json::to_string(&store.export()).unwrap()
    };
    assert!(
        !bundle.contains("\"health\""),
        "the backup bundle must carry no board-health verdict"
    );
    assert!(
        !bundle.contains("job-secret"),
        "the backup bundle must carry no board-health run id"
    );
}

#[test]
fn export_strips_board_health_even_from_a_record_that_already_had_it() {
    // Independent of `record_run`'s strip: a record written by an intermediate
    // build (or restored from one) can already carry a verdict, and `export` is
    // the boundary where it would actually leave the machine.
    let (_temp, store) = temp_store();
    create_ap(&store, "ap", "aggregator", 0.0, "manual");

    let mut records = store.list();
    records[0].last_run_summaries = vec![planted_summary(7, "machine-a-only", "job-machine-a", 11)];
    store.replace_all(records);

    let bundle = {
        use crate::data_store::DataStore as _;
        serde_json::to_string(&store.export()).unwrap()
    };
    // The run's own diagnostics still export — only the cross-run verdict is cut.
    assert!(
        bundle.contains("HTTP 500"),
        "the run summary itself exports"
    );
    for leak in [
        "\"health\"",
        "consecutiveFailures",
        "machine-a-only",
        "job-machine-a",
    ] {
        assert!(
            !bundle.contains(leak),
            "the bundle must not carry '{leak}'; got {bundle}"
        );
    }
}

#[test]
fn import_strips_board_health_from_a_legacy_or_tampered_bundle() {
    // The other direction of `export_strips_board_health_even_from_a_record_
    // that_already_had_it`: a bundle that already carries `lastRunSummaries[].
    // health` (a pre-strip build's export, or a hand-edited backup) must not
    // land the verdict back on the machine that imports it.
    let (_source_dir, source) = temp_store();
    let ap = create_ap(&source, "ap", "aggregator", 0.0, "manual");

    let mut records = source.list();
    records[0].last_run_summaries = vec![planted_summary(7, "machine-a-only", "job-machine-a", 11)];
    let bundle = serde_json::to_value(&records).unwrap();
    // Sanity: the synthetic bundle really does carry the verdict pre-import.
    assert!(serde_json::to_string(&bundle)
        .unwrap()
        .contains("machine-a-only"));

    let (restored_dir, restored) = temp_store();
    {
        use crate::data_store::DataStore as _;
        restored.import(&bundle).unwrap();
    }

    // `restored`'s cache was just set directly by `import` → `replace_all` →
    // `save` — NOT via `load()` (`save` bypasses it) — so this proves
    // `import`'s OWN strip fired, independent of `load`'s separate scrub for
    // the same leak's third sink (see `load_strips_board_health_left_by_an_
    // intermediate_build`), which would otherwise mask a regression here.
    let reloaded = restored.get(&ap.id).unwrap();
    assert_eq!(reloaded.last_run_summaries.len(), 1);
    assert!(
        reloaded.last_run_summaries[0].health.is_none(),
        "import must strip the verdict; got {:?}",
        reloaded.last_run_summaries[0].health
    );

    // And the literal bytes `import`'s `save()` wrote to disk must not carry
    // it either — the strongest proof, independent of any store's read path.
    let on_disk = std::fs::read_to_string(restored_dir.path().join("autopilots.json")).unwrap();
    assert!(
        !on_disk.contains("\"health\"") && !on_disk.contains("machine-a-only"),
        "the verdict must not reach disk via import either; got: {on_disk}"
    );
}

#[test]
fn load_strips_board_health_left_by_an_intermediate_build() {
    // The THIRD sink: an on-disk `autopilots.json` written before the
    // `record_run`/`export`/`import` strips existed (or hand-edited) can
    // still carry `lastRunSummaries[].health`. A cold `load()` (fresh store,
    // no warm cache) must scrub it going IN — otherwise any unrelated
    // mutation (`set_run_status`, `stamp_last_run`, …) would keep
    // re-persisting the stale verdict forever, since only `record_run`
    // itself ever touches that field.
    let (_temp, dir) = temp_dir();

    let seed = AutopilotStore::new(&dir);
    let ap = create_ap(&seed, "ap", "aggregator", 0.0, "manual");
    let mut records = seed.list();
    records[0].last_run_summaries = vec![planted_summary(3, "stale-from-old-build", "job-old", 5)];
    // Written directly — bypassing every strip this build has, simulating a
    // file this build never wrote through.
    std::fs::write(
        dir.join("autopilots.json"),
        serde_json::to_string_pretty(&records).unwrap(),
    )
    .unwrap();

    // Cold cache: this must go through `load()`'s parse path, not reuse
    // `seed`'s in-memory cache (which `save()` set directly, bypassing it).
    let cold = AutopilotStore::new(&dir);
    let reloaded = cold.get(&ap.id).unwrap();
    assert_eq!(reloaded.last_run_summaries.len(), 1);
    assert!(
        reloaded.last_run_summaries[0].health.is_none(),
        "a cold load must strip health left by an intermediate build; got {:?}",
        reloaded.last_run_summaries[0].health
    );
}
