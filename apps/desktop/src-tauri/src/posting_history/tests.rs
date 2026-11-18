use super::*;
use crate::data_store::SyncSection;
use tempfile::TempDir;

mod backup;
mod repost;

const DAY: u64 = DAY_MS;
const T0: u64 = 1_700_000_000_000;
/// Two sources: the same board with two different searches, or two boards.
const A: &str = "greenhouse|rust berlin";
const B: &str = "lever|rust berlin";

fn open() -> (TempDir, PostingHistoryStore) {
    let dir = TempDir::new().unwrap();
    let store = PostingHistoryStore::open(dir.path()).unwrap();
    (dir, store)
}

/// A successful scrape of `src` that listed these postings.
fn run(
    store: &PostingHistoryStore,
    at: u64,
    src: &str,
    postings: &[(&str, &str, &str)],
) -> HashMap<String, GhostSignals> {
    let rows = postings.iter().map(|&(c, t, u)| (c, t, u, src));
    store.record_scrape(rows, &[src.to_string()], at).unwrap()
}

/// The Acme posting listed by `src`.
fn see_from(store: &PostingHistoryStore, at: u64, src: &str) -> GhostSignals {
    let out = run(
        store,
        at,
        src,
        &[("Acme GmbH", "Rust Engineer (m/w/d)", "https://acme.test/1")],
    );
    out.into_values().next().unwrap()
}

fn see(store: &PostingHistoryStore, at: u64) -> GhostSignals {
    see_from(store, at, A)
}

/// A successful scrape of `src` that did NOT list Acme (it listed another
/// posting): evidence the posting was absent from that source.
fn without_acme(store: &PostingHistoryStore, at: u64, src: &str) {
    run(
        store,
        at,
        src,
        &[("Other", "Other Role", "https://other.test/1")],
    );
}

fn acme_row(store: &PostingHistoryStore) -> HistoryRow {
    store
        .rows()
        .into_iter()
        .find(|r| r.ghost_key.starts_with("acme|"))
        .unwrap()
}

#[test]
fn ghost_key_ignores_legal_suffix_gender_tag_case_and_url() {
    let a = ghost_key("Acme GmbH", "Senior Rust Developer (m/w/d)");
    let b = ghost_key("ACME", "senior rust developer");
    assert!(a.is_some());
    assert_eq!(a, b);
    assert_ne!(a, ghost_key("Acme", "Junior Rust Developer"));
    assert_ne!(a, ghost_key("Other", "Senior Rust Developer"));
}

#[test]
fn ghost_key_is_none_when_either_half_is_empty() {
    assert_eq!(ghost_key("", "Engineer"), None);
    assert_eq!(ghost_key("Acme", "  "), None);
}

#[test]
fn ghost_key_is_cut_at_the_byte_cap_without_splitting_a_char() {
    let key = ghost_key("Acme", &"日".repeat(500)).unwrap();
    // "acme|" is 5 bytes, each 日 is 3: 131 of them fit under 400, a 132nd
    // would straddle the cap and must be dropped whole.
    assert_eq!(key.len(), 5 + 3 * 131);
    assert!(key.ends_with('日'));
}

#[test]
fn source_id_normalizes_case_and_whitespace_and_separates_searches() {
    assert_eq!(
        source_id(" Greenhouse ", "Rust   Berlin"),
        source_id("greenhouse", "rust berlin")
    );
    assert_ne!(source_id("greenhouse", "rust"), source_id("lever", "rust"));
    assert_ne!(
        source_id("greenhouse", "rust"),
        source_id("greenhouse", "go")
    );
}

#[test]
fn first_sighting_has_no_reposts_and_zero_open_days() {
    let (_dir, store) = open();
    let s = see(&store, T0);
    assert_eq!(s, signals_for(T0, 0, T0));
    assert_eq!((s.open_days, s.repost_count), (0, 0));
    assert!(!s.open_long && !s.reposted);
}

#[test]
fn open_long_and_reposted_flip_at_their_thresholds() {
    let below = signals_for(0, REPOSTED_MIN - 1, (u64::from(OPEN_LONG_DAYS) - 1) * DAY);
    assert!(!below.open_long && !below.reposted);
    let at = signals_for(0, REPOSTED_MIN, u64::from(OPEN_LONG_DAYS) * DAY);
    assert!(at.open_long && at.reposted);
    assert_eq!(at.open_days, OPEN_LONG_DAYS);
}

#[test]
fn keys_unseen_for_the_retention_window_are_pruned_at_ingest() {
    let (_dir, store) = open();
    run(&store, T0, A, &[("Old Labs", "Old Role", "u")]);
    run(
        &store,
        T0 + POSTING_HISTORY_RETENTION_DAYS * DAY - DAY,
        A,
        &[("Kept Labs", "Kept Role", "u")],
    );
    assert_eq!(store.rows().len(), 2, "inside the window nothing is pruned");
    run(
        &store,
        T0 + POSTING_HISTORY_RETENTION_DAYS * DAY + DAY,
        A,
        &[("New Labs", "New Role", "u")],
    );
    let keys: Vec<_> = store.rows().into_iter().map(|r| r.ghost_key).collect();
    assert_eq!(keys, ["kept labs|kept role", "new labs|new role"]);
}

#[test]
fn a_posting_with_no_company_or_title_is_not_recorded() {
    let (_dir, store) = open();
    let out = run(&store, T0, A, &[("", "Role", "u"), ("Co", "", "u")]);
    assert!(out.is_empty());
    assert!(store.rows().is_empty());
}

#[test]
fn migration_creates_the_tables_on_a_fresh_db_and_reopen_is_a_no_op() {
    let dir = TempDir::new().unwrap();
    {
        let store = PostingHistoryStore::open(dir.path()).unwrap();
        see(&store, T0);
    }
    let store = PostingHistoryStore::open(dir.path()).unwrap();
    assert_eq!(store.rows().len(), 1, "reopen keeps data");
    assert_eq!(acme_row(&store).last_source, A);
    let v: i64 = store
        .conn
        .lock()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, PostingHistoryStore::MIGRATIONS.len() as i64);
}

fn marker_count(store: &PostingHistoryStore) -> i64 {
    store
        .conn
        .lock()
        .query_row("SELECT COUNT(*) FROM posting_history_meta", [], |r| {
            r.get(0)
        })
        .unwrap()
}

#[test]
fn clear_all_wipes_history_and_markers() {
    let (_dir, store) = open();
    see(&store, T0);
    assert_eq!(marker_count(&store), 1);
    store.clear_all();
    assert!(store.rows().is_empty());
    assert_eq!(marker_count(&store), 0);
}
